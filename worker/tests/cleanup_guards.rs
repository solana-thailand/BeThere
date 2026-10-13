//! Regression guards for the nightly cleanup cron.
//!
//! The cron runs unattended at 03:00 UTC, and what it deletes cannot be
//! recovered from production. Every invariant below fails SILENTLY if broken:
//! the compile stays green, the cron reports success, and the loss shows months
//! later when someone asks what happened to a deposit.
//!
//! 1. It never deletes payment rows from D1 (owner 2026-10-13: payment history
//!    is the audit record). It used to, 90 days after the refund window: RTM#3's
//!    14 deposits on 2026-09-19 (`.issues/126`), and held credit lost its
//!    deposit and dropped out of the admin payout lists.
//! 2. The PII-free archive it still writes keeps no personal data.
//! 3. The archive is keyed on the deposit ROW, not (event_id, attendee_id),
//!    which `thb_deposits` does not enforce (`.issues/127`).

use std::fs;
use std::path::Path;

fn src(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Strip `//` comments so a guard cannot be satisfied by prose about the thing
/// it is guarding.
fn strip_comments(code: &str) -> String {
    code.lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Payment history is the audit record of every past event (owner 2026-10-13):
/// the nightly cleanup must not delete D1 deposit rows or deposit statuses.
/// It did for months (90 days after the refund window) and, on 2026-09-19, took
/// RTM#3's 14 deposits; two people's held credit was left with no deposit
/// behind it and dropped out of every admin payout list.
#[test]
fn cleanup_never_deletes_payment_rows() {
    let code = strip_comments(&src("src/cleanup.rs"));
    for gone in [
        "delete_thb_deposits_for_event",
        "delete_deposit_statuses_for_event",
        "slip_proposals::delete_for_event",
        "DELETE FROM thb_deposits",
        "DELETE FROM deposit_statuses",
        "DELETE FROM credit_ledger",
    ] {
        assert!(
            !code.contains(gone),
            "cleanup.rs must not delete payment rows: found `{gone}`"
        );
    }
    // the event-wide deletes are gone from the db layer too, so nothing can
    // quietly bring the purge back by calling them
    assert!(!src("src/db/thb_deposits.rs").contains("fn delete_thb_deposits_for_event("));
    assert!(!src("src/db/deposit_statuses.rs").contains("fn delete_deposit_statuses_for_event("));
}

#[test]
fn deposit_archive_keeps_no_personal_data() {
    // The delete exists to remove these. Copying one into the archive first
    // would turn a purge into a move.
    let sql = src("migrations/0044_thb_deposit_archive.sql");
    let create = &sql[sql
        .find("CREATE TABLE IF NOT EXISTS thb_deposit_archive")
        .expect("the archive table must exist")..];
    let create = &create[..create.find(");").expect("CREATE TABLE ends")];

    for column in [
        "attendee_name",
        "bank_account",
        "bank_name",
        "account_name",
        "slip_url",
        "refund_proof_url",
        "verified_by",
    ] {
        assert!(
            !create.contains(column),
            "`{column}` is personal data the 90-day purge exists to delete — it must \
             not survive in the archive. Keep the money, drop the people."
        );
    }

    // And the amounts must actually be there, or the archive is decorative.
    for column in ["amount_thb", "refunded", "held_as_credit"] {
        assert!(
            create.contains(column),
            "the archive must keep `{column}` — without it the deposits cannot be \
             reconciled after the purge"
        );
    }
}

#[test]
fn archive_is_keyed_on_the_deposit_row_not_the_attendee() {
    let code = src("src/db/thb_deposits.rs");
    let f = &code[code
        .find("pub async fn archive_thb_deposits_for_event(")
        .expect("archive fn exists")..];
    let body = &f[..f.find("\n}\n").expect("fn ends")];

    assert!(
        body.contains("ON CONFLICT (source_deposit_id) DO NOTHING"),
        "the archive insert must be idempotent PER DEPOSIT ROW. `thb_deposits` has \
         no UNIQUE on (event_id, attendee_id) — 0013 makes it a plain INDEX — and \
         save_thb_deposit is check-then-act, so one attendee can hold two rows. \
         Keying on the pair archives one and deletes both: 700 THB in the \
         reproduction (.issues/127). The money is per row, so the key is per row."
    );
    assert!(
        !body.contains("ON CONFLICT (event_id, attendee_id)"),
        "the (event_id, attendee_id) key is the 0044 defect — it must not come back"
    );
}

#[test]
fn cron_alerts_when_cleanup_did_not_complete() {
    let code = strip_comments(&src("src/lib.rs"));

    let call_at = code
        .find("run_cleanup(")
        .expect("the daily cron still runs the cleanup pass");
    let next_reconcile = code[call_at..]
        .find("reconcile(")
        .map(|i| call_at + i)
        .unwrap_or(code.len());
    let window = &code[call_at..next_reconcile];

    assert!(
        window.contains("is_clean()"),
        "the cron must inspect the cleanup summary — discarding it is what made \
         four nights of aborted runs indistinguishable from four clean ones"
    );
    assert!(
        window.contains("post_slack"),
        "a failed cleanup must alert like the other nightly reconciles, not just log"
    );
}

/// 6. The alert payload must stay free of raw backend error text.
///
/// `post_slack` does not redact, and a D1/KV error string can carry a bound
/// value straight out of the isolate. `CleanupFailure` is a closed enum for that
/// reason; a `String` variant would quietly hand the detail to an external
/// webhook.
#[test]
fn cleanup_failures_carry_no_free_form_error_text() {
    let code = strip_comments(&src("src/cleanup.rs"));

    let start = code
        .find("pub enum CleanupFailure")
        .expect("CleanupFailure must exist");
    let end = start
        + code[start..]
            .find("\n}")
            .expect("CleanupFailure must be a closed enum");
    let variants = &code[start..end];

    for banned in ["error", "err", "message", "detail", "reason"] {
        assert!(
            !variants.to_lowercase().contains(banned),
            "CleanupFailure carries a `{banned}` field — raw backend error text \
             reaches Slack unredacted; keep the detail in tracing"
        );
    }
}

/// 4. A cleanup phase that fails must be REPORTABLE, not merely logged.
///
/// Every abort path in `run_cleanup` used to write to `tracing` and return a
/// summary that looked identical to a healthy one. On staging a malformed index
/// entry aborted the whole run every night from 2026-09-19 and nobody knew for
/// four days — the cron reported success the entire time. A new error site that
/// forgets to record its failure silently re-opens exactly that hole, so the
/// count is pinned rather than spot-checked.
#[test]
fn every_cleanup_error_site_records_a_failure() {
    let code = strip_comments(&src("src/cleanup.rs"));

    // Whitespace-free, because rustfmt breaks a long push across three lines
    // and a literal `failures.push(` search would then miss it — a guard that
    // silently stops matching is worse than no guard.
    let dense: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let logged = dense.matches("tracing::error!").count();
    let recorded = dense.matches(".failures.push(").count();

    assert!(
        logged > 0,
        "cleanup should still log its failures — the guard reads the wrong file otherwise"
    );
    assert_eq!(
        logged, recorded,
        "{logged} error site(s) in cleanup.rs but {recorded} recorded failure(s): an \
         unrecorded abort is invisible to the nightly alert, which is how the \
         2026-09-19 staging outage lasted four days"
    );
}
