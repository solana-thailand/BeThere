//! One slug, one event.
//!
//! Plan 028 W11 recorded that `PUT /api/events/{id}` could rename an event onto
//! a slug another event already used; create deduplicated only against ids,
//! which stop matching slugs after a rename. Two events on one slug make the
//! public page serve whichever match an isolate found first. The rule is
//! `domain::models::event::slug_taken_by_other` (unit-tested in `domain`);
//! this guard keeps every writer on it.

use std::{fs, path::Path};

fn source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    let body = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} is unreadable: {e} — was the module moved?"));
    // Drop `//` comments so prose naming an old call is not read as a call.
    body.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn function_body<'a>(rel: &str, code: &'a str, signature: &str) -> &'a str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("{rel}: `{signature}` is gone — was it renamed?"));
    let rest = &code[start..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    &rest[..end]
}

const STORE: &str = "event_store/write/update.rs";
const HANDLER: &str = "handlers/events/update.rs";
const CREATE: &str = "event_store/write/create.rs";

#[test]
fn both_update_entry_points_run_the_slug_check() {
    let store = source(STORE);
    let body = function_body(STORE, &store, "pub async fn update_event(");
    assert!(
        body.contains("apply_update_checked(") && !body.contains(" apply_update("),
        "{STORE}: update_event must go through apply_update_checked"
    );
    let handler = source(HANDLER);
    assert!(
        handler.contains("apply_update_checked(") && !handler.contains("::apply_update("),
        "{HANDLER}: PUT /events/{{id}} must go through apply_update_checked"
    );
}

#[test]
fn a_changed_slug_is_checked_in_both_stores() {
    let store = source(STORE);
    let checked = function_body(STORE, &store, "pub async fn apply_update_checked(");
    assert!(
        checked.contains("ensure_slug_free("),
        "{STORE}: apply_update_checked must check a changed slug"
    );
    let ensure = function_body(STORE, &store, "async fn ensure_slug_free(");
    assert!(
        ensure.contains("event_slugs::slug_taken_by_other(")
            && ensure.contains("get_event_index(")
            && ensure.contains("slug_taken_by_other("),
        "{STORE}: ensure_slug_free must consult D1 and the KV index"
    );
}

#[test]
fn create_dedups_against_slugs_not_only_ids() {
    let create = source(CREATE);
    let body = function_body(CREATE, &create, "pub async fn create_event(");
    assert!(
        body.contains("e.slug") && body.contains("row.slug"),
        "{CREATE}: create_event must treat existing slugs (KV and D1) as taken"
    );
}
