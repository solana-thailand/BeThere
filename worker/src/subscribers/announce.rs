//! The hourly announcer: note what opened, then mail it to everyone who
//! asked before it did, at most [`BATCH`] mails a run.
//!
//! The free plan allows 50 outbound requests per invocation (one token
//! refresh, then one per mail), and `docs/gmail_sender_setup.md` budgets 400
//! recipients a day (Gmail allows about 500): 16 a tick, 23 ticks a day (the
//! 03:xx tick runs the daily jobs instead, `schedule.rs`) is 368. Each mail is claimed before it is
//! sent (one per event per subscriber). A refusal for the account (401, 403,
//! 429) frees the claim for the next run; any other refusal, a network error
//! or a 5xx keeps it: the mail is bad, or it may have gone, and an ambiguous
//! send is never replayed.

use worker::{D1Database, Env};

use super::{copy, db};
use crate::mail::gmail::{Gmail, SendError};
use crate::mail::message::{Mail, gmail_raw};

/// Mails per run.
pub const BATCH: u32 = 16;

/// What one run did, for the log (counts only: no addresses).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub newly_open: usize,
    pub due: usize,
    pub sent: usize,
    pub refused: usize,
    pub ambiguous: usize,
    pub skipped: &'static str,
}

pub async fn run(env: &Env, d1: &D1Database) -> Summary {
    let mut summary = Summary::default();
    let now_ms = chrono::Utc::now().timestamp_millis();
    match db::mark_open(d1, now_ms).await {
        Ok(n) => summary.newly_open = n,
        Err(e) => {
            tracing::warn!(error = %e, "announce: mark_open failed");
            summary.skipped = "mark_open";
            return summary;
        }
    }
    let due = match db::due(d1, now_ms, BATCH).await {
        Ok(due) => due,
        Err(e) => {
            tracing::warn!(error = %e, "announce: due query failed");
            summary.skipped = "due";
            return summary;
        }
    };
    summary.due = due.len();
    if due.is_empty() {
        return summary;
    }
    let site = match env.var("SERVER_URL") {
        Ok(v) => v.to_string().trim_end_matches('/').to_string(),
        Err(_) => {
            summary.skipped = "SERVER_URL";
            return summary;
        }
    };
    let gmail = match Gmail::connect(env).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            summary.skipped = "no gmail secrets";
            return summary;
        }
        Err(e) => {
            tracing::warn!(error = %e, "announce: gmail connect failed");
            summary.skipped = "gmail connect";
            return summary;
        }
    };
    for d in due {
        match db::claim(d1, &d.event.id, &d.email).await {
            Ok(true) => {}
            Ok(false) => continue, // another run has it
            Err(e) => {
                tracing::warn!(error = %e, "announce: claim failed");
                continue;
            }
        }
        let (subject, text) = copy::announcement(&d.event, d.thai, &site, &d.unsub_token);
        let unsubscribe_url = format!("{site}/api/unsubscribe/{}", d.unsub_token);
        let mail = Mail {
            from_name: "BeThere",
            from_email: &gmail.from,
            to: &d.email,
            subject: &subject,
            text: &text,
            unsubscribe_url: &unsubscribe_url,
        };
        let Some(raw) = gmail_raw(&mail) else {
            // An address with a line break never reaches a header; keep the
            // claim so it is not tried again.
            summary.refused += 1;
            continue;
        };
        match gmail.send(&raw).await {
            Ok(()) => {
                summary.sent += 1;
                if let Err(e) = db::sent(d1, &d.event.id, &d.email).await {
                    tracing::warn!(error = %e, "announce: sent mark failed (mail went)");
                }
            }
            Err(SendError::Refused(code)) => {
                summary.refused += 1;
                tracing::warn!(status = code, "announce: gmail refused a mail");
                // Token, permission or quota: the next run can send it. Any
                // other refusal is about this mail, which would come first
                // again every hour and crowd out the rest: keep it claimed.
                if matches!(code, 401 | 403 | 429) {
                    let _ = db::release(d1, &d.event.id, &d.email).await;
                }
            }
            Err(SendError::Ambiguous(why)) => {
                summary.ambiguous += 1;
                tracing::warn!(reason = %why, "announce: send ambiguous, kept claimed");
            }
        }
    }
    summary
}
