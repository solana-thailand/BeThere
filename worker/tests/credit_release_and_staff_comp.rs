//! Guards for two attendee-journey rules fixed on 2026-09-17.
//!
//! **Credit is not forfeited by a no-show.** Rolling credit is the attendee's
//! cash the organizer still holds; it stays theirs until it is paid back. An
//! `apply` locks it for one event, and `release_ended_applies` returns it once
//! that event ends, attended or not. The old rule only returned it on check-in,
//! a best-effort write, and two people lost ฿500 (one of them *had* checked
//! in). The release SQL was run against a copy of the prod backup: exactly
//! those two balances moved, and a second run changed nothing.
//!
//! **Staff comps write the deposit status.** The comp wrote only a ฿0
//! `thb_deposits` row, while every page that routes deposit-vs-ticket reads
//! `deposit_statuses`, so an organizer was sent back to the deposit page on
//! every visit after registering.
//!
//! Source-scan guards: all of this is SQL and wasm-only code that fails
//! silently (wrong balance, wrong page), not with an error.

use std::{fs, path::Path};

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

/// `(name, body)` for every `pub async fn` in `source`, body up to the next one.
fn pub_async_fns(source: &str) -> Vec<(String, String)> {
    let marker = "pub async fn ";
    let starts: Vec<usize> = source.match_indices(marker).map(|(i, _)| i).collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &start)| {
            let end = starts.get(n + 1).copied().unwrap_or(source.len());
            let body = &source[start..end];
            let name = body[marker.len()..]
                .split(['(', '<'])
                .next()
                .unwrap_or_default()
                .to_string();
            (name, body.to_string())
        })
        .collect()
}

#[test]
fn every_balance_read_releases_ended_applies_first() {
    let ledger = read("src/db/credit_ledger.rs");
    let readers: Vec<_> = pub_async_fns(&ledger)
        .into_iter()
        .filter(|(name, body)| name != "release_ended_applies" && body.contains("SUM(delta)"))
        .collect();
    assert!(
        readers.len() >= 5,
        "expected the ledger's balance readers, found {:?}",
        readers.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    for (name, body) in readers {
        let release = body
            .find("release_ended_applies(db).await?")
            .unwrap_or_else(|| {
                panic!("{name} reads a balance without releasing ended events first")
            });
        let sum = body.find("SUM(delta)").expect("filtered on SUM(delta)");
        assert!(release < sum, "{name} must release before it sums");
    }
}

#[test]
fn release_sql_returns_ended_applies_once_regardless_of_attendance() {
    let ledger = read("src/db/credit_ledger.rs");
    let start = ledger
        .find("RELEASE_ENDED_APPLIES_SQL: &str = ")
        .expect("release SQL is a shared constant");
    let sql = &ledger[start..start + ledger[start..].find("\";").expect("constant ends")];
    for needle in [
        "a.reason = 'apply'",
        "-a.delta, 'return'",
        "'return:' || a.event_id || ':' || a.email",
        "e.event_end_ms > 0",
        "e.event_end_ms <= CAST(strftime('%s', 'now') AS INTEGER) * 1000",
        "ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING",
    ] {
        assert!(sql.contains(needle), "release SQL: missing `{needle}`");
    }
    for forbidden in ["checked_in_at", "attendees"] {
        assert!(
            !sql.contains(forbidden),
            "release must not depend on attendance (`{forbidden}`)"
        );
    }
    // The check-in writer must use the same key, or the two double-return.
    let checkin = read("src/handlers/checkin.rs");
    assert!(
        checkin.contains("format!(\"return:{}:{}\", event.id, email)"),
        "check-in return key must match the release key"
    );
}

#[test]
fn atomic_apply_releases_inside_its_batch() {
    let coverage = read("src/db/credit_coverage.rs");
    assert!(
        coverage.contains("db.prepare(crate::db::credit_ledger::RELEASE_ENDED_APPLIES_SQL)")
            && coverage.contains("vec![release, spend]"),
        "the spend guard must see released credit in the same transaction"
    );
    assert!(
        coverage.contains(".get(1)") && !coverage.contains("results\n        .first()"),
        "newly_spent must read the spend's result, not the release's"
    );
}

#[test]
fn payout_queue_releases_before_showing_amounts() {
    let contacts = read("src/db/contacts.rs");
    let body = &contacts[contacts
        .find("pub async fn credit_refund_requests(")
        .expect("payout queue exists")..];
    let release = body
        .find("release_ended_applies(db)")
        .expect("payout queue must release first, matching the reversal");
    assert!(release < body.find("SUM(l.delta)").expect("queue sums the ledger"));
}

#[test]
fn undo_checkin_cannot_take_back_released_credit() {
    let ledger = read("src/db/credit_ledger.rs");
    let body = &ledger[ledger.find("pub async fn remove_return(").expect("exists")..];
    let body = &body[..body.find("\n}\n").expect("fn ends")];
    assert!(
        body.contains("AND NOT EXISTS (SELECT 1 FROM events e WHERE e.id = ?1")
            && body.contains("e.event_end_ms <= CAST(strftime('%s', 'now') AS INTEGER) * 1000"),
        "remove_return must be a no-op once the event has ended"
    );
    assert!(
        !ledger.contains("forfeiting it"),
        "the no-show-forfeits rule is gone; do not document it"
    );
}

#[test]
fn staff_comp_writes_the_status_the_router_reads() {
    let signup = read("src/handlers/register/signup.rs");
    let body = &signup[signup
        .find("async fn record_staff_comp(")
        .expect("one comp writer")..];
    let body = &body[..body.find("\n}\n").expect("fn ends")];
    for needle in [
        "slip_url: Some(\"STAFF_COMP_WAIVED\".to_string())",
        "save_thb_deposit(",
        "save_deposit_status_with_fallback(",
        "method: DepositMethod::Thb",
        "amount: 0",
        "verified: true",
        "refundable: false",
    ] {
        assert!(
            body.contains(needle),
            "record_staff_comp: missing `{needle}`"
        );
    }
    assert_eq!(
        signup.matches("record_staff_comp(&state").count(),
        2,
        "new registrations and the re-entry repair must both use the one writer"
    );
    assert_eq!(
        signup.matches("\"STAFF_COMP_WAIVED\"").count(),
        1,
        "no second, status-less comp writer"
    );
    let migration = read("migrations/0042_staff_comp_deposit_status.sql");
    assert!(
        migration.contains("WHERE d.slip_url = 'STAFF_COMP_WAIVED'")
            && migration.contains("ON CONFLICT (event_id, attendee_id) DO NOTHING"),
        "existing comps are backfilled without overwriting a real status"
    );
}
