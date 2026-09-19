//! Regression guards for the nightly cleanup cron's destructive half.
//!
//! `cleanup.rs` is the only place in the worker that deletes financial rows, it
//! runs unattended at 03:00 UTC, and what it deletes cannot be recovered from
//! production. Every invariant below fails SILENTLY if broken — the compile
//! stays green, the cron keeps reporting success, and the loss is only visible
//! months later when someone asks what happened to a deposit:
//!
//! 1. The archive runs BEFORE the delete, and a failed archive ABORTS it.
//!    Without the gate, the 2026-09-19 run repeats: 14 rows and 7,000 THB gone
//!    with no record that the money ever arrived (`.issues/126`).
//! 2. The archive keeps no personal data. If it did, the delete would stop
//!    being a purge and the retention policy would be a lie.
//! 3. The archive is keyed on the deposit ROW and the delete is gated on the
//!    archive covering every live row. Keyed on (event_id, attendee_id) — which
//!    `thb_deposits` does not enforce — one attendee's two deposits archive once
//!    and delete twice (`.issues/127`).

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

#[test]
fn deposit_delete_is_gated_on_a_successful_archive() {
    let code = strip_comments(&src("src/cleanup.rs"));

    let archive_at = code
        .find("archive_thb_deposits_for_event(")
        .expect("cleanup must archive deposits before deleting them");
    let delete_at = code
        .find("delete_thb_deposits_for_event(")
        .expect("cleanup still deletes deposits");
    assert!(
        archive_at < delete_at,
        "the archive must run BEFORE the delete — after it, there is nothing left to copy"
    );

    // The delete has to sit inside the archive's success arm. Anything between
    // them that re-opens the flow (an `Err(_) =>` that falls through, a `let _ =`)
    // means a failed archive still deletes.
    let between = &code[archive_at..delete_at];
    assert!(
        between.contains("Ok("),
        "the delete must be inside the archive's Ok arm, not merely after the call"
    );
    assert!(
        !between.contains("unwrap_or") && !between.contains("let _ ="),
        "a swallowed archive error deletes the deposits anyway — that is the whole defect"
    );
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

/// A successful archive write is not the same as a complete archive. The delete
/// has to compare what it is about to destroy with what was kept, because the
/// next way to lose a row will not be the duplicate-pair way.
#[test]
fn deposit_delete_requires_the_archive_to_cover_every_live_row() {
    let code = strip_comments(&src("src/cleanup.rs"));
    let archive_at = code
        .find("archive_thb_deposits_for_event(")
        .expect("cleanup must archive deposits");
    let delete_at = code
        .find("delete_thb_deposits_for_event(")
        .expect("cleanup still deletes deposits");
    let between = &code[archive_at..delete_at];

    assert!(
        between.contains("is_complete()"),
        "the delete must be gated on ArchiveCoverage::is_complete(), not merely on \
         the archive call returning Ok — a partial archive returns Ok too"
    );

    let db = strip_comments(&src("src/db/thb_deposits.rs"));
    let f = &db[db.find("fn is_complete(").expect("is_complete exists")..];
    let body = &f[..f.find("\n    }").expect("fn ends")];
    assert!(
        body.contains("self.unarchived == 0"),
        "coverage must be decided by MATCHING ids, not by comparing totals. An \
         `archived >= live` form asks whether the archive is big enough, which \
         passes whenever `archived` is inflated by rows corresponding to nothing \
         live — 0044-era rows with a NULL source_deposit_id, or rows kept from an \
         earlier purge of an event that has since taken new deposits"
    );
    assert!(
        !body.contains(">= self.live"),
        "the totals comparison must not come back — it reads as the guarantee \
         while the INSERT … SELECT is what actually provides it"
    );

    let archive = &db[db
        .find("pub async fn archive_thb_deposits_for_event(")
        .expect("archive fn exists")..];
    let archive = &archive[..archive.find("\n}\n").expect("fn ends")];
    assert!(
        archive.contains("a.source_deposit_id = d.id"),
        "`unarchived` must be counted by joining live rows to archive rows on the \
         deposit id, or it is measuring something other than what it claims"
    );
}
