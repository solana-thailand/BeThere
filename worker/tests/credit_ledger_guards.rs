//! Regression guards for the credit ledger's money-critical SQL.
//!
//! The ledger's correctness lives in D1/SQLite statements that run in the wasm
//! worker runtime, not in host-unit-testable Rust. So these are **source-scan
//! guards** for the invariants that, if broken, fail SILENTLY in production
//! (compile stays green):
//!
//! 1. Every `ON CONFLICT (deposit_id, reason)` repeats the partial-index WHERE
//!    predicate. Without it SQLite errors at runtime ("ON CONFLICT clause does
//!    not match any PRIMARY KEY or UNIQUE constraint") — and for the hold path
//!    that means credit is never recorded (the exact 2026-08-14 loss signature).
//!    This bug compiled fine and was only caught running the backfill.
//! 2. The balance read stays org-scoped — dropping `organization_id` would let
//!    Org A's credit cover Org B's deposit (Issue #029 isolation).
//! 3. `try_spend` keeps its `balance >= amount` guard — the single-statement
//!    atomicity that prevents double-spend / negative balances.
//! 4. No caller guesses the org (`""`) instead of enumerating buckets.
//! 5. The payout reversal fails CLOSED — the flag clear must not run when the
//!    reversal could not be written, or the attendee has the cash *and* keeps
//!    spendable credit.
//! 6. `reconcile` checks both directions of the money, not just the loss one.
//! 7. The admin payout queue reads the amount from the LEDGER, not from the
//!    superseded `contacts.deposit_credit_*` cells.
//! 8. The attendee's refund REQUEST fails closed on the D1 write. That write is
//!    the only thing `credit_refund_requests` reads, so a swallowed failure —
//!    or a 0-row UPDATE, which is not an `Err` at all — reports "queued" to the
//!    attendee while no organizer ever sees the request.

use std::fs;
use std::path::Path;

fn ledger_src() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db/credit_ledger.rs");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn on_conflict_repeats_partial_index_predicate() {
    let src = ledger_src();
    let needle = "ON CONFLICT (deposit_id, reason)";
    let mut idx = 0;
    let mut count = 0;
    while let Some(pos) = src[idx..].find(needle) {
        let start = idx + pos;
        let tail = &src[start..(start + 120).min(src.len())];
        assert!(
            tail.contains("WHERE deposit_id IS NOT NULL"),
            "ON CONFLICT (deposit_id, reason) must repeat `WHERE deposit_id IS NOT NULL` \
             (partial unique index idx_credit_ledger_once). Without it SQLite errors at \
             runtime and credit is never recorded (silent loss). Near: {}",
            &tail[..tail.len().min(90)]
        );
        count += 1;
        idx = start + needle.len();
    }
    assert!(
        count >= 2,
        "expected ON CONFLICT in both record() and try_spend(), found {count}"
    );
}

#[test]
fn balance_read_is_org_scoped() {
    let src = ledger_src();
    assert!(
        src.contains("email = ?1 AND organization_id = ?2 AND currency = ?3"),
        "balance() must be scoped by (email, organization_id, currency) — dropping \
         organization_id would let one org's credit cover another org's deposit (Issue #029)"
    );
}

#[test]
fn try_spend_keeps_balance_guard() {
    let src = ledger_src();
    // The conditional insert must only fire when the current balance covers the
    // amount (the `... ) >= ?4` guard in the WHERE of the SELECT-INSERT).
    assert!(
        src.contains(") >= ?4"),
        "try_spend() must guard the insert on balance >= amount — without the \
         `>= ?4` guard credit could be over-spent into a negative balance"
    );
}

// ---------------------------------------------------------------------------
// 4. No caller guesses the organization (plan 022 §6)
// ---------------------------------------------------------------------------

/// The ledger is org-scoped, so a caller with no event context has to *enumerate*
/// the orgs an email holds credit in — never hard-code one. Two paths hang off
/// the contact rather than an event (the balance chip and the "return my held
/// credit" payout reversal) and both used to pass `""`. That reads as zero for
/// any event whose Org ID column is filled in: the chip under-reports, and the
/// reversal silently does nothing while the organizer pays the cash out, leaving
/// the attendee holding spendable credit as well.
///
/// `positive_balances` is the enumerating read. This guard fails if any call site
/// passes an empty-string org to the scoped helpers again.
#[test]
fn no_caller_hardcodes_the_default_org() {
    const SCOPED: [&str; 3] = [
        "credit_ledger::balance(",
        "credit_ledger::record(",
        "credit_ledger::try_spend(",
    ];

    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();

    visit_rs(&src_root, &mut |path, contents| {
        // The ledger module itself takes the org as a parameter; the guard is
        // about its *callers* choosing a value.
        if path.ends_with("credit_ledger.rs") {
            return;
        }
        let code = strip_comments(contents);
        for needle in SCOPED {
            let mut idx = 0;
            while let Some(pos) = code[idx..].find(needle) {
                let start = idx + pos;
                let args = &code[start..(start + 260).min(code.len())];
                // The org is the argument right after the email; an empty literal
                // anywhere in the argument list is the shape we are banning.
                if args.contains("\"\"") {
                    offenders.push(format!("{}: {needle}", path.display()));
                }
                idx = start + needle.len();
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "these call sites pass a hard-coded organization to the org-scoped credit \
         ledger. With no event context, enumerate with `positive_balances` instead \
         — guessing `\"\"` under-reports to zero for any event whose Org ID is set, \
         and on the payout-reversal path that means a double payout: {offenders:?}"
    );
}

#[test]
fn payout_reversal_fails_closed() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/handlers/deposit/thb/handlers/hold_refund_request.rs");
    let src = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let code = strip_comments(&src);

    // The reversal must propagate: `unwrap_or_default()` on the bucket read, or
    // a `match`/`if let Err` that only logs, silently clears the flag with the
    // credit still live.
    let helper = code
        .find("async fn reverse_held_credit")
        .map(|i| &code[i..])
        .expect("reverse_held_credit must own the reversal so the failure path is one place");
    let body = &helper[..helper.find("\n}\n").map_or(helper.len(), |i| i + 2)];

    assert!(
        !body.contains("unwrap_or_default") && !body.contains("unwrap_or("),
        "reverse_held_credit must not swallow a failed bucket read — an empty list \
         reverses nothing and the clear still runs (double payout)"
    );
    assert!(
        body.matches(".await?").count() >= 2,
        "both the bucket read and every record() write must use `?` so a failure \
         aborts the clear"
    );

    // And the handler must actually propagate that error before clearing.
    let handler = code
        .find("pub async fn clear_credit_refund_request_handler")
        .map(|i| &code[i..])
        .expect("clear handler must exist");
    let reversal_at = handler
        .find("reverse_held_credit(")
        .expect("the clear handler must call reverse_held_credit");
    let clear_at = handler
        .find("contacts::clear_credit_refund_requested(")
        .expect("the clear handler must clear the flag");
    assert!(
        reversal_at < clear_at,
        "the ledger reversal must run BEFORE the flag clear"
    );
    assert!(
        handler[reversal_at..clear_at].contains("map_err(AppError::Internal)?"),
        "a failed reversal must abort the request — clearing the flag anyway leaves \
         the attendee with the payout AND spendable credit, and nothing detects it"
    );
}

#[test]
fn reconcile_checks_both_money_directions() {
    let src = ledger_src();
    // Loss direction was always covered; the creation direction (credit that
    // exists and should not) was silent until plan 022 §6.
    for field in [
        "orphan_holds",
        "negative_balances",
        "double_settled",
        "phantom_holds",
    ] {
        assert!(
            src.contains(&format!("{field},")),
            "ReconcileReport must still report `{field}` — dropping a check makes that \
             money defect silent for a full day or forever"
        );
        assert!(
            src.contains(&format!("self.{field} == 0")),
            "is_clean() must include `{field}`; a check that never fires the alert is \
             the same as no check"
        );
    }
    assert!(
        src.contains("held_as_credit = 1 AND refunded = 1"),
        "double_settled must look for deposits settled BOTH ways (the CAS invariant)"
    );
    assert!(
        src.contains("l.reason = 'hold' AND l.deposit_id IS NOT NULL"),
        "phantom_holds must look for ledger credit with no held deposit behind it"
    );
    // Each field must be backed by a real query — stubbing one to a constant
    // keeps `is_clean()` honest-looking while the check no longer runs.
    assert!(
        src.matches("count_query(").count() >= 5,
        "each of the four reconcile checks needs its own count_query (plus the fn \
         definition); a field assigned a literal is a check that never fires"
    );
}

#[test]
fn payout_queue_reads_the_ledger_not_the_superseded_columns() {
    // The organizer reads this queue to decide how much cash to hand back, so
    // the number must be the one `reverse_held_credit` will remove. The
    // `contacts.deposit_credit_thb/usdc` cells are what the append-only ledger
    // replaced after the 2026-08-14 loss and nothing writes them any more, so
    // sourcing the queue from them renders 0 THB against a live balance — the
    // same defect that made the old liability chip always read zero.
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db/contacts.rs");
    let src = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let code = strip_comments(&src);

    let start = code
        .find("pub async fn credit_refund_requests")
        .expect("credit_refund_requests must exist");
    // Back up to the SQL literal that the function opens with.
    let body = &code[start
        ..code[start..]
            .find("\n}\n")
            .map_or(code.len(), |i| start + i)];

    assert!(
        body.contains("FROM credit_ledger"),
        "the payout queue's amounts must be summed from credit_ledger"
    );
    for banned in ["deposit_credit_thb", "deposit_credit_usdc"] {
        assert!(
            !body.contains(banned),
            "the payout queue must not read `contacts.{banned}` — nothing writes that \
             cell any more, so the organizer would be shown 0 against a live balance"
        );
    }
    // And no writer may resurrect the mutable cells anywhere in the module.
    assert!(
        !code.contains("SET deposit_credit_thb"),
        "`contacts.deposit_credit_*` is superseded by the append-only ledger; a \
         mutable-cell writer reintroduces the exact 2026-08-14 loss shape"
    );
}

fn refund_request_src() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/handlers/deposit/thb/handlers/hold_refund_request.rs");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Extract the body of `request_credit_refund_handler`, comments stripped.
fn request_handler_body() -> String {
    let code = strip_comments(&refund_request_src());
    let start = code
        .find("pub async fn request_credit_refund_handler")
        .expect("request_credit_refund_handler must exist");
    let end = code[start..]
        .find("\n}\n")
        .map_or(code.len(), |i| start + i);
    code[start..end].to_string()
}

#[test]
fn refund_request_d1_write_fails_closed() {
    let body = request_handler_body();

    // The failure the old code shipped: `if let Err(e) = ...set_credit_refund_requested`
    // followed by a warn and a 200. The organizer's queue reads D1 only, so that
    // is a silently dropped payout obligation.
    assert!(
        !body.contains("if let Err(e) = crate::db::contacts::set_credit_refund_requested"),
        "the D1 flag write must not be logged-and-continued — `credit_refund_requests` \
         reads that column alone, so swallowing the error tells the attendee their \
         refund is queued when no organizer will ever see it"
    );
    assert!(
        body.contains("crate::db::contacts::set_credit_refund_requested(db, &claims.email)")
            && body.contains(".map_err(AppError::Internal)?"),
        "the D1 flag write must propagate its error to the caller"
    );

    // A missing binding is the same outcome as a failed write, and must not
    // silently fall through to the display-only Sheets mirror.
    assert!(
        body.contains("state.d1.as_deref().ok_or_else("),
        "a missing D1 binding must fail closed, not fall back to a Sheets-only write"
    );
}

#[test]
fn refund_request_rejects_zero_row_update() {
    // An UPDATE matching no contact row succeeds with `changes == 0`. Without a
    // rows-affected check the handler cannot see it, and the request is dropped
    // exactly as silently as a swallowed error would drop it.
    let db = strip_comments(&{
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db/contacts.rs");
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    });
    let start = db
        .find("pub(crate) async fn set_credit_refund_requested")
        .expect("set_credit_refund_requested must exist");
    let end = db[start..].find("\n}\n").map_or(db.len(), |i| start + i);
    let setter = &db[start..end];

    assert!(
        setter.contains("-> Result<bool, String>"),
        "set_credit_refund_requested must report whether a row was flagged"
    );
    assert!(
        setter.contains(".and_then(|m| m.changes)") && setter.contains("Ok(changes > 0)"),
        "the rows-affected signal must come from D1 meta().changes, not be assumed"
    );

    let body = request_handler_body();
    assert!(
        body.contains("if !flagged") && body.contains("return Err(AppError::Internal("),
        "the handler must reject a 0-row UPDATE instead of reporting `requested: true`"
    );
}

#[test]
fn refund_request_sheets_mirror_is_non_fatal() {
    let body = request_handler_body();

    // Polarity: authoritative write first and fatal, display-only mirror second
    // and best-effort. The inverse (what shipped) 500s on a stale mirror while
    // letting the real record vanish.
    let d1_at = body
        .find("set_credit_refund_requested(db, &claims.email)")
        .expect("D1 write must exist");
    let sheets_at = body
        .find("crate::sheets::contacts::set_credit_refund_requested")
        .expect("Sheets mirror must exist");
    assert!(
        d1_at < sheets_at,
        "the authoritative D1 write must run before the display-only Sheets mirror"
    );

    let mirror = &body[sheets_at..];
    assert!(
        !mirror.contains(".map_err(AppError::Internal)?"),
        "the Sheets mirror is display-only and must not fail the request — a 500 here \
         reports failure for a request that is already in the organizer's queue"
    );
    assert!(
        body.contains("if let Err(e) = crate::sheets::contacts::set_credit_refund_requested"),
        "the Sheets mirror failure must be caught and logged"
    );

    // Neither missing binding may gate the authoritative write either.
    let kv_gate = body
        .find("state.events_kv.as_ref()")
        .expect("KV lookup must exist");
    assert!(
        kv_gate > d1_at,
        "a missing EVENTS KV binding must not block the D1 write that queues the payout"
    );
    assert!(
        !body[..d1_at].contains("sheet_id.is_empty()"),
        "an unconfigured contacts sheet must not block the D1 write that queues the payout"
    );
}

/// Strip `//`-prefixed lines so prose about the banned shape cannot trip (or
/// satisfy) a rule about code.
fn strip_comments(source: &str) -> String {
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn visit_rs(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    for entry in fs::read_dir(dir)
        .expect("worker/src must be readable")
        .flatten()
    {
        let path = entry.path();
        match path.is_dir() {
            true => visit_rs(&path, f),
            false if path.extension().is_some_and(|e| e == "rs") => {
                let contents = fs::read_to_string(&path).expect("source file must be readable");
                f(&path, &contents);
            }
            false => {}
        }
    }
}
