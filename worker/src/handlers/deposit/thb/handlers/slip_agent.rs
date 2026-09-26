//! The slip agent's worker half (`.plans/033` W1), in shadow mode.
//!
//! Takes the facts read off a slip, runs the deterministic checker in
//! `domain::slip_proposal`, and stores the proposal for the admin screen.
//! Nothing here can fail an upload or touch a deposit: every error is logged
//! and swallowed, because in shadow mode the organizer's decision is the only
//! one that counts and a bug in this file must not stand between an attendee
//! and their ticket.

use chrono::{DateTime, Utc};
use event_checkin_domain::models::event::EventConfig;
use event_checkin_domain::slip_proposal::{
    Expectations, FactSource, SlipFacts, SlipProposal, Verdict, evaluate,
};
use event_checkin_domain::slip_verify::parse_slip_verify;
use worker::D1Database;

use crate::db::slip_proposals::{self, SaveError};

/// Facts from a mini-QR payload the browser decoded.
///
/// The payload is the client's claim; it is re-parsed here, CRC included, so
/// the stored reference is one the server read itself. A payload that does not
/// parse yields `None` and a log line: an edited QR (bad checksum) is worth
/// seeing in the logs, but it proves nothing about who edited it.
pub(crate) fn facts_from_qr(payload: &str, event_id: &str, attendee_id: &str) -> Option<SlipFacts> {
    match parse_slip_verify(payload.trim()) {
        Ok(reference) => Some(SlipFacts {
            bank_ref: Some(reference.storage_key()),
            ..SlipFacts::default()
        }),
        Err(e) => {
            tracing::warn!(
                event_id = %event_id,
                attendee_id = %attendee_id,
                reason = %e,
                "slip agent: client-decoded QR did not parse"
            );
            None
        }
    }
}

/// Everything [`record`] needs besides the database.
pub(crate) struct ProposalInput<'a> {
    pub event: &'a EventConfig,
    pub attendee_id: &'a str,
    /// The attendee's registration time (RFC 3339), the start of the window.
    pub registered_at: Option<&'a str>,
    pub source: FactSource,
    pub model: Option<String>,
    pub facts: SlipFacts,
}

/// Evaluate and store one proposal. Returns the stored verdict, or `None` if
/// nothing was stored.
pub(crate) async fn record(db: &D1Database, input: ProposalInput<'_>) -> Option<Verdict> {
    let ProposalInput {
        event,
        attendee_id,
        registered_at,
        source,
        model,
        facts,
    } = input;
    let now = Utc::now();

    let claimed_elsewhere = match &facts.bank_ref {
        None => false,
        Some(claimed) => {
            match slip_proposals::ref_claimed_elsewhere(db, claimed, &event.id, attendee_id).await {
                Ok(claimed) => claimed,
                Err(e) => {
                    tracing::warn!(event_id = %event.id, error = %e, "slip agent: ref lookup failed");
                    return None;
                }
            }
        }
    };

    let build = |claimed_elsewhere: bool| {
        let expect = Expectations {
            deposit_amount_thb: event.deposit_amount_thb,
            window_start: registered_at
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.with_timezone(&Utc)),
            window_end: now,
            ref_claimed_elsewhere: claimed_elsewhere,
            promptpay_id: event.promptpay_id.clone(),
        };
        SlipProposal {
            source,
            model: model.clone(),
            evaluation: evaluate(&facts, &expect),
            facts: facts.clone(),
            created_at: now.to_rfc3339(),
        }
    };

    let owns_ref = facts.bank_ref.is_some() && !claimed_elsewhere;
    let proposal = build(claimed_elsewhere);
    let saved = match slip_proposals::upsert(db, &event.id, attendee_id, &proposal, owns_ref).await
    {
        // Another upload claimed the same reference between the lookup and the
        // insert. The schema caught it; re-evaluate as a reuse and keep the
        // evidence without the UNIQUE column.
        Err(SaveError::RefTaken) => {
            let reused = build(true);
            slip_proposals::upsert(db, &event.id, attendee_id, &reused, false)
                .await
                .map(|()| reused)
        }
        other => other.map(|()| proposal),
    };

    match saved {
        Ok(stored) => {
            tracing::info!(
                event_id = %event.id,
                attendee_id = %attendee_id,
                source = stored.source.as_str(),
                verdict = stored.evaluation.verdict.as_str(),
                "slip agent: proposal recorded (shadow mode)"
            );
            Some(stored.evaluation.verdict)
        }
        Err(e) => {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %attendee_id,
                error = ?e,
                "slip agent: proposal not stored (non-fatal)"
            );
            None
        }
    }
}

/// The shadow-mode hook both upload handlers call once the deposit is saved.
///
/// The QR is read in the browser and re-parsed here; only a slip without a
/// usable one goes to vision, only when vision is switched on, and only for an
/// uploaded image (a pasted URL is someone else's host and is not fetched).
/// Returns nothing: in shadow mode no outcome of this may reach the caller.
pub(crate) async fn propose_after_upload(
    state: &crate::state::AppState,
    event: &EventConfig,
    attendee_id: &str,
    registered_at: Option<&str>,
    slip_qr: Option<&str>,
    slip_url: &str,
) {
    let qr_facts = slip_qr.and_then(|payload| facts_from_qr(payload, &event.id, attendee_id));
    match (qr_facts, state.d1.as_deref()) {
        (Some(facts), Some(db)) => {
            record(
                db,
                ProposalInput {
                    event,
                    attendee_id,
                    registered_at,
                    source: FactSource::Qr,
                    model: None,
                    facts,
                },
            )
            .await;
        }
        (Some(_), None) => {}
        (None, _) => {
            let Some(api_key) = state.slip_vision_key.clone() else {
                return;
            };
            if !slip_url.starts_with("data:image/") {
                return;
            }
            let vision = propose_from_vision(
                state.clone(),
                api_key,
                event.clone(),
                attendee_id.to_string(),
                registered_at.map(str::to_string),
                slip_url.to_string(),
            );
            match &state.worker_ctx {
                Some(ctx) => ctx.wait_until(vision),
                None => vision.await,
            }
        }
    }
}

/// The vision fallback: read the slip image with the model, then record the
/// proposal exactly as the QR path does. Owned arguments so it can run under
/// `wait_until` after the upload has already answered the attendee.
pub(crate) async fn propose_from_vision(
    state: crate::state::AppState,
    api_key: String,
    event: EventConfig,
    attendee_id: String,
    registered_at: Option<String>,
    data_url: String,
) {
    let Some(db) = state.d1.as_deref() else {
        return;
    };
    let facts = match crate::slip_vision::read_slip(&api_key, &data_url).await {
        Ok(facts) => facts,
        Err(e) => {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %attendee_id,
                error = %e,
                "slip agent: vision read failed (non-fatal)"
            );
            return;
        }
    };
    record(
        db,
        ProposalInput {
            event: &event,
            attendee_id: &attendee_id,
            registered_at: registered_at.as_deref(),
            source: FactSource::Vision,
            model: Some(crate::slip_vision::MODEL.to_string()),
            facts,
        },
    )
    .await;
}
