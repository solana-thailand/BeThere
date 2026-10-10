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
//! 3. Every payout endpoint is org-scoped and refuses per-event scanners.
//!    The organizer-initiated payout (`.issues/192`) pays only to the
//!    deposit account, never over an open request, and needs the slip.
//! 4. The payout account dies at payout, on PDPA erasure, and in the purge
//!    once no request is open and no credit is held — and never reaches the
//!    log stream.
//! 5. The account the attendee gave with their deposit is copied by the one
//!    writer of `held_as_credit = 1`, after its flip and without failing the
//!    hold; the attendee API returns it masked, never in full.

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
/// The money-moving core both payout paths share (`.issues/192`).
const CORE: &str = "src/handlers/deposit/thb/handlers/credit_payout.rs";
/// The organizer-initiated payout (`.issues/192`).
const ORGANIZER: &str = "src/handlers/deposit/thb/handlers/organizer_credit_payout.rs";

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
    // .issues/163: the person's release is enough, because TRY_REFUND_SQL reads
    // only `?1`'s buckets. It must run before the write.
    let release = body
        .find("release_person_ended_applies(db, &email_lc).await?")
        .expect("the payout must see ended events' credit returned, like every balance read");
    assert!(
        release
            < body
                .find("TRY_REFUND_SQL")
                .expect("try_refund runs TRY_REFUND_SQL"),
        "the release must run before the payout write"
    );
    assert!(
        body.contains("D1Type::Text(&email_lc)"),
        "try_refund must bind the same email it released"
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
    // The core: compare → guarded reversal → 409 on mismatch → audit.
    let core_code = strip_comments(&src(CORE));
    let core = fn_body(&core_code, "pub(super) async fn settle_payout");
    let at = |needle: &str| {
        core.find(needle)
            .unwrap_or_else(|| panic!("settle_payout lost `{needle}`"))
    };
    let compare = at("if paid != payable");
    let reversal = at("try_refund(");
    let mismatch = at("RefundOutcome::Mismatch");
    let audit = at("audit_payout(");
    assert!(
        compare < reversal && reversal < mismatch && mismatch < audit,
        "order must be: compare → guarded reversal → 409 on mismatch → audit"
    );
    assert!(
        core[compare..reversal].contains("AppError::Conflict(payout_mismatch_message("),
        "a stale amount must be refused before anything is written"
    );
    assert!(
        core[mismatch..audit].contains("AppError::Conflict"),
        "a reversal the guard refused must be a 409, not a clear"
    );
    assert!(
        core[audit..].contains("map_err(AppError::Internal)"),
        "an unrecorded payer must fail the payout (the retry re-audits)"
    );

    // The clear: the confirmed amount, then the core, then the clear.
    let code = strip_comments(&src(HANDLER));
    let handler = fn_body(&code, "pub async fn clear_credit_refund_request_handler");
    let at = |needle: &str| {
        handler
            .find(needle)
            .unwrap_or_else(|| panic!("clear handler lost `{needle}`"))
    };
    let paid = at("body.paid.ok_or_else(");
    let settle = at("settle_payout(");
    let clear = at("contacts::clear_credit_refund_requested(");
    assert!(
        paid < settle && settle < clear,
        "order must be: confirmed amount → settle_payout → clear"
    );
    assert!(
        handler[settle..clear].contains(".await?"),
        "a refused or failed payout must abort the clear"
    );
}

#[test]
fn the_organizer_payout_pays_only_the_deposit_account_with_a_slip() {
    let code = strip_comments(&src(ORGANIZER));
    let handler = fn_body(&code, "pub async fn organizer_credit_payout_handler");
    let at = |needle: &str| {
        handler
            .find(needle)
            .unwrap_or_else(|| panic!("organizer payout lost `{needle}`"))
    };
    let settle = at("settle_payout(");
    for needle in [
        ".validate()",
        "body.paid.is_zero()",
        "attach the transfer slip",
        "open_request_for_person(",
        "AccountSource::Deposit",
    ] {
        assert!(
            at(needle) < settle,
            "`{needle}` must be checked before any money moves"
        );
    }
    assert!(
        handler[settle..].contains("Some(proof)"),
        "the required slip must reach settle_payout"
    );
    assert!(
        handler.contains("PayoutInitiator::Organizer"),
        "the audit entry must say the organizer started it"
    );
}

// ---------------------------------------------------------------------------
// 3. Org scope
// ---------------------------------------------------------------------------

#[test]
fn both_payout_endpoints_are_org_scoped() {
    let code = strip_comments(&src(HANDLER));
    let organizer = strip_comments(&src(ORGANIZER));
    for (code, handler) in [
        (&code, "pub async fn credit_refund_requests_handler"),
        (&code, "pub async fn clear_credit_refund_request_handler"),
        (&organizer, "pub async fn credit_payout_candidates_handler"),
        (&organizer, "pub async fn organizer_credit_payout_handler"),
    ] {
        let body = fn_body(code, handler);
        assert!(
            body.contains("payout_scope(&state, &claims.email).await?")
                && body.contains("require_payout_operator(&scope)?"),
            "{handler} must resolve and enforce the caller's payout scope"
        );
    }
    for (code, list) in [
        (&code, "pub async fn credit_refund_requests_handler"),
        (&organizer, "pub async fn credit_payout_candidates_handler"),
    ] {
        assert!(
            fn_body(code, list).contains("scope.covers("),
            "{list} must filter rows by scope"
        );
    }
    let core = strip_comments(&src(CORE));
    assert!(
        fn_body(&core, "pub(super) async fn scoped_buckets").contains("scope.covers("),
        "scoped_buckets must refuse credit outside the caller's scope"
    );
    for (code, handler) in [
        (&code, "pub async fn clear_credit_refund_request_handler"),
        (&organizer, "pub async fn organizer_credit_payout_handler"),
    ] {
        let body = fn_body(code, handler);
        let covers = body
            .find("scoped_buckets(")
            .unwrap_or_else(|| panic!("{handler} must check scope"));
        assert!(
            covers < body.find("settle_payout(").expect("settle_payout"),
            "{handler}: the scope check must run before any money moves"
        );
    }
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
        "src/db/thb_deposits.rs",
        "src/handlers/deposit/thb/handlers/hold_credit.rs",
        "src/handlers/deposit/thb/handlers/hold_admin.rs",
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

// ---------------------------------------------------------------------------
// 5. The account from the deposit
// ---------------------------------------------------------------------------

#[test]
fn every_hold_snapshots_the_deposit_account() {
    // Every writer of `held_as_credit = 1` in the worker.
    let mut writers = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    visit_rs(&root, &mut |path, text| {
        let code = strip_comments(text);
        if code.contains("held_as_credit = 1,") || code.contains("SET held_as_credit = 1") {
            writers.push(path.display().to_string());
        }
    });
    assert_eq!(
        writers.len(),
        1,
        "one writer flips held_as_credit: {writers:?}"
    );
    assert!(writers[0].ends_with("db/thb_deposits.rs"), "{writers:?}");

    let deposits = strip_comments(&src("src/db/thb_deposits.rs"));
    let cas = fn_body(&deposits, "pub async fn try_settle_hold_credit(");
    let flip = cas.find("SET held_as_credit = 1").expect("the flip");
    let lost = cas
        .find("return Ok(false)")
        .expect("a lost CAS returns early");
    let copy = cas
        .find("credit_refund_accounts::snapshot_from_deposit(")
        .expect("the CAS must copy the deposit's refund account");
    assert!(
        flip < lost && lost < copy,
        "the account is copied only after THIS call won the flip"
    );
    assert!(
        !cas[copy..].contains(".await?") && cas[copy..].contains("Err(e) => tracing::warn!"),
        "a failed copy must not fail the hold"
    );

    // Both hold paths name the credit holder, the same email the ledger credits.
    for (file, email) in [
        (
            "src/handlers/deposit/thb/handlers/hold_credit.rs",
            "&claims.email",
        ),
        (
            "src/handlers/deposit/thb/handlers/hold_admin.rs",
            "&attendee_email",
        ),
    ] {
        let code: String = strip_comments(&src(file))
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let call = &code[code
            .find("try_settle_hold_credit(")
            .expect("hold calls the CAS")..];
        let call = &call[..call.find(".await").expect("awaited")];
        assert!(
            call.ends_with(&format!("&now,{email},)")),
            "{file}: the snapshot must go to {email}: {call}"
        );
        assert!(
            code.contains(&format!("credit_ledger::record(db,{email},")),
            "{file}: the ledger and the snapshot must credit the same email"
        );
    }
}

#[test]
fn the_snapshot_is_event_scoped_and_yields_to_the_attendee() {
    let accounts = strip_comments(&src("src/db/credit_refund_accounts.rs"));
    let at = accounts
        .find("pub(crate) const SNAPSHOT_FROM_DEPOSIT_SQL")
        .expect("snapshot SQL");
    let sql = &accounts[at..at + accounts[at..].find("\";").expect("const ends")];
    for needle in [
        "d.event_id = ?2 AND d.attendee_id = ?3",
        "d.held_as_credit = 1 AND d.refunded = 0",
        "WHERE credit_refund_accounts.source = 'deposit'",
    ] {
        assert!(
            sql.contains(needle),
            "SNAPSHOT_FROM_DEPOSIT_SQL lost `{needle}`"
        );
    }
    let migration = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/0059_credit_refund_payout.sql"),
    )
    .expect("0059");
    assert!(
        migration.contains("JOIN attendees a ON a.id = x.attendee_id AND a.event_id = x.event_id"),
        "the backfill must match the attendee on id AND event (ids are global)"
    );
    assert!(
        migration.contains("ON CONFLICT (email) DO NOTHING"),
        "the backfill must be idempotent and never replace an existing account"
    );
}

#[test]
fn the_attendee_gets_a_masked_preview_only() {
    let code = strip_comments(&src(HANDLER));
    let at = code
        .find("pub struct CreditRefundRequestStatus {")
        .expect("status struct");
    let fields = &code[at..at + code[at..].find("\n}").expect("struct ends")];
    assert!(
        fields.contains("pub saved_account: Option<SavedAccountPreview>,")
            && !fields.contains("RefundAccount"),
        "the attendee's status carries the masked preview, never a RefundAccount"
    );
    let handler = fn_body(&code, "pub async fn credit_refund_request_status_handler");
    assert!(handler.contains("credit_refund_accounts::preview_for_person("));
    assert!(
        !handler.contains("chosen_for_person("),
        "the attendee handler must not read the full account"
    );
}

#[test]
fn the_purge_is_by_balance_not_by_age() {
    let accounts = strip_comments(&src("src/db/credit_refund_accounts.rs"));
    let at = accounts
        .find("pub(crate) const PURGE_SQL")
        .expect("purge SQL");
    let sql = &accounts[at..at + accounts[at..].find(");\n").expect("const ends")];
    for needle in [
        "c.credit_refund_requested = 1",
        "positive_buckets_of!(\"credit_refund_accounts.email\")",
        "unreturned_apply_of!(\"credit_refund_accounts.email\")",
    ] {
        assert!(sql.contains(needle), "PURGE_SQL lost `{needle}`");
    }
    assert!(
        !sql.contains("datetime('now'") && !accounts.contains("RETENTION_DAYS"),
        "an account must not expire by age while its credit is still held"
    );
}
