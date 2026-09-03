//! Guard: the THB settlement columns are owned by the CAS functions.
//!
//! `thb_deposits` has five columns that record an irreversible money event:
//! `refunded` / `refunded_at` / `refund_proof_url` (cash paid out) and
//! `held_as_credit` / `held_as_credit_at` (converted to rolling credit). They
//! are mutually exclusive, and each is flipped 0→1 by a conditional UPDATE that
//! is the whole concurrency story: `try_settle_refund` requires
//! `verified = 1 AND refunded = 0 AND held_as_credit = 0`, and
//! `try_settle_hold_credit` mirrors it. Whichever lands first wins; the loser
//! matches 0 rows. That is what prevents a double payout.
//!
//! `update_thb_deposit` is a blanket read-modify-write of the row, reached from
//! `save_thb_deposit` by seven handlers. It used to include all five settlement
//! columns, which silently reopened the hole the CAS closes: a caller holding a
//! record it read earlier — a slip re-verify, a slip upload whose existence
//! check consults the *other* store (`deposit_status`), the rolling-credit
//! auto-apply that constructs a fresh row with `refunded: false` — would write
//! `refunded = 0` back over a refund that settled in between. The cash is gone,
//! the row says otherwise, and `try_settle_refund` will happily settle it again.
//!
//! Every caller that sets a settlement field in memory does so *after* its own
//! CAS has already written D1, so removing the columns from the blanket UPDATE
//! changes nothing on the intended paths. These tests fail if they come back.

const THB_DB: &str = include_str!("../src/db/thb_deposits.rs");

/// Source with `//`, `///` and `//!` lines stripped, so a rule that talks about
/// code is never satisfied (or broken) by prose describing it.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of one `pub async fn`, up to the next top-level `pub` item.
fn function_body<'a>(code: &'a str, name: &str) -> &'a str {
    let needle = format!("pub async fn {name}");
    let start = code
        .find(&needle)
        .unwrap_or_else(|| panic!("db/thb_deposits.rs no longer defines `{name}`"));
    let rest = &code[start + needle.len()..];
    match rest.find("\npub ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

const SETTLEMENT_COLUMNS: [&str; 5] = [
    "refunded",
    "refunded_at",
    "held_as_credit",
    "held_as_credit_at",
    "refund_proof_url",
];

#[test]
fn the_blanket_update_writes_no_settlement_column() {
    let code = code_only(THB_DB);
    let body = function_body(&code, "update_thb_deposit");

    for column in SETTLEMENT_COLUMNS {
        assert!(
            !body.contains(&format!("{column} = ?")),
            "`update_thb_deposit` writes `{column}`. That column is settled by \
             `try_settle_refund` / `try_settle_hold_credit`; a blanket \
             read-modify-write can retract a settlement that landed after the \
             caller read the row, which re-arms the refund CAS for a second \
             payout on cash already sent."
        );
        assert!(
            !body.contains(&format!("deposit.{column}")),
            "`update_thb_deposit` binds `deposit.{column}` — the settlement \
             state must come from the CAS, never from a caller's in-memory copy"
        );
    }
}

#[test]
fn the_refund_cas_still_carries_its_preconditions() {
    let code = code_only(THB_DB);
    let body = function_body(&code, "try_settle_refund");

    assert!(
        body.contains("SET refunded = 1"),
        "`try_settle_refund` must be the writer that flips `refunded` 0→1"
    );
    for precondition in ["verified = 1", "refunded = 0", "held_as_credit = 0"] {
        assert!(
            body.contains(precondition),
            "`try_settle_refund` dropped its `{precondition}` precondition — \
             that clause is what makes a double payout impossible"
        );
    }
}

#[test]
fn the_hold_cas_still_carries_its_preconditions() {
    let code = code_only(THB_DB);
    let body = function_body(&code, "try_settle_hold_credit");

    assert!(
        body.contains("SET held_as_credit = 1"),
        "`try_settle_hold_credit` must be the writer that flips \
         `held_as_credit` 0→1"
    );
    for precondition in ["refunded = 0", "held_as_credit = 0"] {
        assert!(
            body.contains(precondition),
            "`try_settle_hold_credit` dropped its `{precondition}` \
             precondition — refund and hold must stay mutually exclusive"
        );
    }
}

#[test]
fn the_proof_url_setter_touches_nothing_else() {
    let code = code_only(THB_DB);
    let body = function_body(&code, "set_refund_proof_url");

    assert!(
        body.contains("SET refund_proof_url = ?1"),
        "`set_refund_proof_url` must update exactly the proof URL"
    );
    // It exists only so the data:-URL → R2 migration can rewrite the *same*
    // proof compactly. If it grew a second column it would become another
    // blanket writer, which is the shape this whole guard exists to forbid.
    for column in ["refunded", "held_as_credit", "verified"] {
        assert!(
            !body.contains(&format!("{column} = ")),
            "`set_refund_proof_url` also writes `{column}` — keep it to the one \
             column so it can never retract a settlement"
        );
    }
}

#[test]
fn only_the_cas_pair_and_the_proof_setter_write_settlement_columns() {
    let code = code_only(THB_DB);

    // Any `SET <settlement column> = ` in this file must sit inside one of the
    // three functions licensed to write it. A fourth writer is the regression.
    let licensed = [
        function_body(&code, "try_settle_refund"),
        function_body(&code, "try_settle_hold_credit"),
        function_body(&code, "set_refund_proof_url"),
        // Insert writes the whole row for a record that does not exist yet, so
        // there is no settlement to retract.
        function_body(&code, "insert_thb_deposit"),
    ];

    for column in SETTLEMENT_COLUMNS {
        let pattern = format!("{column} = ");
        let total = code.matches(&pattern).count();
        let accounted: usize = licensed.iter().map(|b| b.matches(&pattern).count()).sum();
        assert_eq!(
            total, accounted,
            "db/thb_deposits.rs writes `{column}` outside \
             `try_settle_refund` / `try_settle_hold_credit` / \
             `set_refund_proof_url` / `insert_thb_deposit` — every settlement \
             write must go through the CAS pair"
        );
    }
}
