//! Regression guards for the duplicate-payment-slip check (`.issues/129`).
//!
//! Anyone can upload any image to the THB deposit page — nothing about a slip
//! is verified — so the organizer catches the people who did not really pay by
//! recognising them, from memory, at refund time. Migration 0046 and
//! `slip_fingerprint.rs` move that knowledge into the system by hashing the
//! image bytes.
//!
//! Every invariant below fails SILENTLY if broken. The column still exists, the
//! handler still returns 200, the upload still succeeds — and the check quietly
//! stops catching anything. That is the failure mode this file exists for:
//! `.issues/072` (a rule that can only ever pass is not a rule) and the
//! recurring defect where a guard lands on one writer and its sibling keeps the
//! bug.

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

const ATTENDEE_UPLOAD: &str = "src/handlers/deposit/thb/handlers/slip_upload.rs";
const ADMIN_UPLOAD: &str = "src/handlers/deposit/thb/handlers/slip_admin_upload.rs";

/// BOTH writers must record the hash. A slip that entered through the admin
/// screen with no fingerprint is invisible to the check, and the next attendee
/// to submit that same image looks like the first person to use it.
#[test]
fn every_slip_writer_records_the_fingerprint() {
    for path in [ATTENDEE_UPLOAD, ADMIN_UPLOAD] {
        let code = strip_comments(&src(path));
        assert!(
            code.contains("slip_blake3: fingerprint"),
            "{path} builds a ThbDeposit without recording slip_blake3 — a slip written by \
             this path can never be matched, so the duplicate check has a hole the size of \
             this handler"
        );
    }
}

/// The fingerprint has to be taken while the image bytes are still in the
/// request. `maybe_upload_to_r2` replaces the data URL with a storage path, so
/// hashing after it would fingerprint the *path* — which is derived from
/// event_id and attendee_id and is therefore unique per attendee by
/// construction. The check would pass, return green, and never match anything.
#[test]
fn the_fingerprint_is_taken_before_the_bytes_leave_for_r2() {
    for path in [ATTENDEE_UPLOAD, ADMIN_UPLOAD] {
        let code = strip_comments(&src(path));
        let hashed_at = code
            .find("slip_fingerprint::slip_fingerprint(&body.slip_url)")
            .unwrap_or_else(|| panic!("{path} must fingerprint the uploaded slip"));
        let uploaded_at = code
            .find("maybe_upload_to_r2(")
            .unwrap_or_else(|| panic!("{path} must upload the slip to R2"));
        assert!(
            hashed_at < uploaded_at,
            "{path} fingerprints at byte {hashed_at} but uploads to R2 at byte {uploaded_at} — \
             after the upload `slip_url` is a per-attendee storage path, so the hash would be \
             unique by construction and match nothing, forever"
        );
    }
}

/// The three narrowings in the lookup, plus the empty-string clause.
///
/// `thb_deposits` stores `''` rather than SQL NULL for absent text (see
/// `insert_thb_deposit`), and in SQL `'' = ''` is true. Without `slip_blake3 <>
/// ''`, every unhashed row — which is every row uploaded before 2026-09-22 —
/// becomes a duplicate of every other unhashed row, and in `reject` mode that
/// takes the deposit page down for the whole event.
#[test]
fn the_collision_lookup_cannot_match_everything() {
    let code = strip_comments(&src("src/db/thb_deposits.rs"));
    let (_, query) = code
        .split_once("find_slip_hash_collision")
        .expect("the collision lookup must exist");

    for needle in [
        "event_id = ?1",
        "slip_blake3 = ?2",
        "slip_blake3 <> ''",
        "attendee_id <> ?3",
    ] {
        assert!(
            query.contains(needle),
            "the collision lookup lost `{needle}`. Each clause is load-bearing: the event \
             scope keeps it on an index, `<> ''` stops unhashed rows matching each other, \
             and `attendee_id <> ?3` is what lets a rejected attendee re-upload their own slip"
        );
    }

    assert!(
        query.contains("if slip_blake3.is_empty()"),
        "the lookup must refuse an empty fingerprint in Rust as well as in SQL — either \
         guard alone is one edit away from matching every row in the table"
    );
}

/// The attendee path may reject, and only in `reject` mode. If the rejection
/// were unconditional, rolling the check out would immediately start turning
/// people away — which is the opposite of how a first-pass heuristic on real
/// money should ship.
#[test]
fn rejecting_an_attendee_upload_requires_reject_mode() {
    let code = strip_comments(&src(ATTENDEE_UPLOAD));
    assert!(
        code.contains("DuplicateMode::Reject"),
        "the attendee upload path must gate its rejection on DuplicateMode::Reject"
    );
    let reject_at = code.find("DuplicateMode::Reject").unwrap();
    let error_at = code
        .find("this payment slip has already been submitted")
        .expect("the rejection message must exist");
    assert!(
        reject_at < error_at,
        "the duplicate rejection is not gated on the mode — it would fire in `report` mode too"
    );
}

/// The admin path warns but never rejects, and that is a decision rather than
/// an omission: an organizer uploading on someone's behalf has context the
/// check does not, and blocking them mid-event is worse than the duplicate.
/// Written down as a test so nobody "fixes" the asymmetry without meaning to.
#[test]
fn admin_upload_records_the_hash_but_never_rejects() {
    let code = strip_comments(&src(ADMIN_UPLOAD));
    assert!(
        code.contains("slip_blake3: fingerprint"),
        "the admin path must still record the hash"
    );
    assert!(
        !code.contains("DuplicateMode::Reject"),
        "the admin upload path must never reject a duplicate — an organizer acting on an \
         attendee's behalf is making a judgement the hash cannot make for them"
    );
}

/// The migration must stay additive. A UNIQUE here would be enforced against
/// the wrong thing: a rejected attendee re-uploading the same image is
/// legitimate, and the organizer must still be able to accept two genuinely
/// identical submissions. (The UNIQUE that genuinely should exist is on
/// `(event_id, attendee_id)` — `.issues/127`, deferred to after RTM #6.)
#[test]
fn the_slip_hash_column_is_additive_and_not_unique() {
    let sql = src("migrations/0046_thb_deposit_slip_hash.sql");
    let statements: String = sql
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_uppercase();

    assert!(
        statements.contains("ALTER TABLE THB_DEPOSITS ADD COLUMN SLIP_BLAKE3"),
        "0046 must add the column, not rebuild the table — a rebuild of the live deposits \
         table days before an event is exactly what .issues/127 was deferred to avoid"
    );
    assert!(
        !statements.contains("CREATE UNIQUE INDEX"),
        "slip_blake3 must not be UNIQUE: re-uploading your own slip after a rejection is \
         legitimate, and the duplicate decision belongs in the handler where it can tell \
         'same person again' from 'different person, same image'"
    );
    // Scoped to the ADD COLUMN statement on purpose. A blanket search for
    // "NOT NULL" over the whole file also matches `WHERE slip_blake3 IS NOT
    // NULL` in the partial index, which is the opposite of a problem — it is
    // what keeps the index to the hashed rows.
    let add_column = statements
        .lines()
        .find(|l| l.contains("ADD COLUMN SLIP_BLAKE3"))
        .expect("the ADD COLUMN statement must be on one line for this guard to read it");
    assert!(
        !add_column.contains("NOT NULL"),
        "slip_blake3 must stay nullable — every row uploaded before this migration has no \
         hash, and an invented value is worse than an absent one. Offending statement: \
         {add_column}"
    );
}
