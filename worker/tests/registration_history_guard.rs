//! An attendee's registration history must survive the event ending.
//!
//! Issue 086. `my_registrations` filtered out events whose status is
//! `completed`, on both the D1 and the KV path. The moment an organizer marked
//! a past event finished, every attendee who had ever registered for it lost
//! that row — and the profile page told them, in as many words, that they had
//! "not registered for any events yet".
//!
//! It surfaced when 12 production events were marked completed in one sitting
//! and all 477 attendees' histories went blank at once. No data was lost; it
//! was invisible, which is worse, because it looks like data loss.
//!
//! `archived` stays excluded — that is the hidden/soft-deleted state, and the
//! two must not drift apart again.

use std::{fs, path::Path};

fn worker_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = worker_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

#[test]
fn the_d1_query_keeps_completed_events() {
    let sql = read("src/handlers/sql/my_registrations.sql");

    assert!(
        !sql.to_lowercase()
            .contains("not in ('completed','archived')"),
        "excluding 'completed' erases the attendee's history the moment an \
         event ends — that is Issue 086"
    );
    assert!(
        sql.contains("e.status <> 'archived'"),
        "archived is the hidden state and must still be excluded"
    );
}

#[test]
fn the_kv_fallback_keeps_completed_events() {
    let source = read("src/handlers/register/my_registration.rs");

    assert!(
        !source.contains("EventStatus::Completed | EventStatus::Archived"),
        "the KV fallback must not drop completed events either — both paths \
         serve the same page and have to agree"
    );
    assert!(
        source.contains("matches!(meta.status, EventStatus::Archived)"),
        "the KV fallback must still skip archived events"
    );
}

/// The two paths back the same endpoint, so a filter change to one without the
/// other produces a history that depends on whether D1 happens to be bound.
/// Strip `--` comments so the check reads the filter, not the prose that
/// explains it. The first draft of this test matched the word "completed"
/// inside its own explanatory comment.
fn sql_without_comments(sql: &str) -> String {
    sql.lines()
        .map(|line| line.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
}

#[test]
fn both_paths_agree_on_what_is_excluded() {
    let sql = sql_without_comments(&read("src/handlers/sql/my_registrations.sql"));
    let source = read("src/handlers/register/my_registration.rs");

    let sql_drops_completed = sql.contains("'completed'");
    let kv_drops_completed =
        source.contains("EventStatus::Completed |") || source.contains("| EventStatus::Completed");

    assert_eq!(
        sql_drops_completed, kv_drops_completed,
        "the D1 query and the KV fallback disagree about excluding completed \
         events; the same attendee would see a different history depending on \
         which path served the request"
    );
}
