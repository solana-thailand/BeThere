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

/// Position of the first ledger sum, bare or aliased (`SUM(l.delta)` since the
/// person-aware readers join `person_emails`).
fn first_sum(body: &str) -> Option<usize> {
    ["SUM(delta)", "SUM(l.delta)"]
        .iter()
        .filter_map(|needle| body.find(needle))
        .min()
}

#[test]
fn every_balance_read_releases_ended_applies_first() {
    let ledger = read("src/db/credit_ledger.rs");
    let readers: Vec<_> = pub_async_fns(&ledger)
        .into_iter()
        .filter(|(name, body)| !name.starts_with("release_") && first_sum(body).is_some())
        .collect();
    assert!(
        readers.len() >= 5,
        "expected the ledger's balance readers, found {:?}",
        readers.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    for (name, body) in readers {
        let global = body.find("release_ended_applies(db).await?");
        let person = body.find("release_person_ended_applies(db, &email_lc).await?");
        let release = global.or(person).unwrap_or_else(|| {
            panic!("{name} reads a balance without releasing ended events first")
        });
        let sum = first_sum(&body).expect("filtered on a ledger sum");
        assert!(release < sum, "{name} must release before it sums");
        // A scoped release only covers the person it was given, so the read
        // must be over that same person (`?1` bound to `email_lc`).
        if person.is_some() {
            assert!(
                body.contains("person_emails_of!(\"?1\")")
                    || body.contains("unreturned_apply_of!(\"?1\")"),
                "{name} releases one person but does not read that person"
            );
            assert!(
                body.contains("D1Type::Text(&email_lc)"),
                "{name} must bind the same email it released"
            );
        }
    }
}

/// The release SQL template: the body of `release_ended_applies_sql!`.
fn release_template(ledger: &str) -> &str {
    let start = ledger
        .find("macro_rules! release_ended_applies_sql")
        .expect("release SQL is one shared template");
    &ledger[start..start + ledger[start..].find("\n}\n").expect("macro ends")]
}

#[test]
fn release_sql_returns_ended_applies_once_regardless_of_attendance() {
    let ledger = read("src/db/credit_ledger.rs");
    let sql = release_template(&ledger);
    for needle in [
        "-a.delta, 'return'",
        "'return:' || a.event_id || ':' || a.email",
        "a.delta < 0",
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
    // Both releases come from the template and pick only `apply` rows. The
    // person one keeps the unary `+` (.issues/163): without it the planner walks
    // every apply row on the reason index and the cost follows the whole ledger.
    assert!(
        ledger.contains(
            "RELEASE_ENDED_APPLIES_SQL: &str = release_ended_applies_sql!(\"a.reason = 'apply'\");"
        ),
        "global release must come from the template"
    );
    assert!(
        ledger.contains(
            "RELEASE_PERSON_ENDED_APPLIES_SQL: &str = release_ended_applies_sql!(\n    \"+a.reason = 'apply' AND a.email IN \",\n    person_emails_of!(\"?1\")\n);"
        ),
        "person release must come from the template, scoped to person_emails_of!(\"?1\")"
    );
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
        coverage.contains(".prepare(crate::db::credit_ledger::RELEASE_PERSON_ENDED_APPLIES_SQL)")
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

/// Plan 028 W5: a page that needs both currencies reads them with one
/// `balances` call. Two `balance` calls ran the release write twice per request.
#[test]
fn both_currency_readers_release_once() {
    for file in [
        "src/handlers/register/signup.rs",
        "src/handlers/deposit/thb/handlers/hold_credit.rs",
    ] {
        let code = read(file);
        assert!(
            code.contains("credit_ledger::balances("),
            "{file} must read both currencies with one `balances` call"
        );
        assert!(
            !code.contains("credit_ledger::balance("),
            "{file} reads one currency at a time again: each `balance` call reruns the release write"
        );
    }
}
