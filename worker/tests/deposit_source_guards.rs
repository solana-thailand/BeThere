//! Regression guards for the comp action and the `deposit_source` column
//! (`.issues/129` Gap 1).
//!
//! Approving a payment slip is the only way an attendee receives their ticket
//! QR, and approving is simultaneously the promise to refund ฿500. The comp
//! action is the third move — admit them, owe nothing — and every invariant
//! below fails SILENTLY if broken: the endpoint still returns 200, the attendee
//! still gets in, and the money goes wrong somewhere nobody is looking.

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

const COMP: &str = "src/handlers/deposit/thb/handlers/comp.rs";

/// Migration 0047 backfills `deposit_source` from the classifier in
/// `ThbDeposit::source()`. The two are separate implementations of one rule —
/// SQL and Rust — so they can drift, and the drift is invisible: the column
/// simply starts disagreeing with the fallback for rows written later.
///
/// Order is the part that matters most. A row that is BOTH rolling-credit and
/// ฿0 must classify as Credit; if the SQL put the comp arm first, every
/// credit-covered ฿0 deposit would be backfilled as a comp, writing off money
/// the attendee is still owed.
#[test]
fn deposit_source_backfill_matches_the_legacy_classifier() {
    let sql = src("migrations/0047_thb_deposit_source.sql");
    let body: String = sql
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");

    let credit_at = body
        .find("THEN 'credit'")
        .expect("0047 must backfill the credit arm");
    let comp_at = body
        .find("THEN 'comp'")
        .expect("0047 must backfill the comp arm");
    let cash_at = body
        .find("ELSE 'cash'")
        .expect("0047 must backfill the cash arm");

    assert!(
        credit_at < comp_at && comp_at < cash_at,
        "0047's CASE arms are out of order (credit at {credit_at}, comp at {comp_at}, cash at \
         {cash_at}). ThbDeposit::source() checks credit FIRST, so a ฿0 rolling-credit deposit \
         is Credit — reorder these and the backfill writes off credit the attendee still owns"
    );

    // Every sentinel the Rust fallback reads must appear in the SQL.
    for sentinel in [
        "SYSTEM_ROLLING_CREDIT",
        "ROLLING_CREDIT_AUTO_APPLIED",
        "SYSTEM_STAFF_WAIVE",
        "STAFF_COMP_WAIVED",
        "amount_thb = 0",
    ] {
        assert!(
            body.contains(sentinel),
            "0047's backfill does not read `{sentinel}`, which ThbDeposit::source() does — \
             rows matching only that sentinel would be backfilled as the wrong source"
        );
    }
}

/// The column must be addable to a live table days before an event, and must
/// stay nullable so "never decided" is distinguishable from "decided as cash".
#[test]
fn the_deposit_source_column_is_additive_and_constrained() {
    let sql = src("migrations/0047_thb_deposit_source.sql").to_uppercase();
    assert!(
        sql.contains("ALTER TABLE THB_DEPOSITS ADD COLUMN DEPOSIT_SOURCE"),
        "0047 must ADD COLUMN, not rebuild the table — a rebuild of the live deposits table \
         before RTM #6 is what .issues/127 was deferred to avoid"
    );
    assert!(
        sql.contains("CHECK")
            && sql.contains("'CASH'")
            && sql.contains("'CREDIT'")
            && sql.contains("'COMP'"),
        "deposit_source must be CHECK-constrained to the three DepositSource values; without \
         it a typo in any writer becomes a row that classifies as None and silently falls \
         back to the sentinels"
    );
}

/// A comp writes off money while leaving the record of what was claimed. If it
/// zeroed the amount or blanked the slip URL it would be doing exactly what the
/// column exists to avoid — and the evidence would be gone at the moment it is
/// most likely to be wanted.
#[test]
fn comping_never_destroys_the_evidence() {
    let code = strip_comments(&src(COMP));
    for forbidden in ["amount_thb = 0", "slip_url = None", "amount_thb: 0"] {
        assert!(
            !code.contains(forbidden),
            "the comp handler writes `{forbidden}`. Destroying the amount or the slip URL is \
             the pre-0047 workaround this action replaces: the whole point is that a deposit \
             can be written off with its evidence intact"
        );
    }
    assert!(
        code.contains("deposit.deposit_source = Some(DepositSource::Comp)"),
        "the comp handler must record the source outright rather than manufacturing a sentinel"
    );
}

/// Three states where comping would make an existing money fact untrue.
#[test]
fn comping_refuses_every_already_settled_deposit() {
    let code = strip_comments(&src(COMP));
    assert!(
        code.contains("deposit.refunded"),
        "comping a refunded deposit must be refused — the cash has already gone back out"
    );
    assert!(
        code.contains("deposit.held_as_credit"),
        "comping a held-as-credit deposit must be refused — it would delete credit the \
         attendee can already spend"
    );
    assert!(
        code.contains("DepositSource::Credit =>"),
        "comping a rolling-credit-covered deposit must be refused — that ฿500 came out of the \
         attendee's balance, so writing it off consumes credit they are still owed"
    );
}

/// A comp must never reach the refund queue, which reads `refundable`.
#[test]
fn a_comped_deposit_is_marked_unrefundable() {
    let code = strip_comments(&src(COMP));
    assert!(
        code.contains("status.refundable = false"),
        "the comp handler must clear `refundable` on the deposit status. Marking the deposit \
         source alone is not enough: the refund queue reads deposit_statuses, so a comp would \
         keep appearing in the organizer's list of people to pay"
    );
}

/// The ticket must be identical to an approved attendee's, and it must come
/// from the same code as approval. Two copies of "issue the QR" is how one path
/// silently stops issuing it.
#[test]
fn comp_and_approval_issue_the_same_ticket() {
    let comp = strip_comments(&src(COMP));
    let verify = strip_comments(&src("src/handlers/deposit/thb/handlers/slip_verify.rs"));
    let helper = "issue_ticket_qr_if_absent";

    assert!(
        comp.contains(helper),
        "the comp handler must issue the ticket QR — without it the attendee is marked \
         admitted and still cannot get in, which is the bug with extra steps"
    );
    assert!(
        verify.contains(helper),
        "the approval path must issue the QR through the same helper as comp; two copies is \
         how one of them stops matching the other"
    );
    assert_eq!(
        verify.matches(helper).count(),
        1,
        "the approval path should call the QR helper exactly once. It previously contained two \
         hand-written copies of this block, one per worker_ctx branch — the helper picks the \
         write path itself, so a second call site means the duplication came back"
    );
}

/// `Extension<Claims>` only resolves in the authed router. A handler that needs
/// it and is registered anywhere else returns 500, not 401 — a failure that
/// reads like a bug in the handler rather than a routing mistake.
#[test]
fn the_comp_route_is_in_the_authed_router() {
    let router = strip_comments(&src("src/handlers/mod.rs"));
    let comp_at = router
        .find("/deposit/thb/comp")
        .expect("the comp route must be registered");
    let verify_at = router
        .find("/deposit/thb/verify")
        .expect("the verify route must be registered");

    // Both are organizer actions on the same resource; they belong to the same
    // router. Anchoring to `verify` rather than to a line number keeps this
    // true as the file is reorganised.
    let gap = comp_at.abs_diff(verify_at);
    assert!(
        gap < 2000,
        "the comp route is {gap} bytes from the verify route — it has probably drifted out of \
         the authed router, where Extension<Claims> resolves. Outside it the handler 500s \
         instead of 401ing, which reads as a handler bug"
    );
}

/// A deferred migration must live OUTSIDE `migrations_dir`, because
/// `wrangler d1 migrations apply` applies every `.sql` it finds there and never
/// reads the file. `0048` carried a `-- DO NOT APPLY BEFORE 2026-09-28` header
/// while sitting in `migrations/` for about twenty minutes on 2026-09-22, and
/// wrangler cheerfully applied it — a small re-enactment of the bug
/// `.issues/127` is about: a rule written where nothing enforces it.
///
/// The directory is the enforcement. This test is what keeps it that way.
#[test]
fn deferred_migrations_are_not_in_the_applied_directory() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let deferred: Vec<String> = fs::read_dir(&dir)
        .expect("migrations dir must exist")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
        .filter(|e| {
            fs::read_to_string(e.path())
                .map(|body| body.to_uppercase().contains("DO NOT APPLY"))
                .unwrap_or(false)
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    assert!(
        deferred.is_empty(),
        "these migrations say DO NOT APPLY but sit in the directory wrangler applies from, \
         where nothing reads that header: {deferred:?}. Move them to \
         worker/migrations-pending/ — the directory is the enforcement, the comment is only \
         the explanation."
    );

    // ...and the deferral directory must actually be holding something, or this
    // guard is passing because the convention was quietly abandoned.
    let pending = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations-pending");
    assert!(
        pending.join("README.md").exists(),
        "worker/migrations-pending/README.md is missing — without it the convention is \
         invisible to the next person, who will put the next deferred migration back in \
         migrations/ and have it applied"
    );
}
