//! Regression guards for `attendees.ticket_name` (`.issues/136`, migration 0050).
//!
//! The bug this exists to prevent happening again: D1 had no `ticket_name`
//! column, the D1 read path is what almost every screen uses, and it filled the
//! gap with the attendee's **name**. The admin list flags a VIP with
//! `ticket_name.to_lowercase().contains("vip")`, so anyone called Vipada,
//! Vipawee or Vipawadee — ordinary Thai given names — wore a VIP badge.
//!
//! Every invariant below fails silently if broken. Nothing errors, no test goes
//! red on its own, and the first sign is an organizer looking at a badge that
//! is not true.
//!
//! 1. The column exists, and is nullable — "nobody told us" must stay
//!    distinguishable from "set to nothing", or the backfill cannot tell which
//!    rows it still has to visit.
//! 2. Every writer that creates an attendee sets it. Four writers is exactly
//!    the shape where one gets missed.
//! 3. The read path reads the column and not some other field.
//! 4. The sync carries the sheet value through — it is the only backfill there
//!    is, because the values live in Google Sheets and SQL cannot reach them.
//! 5. The two halves of the same flow agree on the same constant.

use std::fs;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Strip `--` comments so a guard cannot be satisfied by prose *about* the
/// thing it guards.
fn strip_sql_comments(sql: &str) -> String {
    sql.lines()
        .map(|l| match l.trim_start().starts_with("--") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Same, for Rust `//` line comments.
fn strip_rs_comments(code: &str) -> String {
    code.lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn migration_0050_adds_a_nullable_ticket_name() {
    let sql = strip_sql_comments(&repo_file("migrations/0050_attendees_ticket_name.sql"));
    assert!(
        sql.contains("ALTER TABLE attendees ADD COLUMN ticket_name TEXT"),
        "migration 0050 must add attendees.ticket_name"
    );
    assert!(
        !sql.to_uppercase().contains("NOT NULL"),
        "ticket_name must stay NULLABLE. A NOT NULL DEFAULT '' collapses \"never \
         set\" into \"set to nothing\", and the sheet backfill then cannot tell \
         which rows it still has to visit"
    );
}

/// The four D1 writers that create an `attendees` row. A writer that inserts a
/// row without a ticket name leaves a hole exactly the size of its own flow —
/// and the hole is invisible, because the read returns an empty string and the
/// badge simply does not appear.
#[test]
fn every_attendee_writer_sets_ticket_name() {
    for (file, writer) in [
        ("src/db/attendees/writes.rs", "upsert_attendee"),
        ("src/db/attendees/walkin.rs", "try_insert_walkin"),
        ("src/db/attendees/management.rs", "upsert_attendee_full"),
    ] {
        let code = strip_rs_comments(&repo_file(file));
        assert!(
            code.contains("ticket_name"),
            "{file} writes attendee rows ({writer}) but never mentions ticket_name — \
             see .issues/136"
        );
    }
}

/// `upsert_post_event_attendee` is deliberately NOT in the list above, and this
/// test pins that decision down so it reads as a choice rather than an
/// oversight to whoever finds it next.
#[test]
fn post_event_leads_deliberately_have_no_ticket_name() {
    let code = repo_file("src/db/attendees/writes.rs");
    let start = code
        .find("pub(crate) async fn upsert_post_event_attendee")
        .expect("upsert_post_event_attendee exists");
    let body = &code[start..];
    let end = body.find("\npub(crate) async fn ").unwrap_or(body.len());
    assert!(
        !strip_rs_comments(&body[..end]).contains("ticket_name"),
        "post-event registrants are leads, not ticket holders — they are excluded \
         from capacity and check-in queries and never receive a ticket, so \
         inventing a tier for them would put a badge on a row that is not an \
         attendee. If this changes, change .issues/136 too"
    );
}

/// The whole defect in one line: the read must come from the column, not from
/// another field that happens to be in scope.
#[test]
fn the_read_path_reads_the_column_and_not_the_name() {
    let code = strip_rs_comments(&repo_file("src/db/attendees/reads.rs"));
    assert_eq!(
        code.matches("ticket_name: self.ticket_name.clone().unwrap_or_default()")
            .count(),
        2,
        "BOTH D1 -> Attendee conversions (D1AttendeeRow and D1AttendeeWithCounts) \
         must read the ticket_name column. Fixing one and leaving the sibling is \
         how this bug survived in two places at once"
    );
    assert!(
        !code.contains("ticket_name: self.name."),
        "ticket_name must never be a copy of the attendee's name — that is \
         .issues/136 verbatim: it made every Vipada a VIP"
    );
    // A column absent from the SELECT deserializes as NULL and reads exactly
    // like "the organizer set no tier", which is a silent way to lose it again.
    //
    // Matched on the projection prefix rather than on "SELECT" anywhere, so the
    // nested `(SELECT COUNT(*) FROM attendees ...)` aggregates inside the
    // counts query do not inflate the number and make this look satisfied.
    let projections = [
        "\"SELECT id, event_id, email, name,",
        "\"SELECT a.id, a.event_id, a.email, a.name,",
    ];
    let found: Vec<&str> = projections
        .iter()
        .flat_map(|prefix| code.match_indices(prefix))
        .map(|(i, _)| &code[i..code.len().min(i + 200)])
        .collect();
    assert_eq!(
        found.len(),
        4,
        "expected 4 attendee-projecting SELECTs in reads.rs, found {}. If that \
         count changed, the new one needs ticket_name too",
        found.len()
    );
    for (i, sel) in found.iter().enumerate() {
        assert!(
            sel.contains("ticket_name"),
            "attendee SELECT #{i} in reads.rs does not project ticket_name. A \
             column left out of the projection comes back NULL and is \
             indistinguishable from an unset tier — which is how this gets lost \
             again, silently"
        );
    }
}

/// The sync is the only backfill that exists — the real tiers live in Google
/// Sheets and no SQL migration can reach them.
#[test]
fn the_sheet_sync_carries_ticket_name_through() {
    let code = strip_rs_comments(&repo_file("src/handlers/events/sync.rs"));
    assert!(
        code.contains("&a.ticket_name"),
        "sync_one_attendee must pass the sheet row's ticket_name into \
         upsert_attendee_full. Without it, migration 0050 adds a column that \
         nothing ever fills for the ~514 attendees who already exist, and the \
         organizer's real 'VIP'/'Speaker' tiers stay lost"
    );
}

/// Both halves of a flow must write the same string. They were two unrelated
/// literals in two files before `.issues/136`, which is how they were free to
/// disagree.
#[test]
fn the_sheet_writer_and_the_d1_writer_agree_on_the_same_constant() {
    for (sheet_file, d1_file, constant) in [
        (
            "src/sheets/write/append.rs",
            "src/db/attendees/writes.rs",
            "TICKET_NAME_SELF_REGISTERED",
        ),
        (
            "src/sheets/write/append.rs",
            "src/db/attendees/walkin.rs",
            "TICKET_NAME_WALK_IN",
        ),
    ] {
        for file in [sheet_file, d1_file] {
            assert!(
                strip_rs_comments(&repo_file(file)).contains(constant),
                "{file} must use the shared {constant} from \
                 domain::models::attendee, not a bare string literal. Two \
                 literals in two files is how the Sheet and D1 came to disagree \
                 about what a self-registration is called"
            );
        }
    }
}
