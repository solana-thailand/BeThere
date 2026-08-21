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
