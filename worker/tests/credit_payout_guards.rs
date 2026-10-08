//! Source guards for the held-credit payout (`.issues/190`).
//!
//! The SQL itself is exercised against the real migration chain in
//! `tests/security/test_credit_payout.py`; these pin the Rust around it, the
//! parts that fail silently if they drift:
//!
//! 1. `refund` rows have ONE writer, the guarded `try_refund`. An unguarded
//!    `record(-balance, "refund")` racing a registration spend drove the
//!    balance negative (defect 2).
//! 2. The clear needs the organizer's confirmed amount and compares it inside
//!    the write (defect 1), audits who paid, and only then clears.
//! 3. Both payout endpoints are org-scoped and refuse per-event scanners.
//! 4. The payout account dies with the request: at payout, in the purge, on
//!    PDPA erasure — and never reaches the log stream.

use std::fs;
use std::path::Path;

fn src(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn strip_comments(code: &str) -> String {
    code.lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The text of `fn_sig`'s body up to the first column-0 closing brace.
fn fn_body<'a>(code: &'a str, fn_sig: &str) -> &'a str {
    let start = code
        .find(fn_sig)
        .unwrap_or_else(|| panic!("{fn_sig} must exist"));
    let end = code[start..]
        .find("\n}\n")
        .map_or(code.len(), |i| start + i);
    &code[start..end]
}

fn visit_rs(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    for entry in fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => visit_rs(&path, f),
            false if path.extension().is_some_and(|e| e == "rs") => {
                let text = fs::read_to_string(&path).expect("read rs");
                f(&path, &text);
            }
            false => {}
        }
    }
}

const HANDLER: &str = "src/handlers/deposit/thb/handlers/hold_refund_request.rs";

// ---------------------------------------------------------------------------
// 1. One guarded writer of `refund` rows
// ---------------------------------------------------------------------------

#[test]
fn refund_rows_have_one_guarded_writer() {
    let mut offenders = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    visit_rs(&root, &mut |path, text| {
        let code = strip_comments(text);
        let is_ledger = path.ends_with("db/credit_ledger.rs");
        // Every INSERT into the ledger that writes the literal reason.
        for (at, _) in code.match_indices("INSERT INTO credit_ledger") {
            let stmt = &code[at..(at + 700).min(code.len())];
            let stmt = &stmt[..stmt.find("\";").unwrap_or(stmt.len())];
            if stmt.contains("'refund'") && !is_ledger {
                offenders.push(format!("{}: SQL refund insert", path.display()));
            }
        }
        if !is_ledger && code.contains("REASON_REFUND") {
            offenders.push(format!(
                "{}: REASON_REFUND outside the ledger",
                path.display()
            ));
        }
    });
    assert!(
        offenders.is_empty(),
        "refund rows must be written by credit_ledger::try_refund only: {offenders:?}"
    );

    let ledger = strip_comments(&src("src/db/credit_ledger.rs"));
    let record = fn_body(&ledger, "pub async fn record(");
    assert!(
        record.contains("if reason == REASON_REFUND"),
        "record() must refuse refund rows, so no future caller can write one unguarded"
    );
    let inserts = ledger.matches("'refund', NULL").count();
    assert_eq!(
        inserts, 1,
        "exactly one SQL writes refund rows (TRY_REFUND_SQL)"
    );
}

#[test]
fn try_refund_is_one_guarded_statement() {
    let ledger = strip_comments(&src("src/db/credit_ledger.rs"));
    let sql_at = ledger
        .find("pub(crate) const TRY_REFUND_SQL")
        .expect("TRY_REFUND_SQL must exist");
    let sql = &ledger[sql_at..sql_at + ledger[sql_at..].find(");\n").expect("const ends")];
    for needle in [
        "positive_buckets_of!(\"?1\")",
        "t.currency = 'thb') = ?3",
        "t.currency = 'usdc') = ?4",
        "t.currency NOT IN ('thb', 'usdc')",
        "-b.balance",
        "ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING",
    ] {
        assert!(sql.contains(needle), "TRY_REFUND_SQL lost `{needle}`");
    }
    // The read the organizer is checked against and the write share one
    // definition of "payable".
    let positive = fn_body(&ledger, "pub async fn positive_balances(");
    assert!(positive.contains("positive_buckets_of!(\"?1\")"));

    let body = fn_body(&ledger, "pub async fn try_refund(");
    assert!(
        body.contains("release_ended_applies(db).await?"),
        "the payout must see ended events' credit returned, like every balance read"
    );
    assert!(
        body.contains("0 => RefundOutcome::Mismatch")
            && body.contains("RefundOutcome::AlreadyRecorded"),
        "a 0-row write must be told apart: retry (no-op) vs refusal (409)"
    );
    assert!(
        !body.contains("unwrap_or("),
        "a failed idempotency read must not default to either answer"
    );
}

// ---------------------------------------------------------------------------
// 2. The clear: confirmed amount, guarded write, audit, then clear
// ---------------------------------------------------------------------------

#[test]
fn clear_needs_the_confirmed_amount_and_audits_before_clearing() {
    let code = strip_comments(&src(HANDLER));
    let handler = fn_body(&code, "pub async fn clear_credit_refund_request_handler");
    let at = |needle: &str| {
        handler
            .find(needle)
            .unwrap_or_else(|| panic!("clear handler lost `{needle}`"))
    };
    let paid = at("body.paid.ok_or_else(");
    let compare = at("if paid != payable");
    let reversal = at("reverse_held_credit(");
    let mismatch = at("RefundOutcome::Mismatch");
    let audit = at("audit_payout(");
    let clear = at("contacts::clear_credit_refund_requested(");
    assert!(
        paid < compare
            && compare < reversal
            && reversal < mismatch
            && mismatch < audit
            && audit < clear,
        "order must be: confirmed amount → compare → guarded reversal → 409 on mismatch \
         → audit → clear"
    );
    assert!(
        handler[mismatch..audit].contains("AppError::Conflict"),
        "a reversal the guard refused must be a 409, not a clear"
    );
    assert!(
        handler[audit..clear].contains("map_err(AppError::Internal)?"),
        "an unrecorded payer must abort the clear (the retry re-audits)"
    );
    assert!(
        handler[compare..reversal].contains("AppError::Conflict(payout_mismatch_message("),
        "a stale amount must be refused before anything is written"
    );
}

// ---------------------------------------------------------------------------
// 3. Org scope
// ---------------------------------------------------------------------------

#[test]
fn both_payout_endpoints_are_org_scoped() {
    let code = strip_comments(&src(HANDLER));
    for handler in [
        "pub async fn credit_refund_requests_handler",
        "pub async fn clear_credit_refund_request_handler",
    ] {
        let body = fn_body(&code, handler);
        assert!(
            body.contains("payout_scope(&state, &claims.email).await?")
                && body.contains("require_payout_operator(&scope)?"),
            "{handler} must resolve and enforce the caller's payout scope"
        );
    }
    let list = fn_body(&code, "pub async fn credit_refund_requests_handler");
    assert!(
        list.contains("scope.covers("),
        "the queue must filter rows by scope"
    );
    let clear = fn_body(&code, "pub async fn clear_credit_refund_request_handler");
    let covers = clear.find("scope.covers(").expect("clear must check scope");
    assert!(
        covers < clear.find("reverse_held_credit(").expect("reversal"),
        "the scope check must run before any money moves"
    );
}

// ---------------------------------------------------------------------------
// 4. The payout account
// ---------------------------------------------------------------------------

#[test]
fn the_payout_account_is_deleted_with_the_request() {
    let contacts = strip_comments(&src("src/db/contacts.rs"));
    let clear = fn_body(
        &contacts,
        "pub(crate) async fn clear_credit_refund_requested(",
    );
    assert!(
        clear.contains("delete_for_person_statement(") && clear.contains(".batch("),
        "clearing a request must delete its account in the same batch"
    );
    let erasure = fn_body(&contacts, "pub(crate) async fn clear_contact_pii(");
    assert!(
        erasure.contains("DELETE FROM credit_refund_accounts"),
        "PDPA erasure must delete the payout account"
    );
    let set = fn_body(
        &contacts,
        "pub(crate) async fn set_credit_refund_requested(",
    );
    assert!(
        set.contains("save_statement(") && set.contains(".batch("),
        "the account must be written in the same batch as the request flag"
    );
    let cleanup = strip_comments(&src("src/cleanup.rs"));
    let purge = cleanup
        .find("credit_refund_accounts::purge(db)")
        .expect("the nightly cleanup must purge payout accounts");
    let index = cleanup
        .find("get_event_index(kv).await")
        .expect("cleanup reads the event index");
    assert!(
        purge < index,
        "the purge must not depend on the event index read, which can abort the run"
    );
}

#[test]
fn account_details_never_reach_the_log() {
    const FIELDS: [&str; 5] = [
        "bank_account",
        "account_name",
        "promptpay_id",
        "bank_name",
        "account =",
    ];
    for file in [
        HANDLER,
        "src/handlers/deposit/thb/handlers/credit_payout.rs",
        "src/db/credit_refund_accounts.rs",
        "src/db/contacts.rs",
    ] {
        let code = strip_comments(&src(file));
        for (at, _) in code.match_indices("tracing::") {
            let call = &code[at..];
            let call = &call[..call.find(");").unwrap_or(call.len())];
            for field in FIELDS {
                assert!(
                    !call.contains(field),
                    "{file}: a tracing call mentions `{field}` — account details are \
                     personal data and must never be logged:\n{call}"
                );
            }
            assert!(
                !call.contains("?account") && !call.contains("%account") && !call.contains("?body"),
                "{file}: a tracing call records the account or the request body:\n{call}"
            );
        }
    }
}

#[test]
fn the_slip_key_carries_no_email() {
    use event_checkin_worker::storage::credit_payout_key;
    let key = credit_payout_key("", "Person@Gmail.com", "2026-10-01 09:00:00");
    assert!(key.starts_with("credit-payouts/default/"), "{key}");
    assert!(key.ends_with("/20261001090000"), "{key}");
    assert!(!key.to_lowercase().contains("person"), "{key}");
    assert_eq!(
        key,
        credit_payout_key("", " person@gmail.com ", "2026-10-01 09:00:00"),
        "one contact, one owner segment"
    );
    assert_ne!(
        key,
        credit_payout_key("", "person@gmail.com", "2026-10-02 09:00:00"),
        "a new request never overwrites an earlier slip"
    );
    let org = credit_payout_key("Org A/../x", "p@x.example", "1");
    assert!(org.starts_with("credit-payouts/org_a____x/"), "{org}");
}
