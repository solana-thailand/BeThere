//! A slug is one event's locator: `PUT /api/events/{id}` and create must not
//! hand it to a second event (plan 028 W11 known limit).

use event_checkin_domain::models::event::slug_taken_by_other;

const EVENTS: [(&str, &str); 2] = [("bkk-2025", "bkk-2025"), ("old-id", "renamed")];

#[test]
fn another_events_slug_is_taken() {
    assert!(slug_taken_by_other("renamed", "bkk-2025", EVENTS));
}

#[test]
fn another_events_id_is_taken_even_after_its_slug_moved() {
    // `old-id` now answers to `renamed`, but id-first lookups still reach it.
    assert!(slug_taken_by_other("old-id", "bkk-2025", EVENTS));
}

#[test]
fn an_events_own_locators_are_not_a_conflict() {
    assert!(!slug_taken_by_other("renamed", "old-id", EVENTS));
    assert!(!slug_taken_by_other("old-id", "old-id", EVENTS));
}

#[test]
fn a_fresh_slug_is_free() {
    assert!(!slug_taken_by_other("chiang-mai-2026", "bkk-2025", EVENTS));
    assert!(!slug_taken_by_other("chiang-mai-2026", "", []));
}

#[test]
fn a_new_event_conflicts_with_every_existing_locator() {
    assert!(slug_taken_by_other("bkk-2025", "", EVENTS));
    assert!(slug_taken_by_other("renamed", "", EVENTS));
}
