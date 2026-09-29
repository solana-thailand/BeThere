//! A brand-new event has no D1 attendee rows, so the attendee read behind
//! registration falls back to the event's sheet. An unreachable sheet used to
//! 500 the very first registration, from both the duplicate check and the
//! capacity check. The duplicate check goes through `register::attendees`,
//! which re-reads D1 when the sheet fails and trusts it, empty or not. The
//! capacity check counts through `handlers::capacity::count_tracks` (plan 028
//! W3), which asks D1 first and only reads the sheet when D1 has no rows, so an
//! unreadable sheet then counts none. The D1 UNIQUE(event_id, lower(email))
//! insert stays the real duplicate guard.

const HELPER: &str = include_str!("../src/handlers/register/attendees.rs");
const SIGNUP: &str = include_str!("../src/handlers/register/signup.rs");
const CAPACITY: &str = include_str!("../src/handlers/register/capacity.rs");
const COUNT: &str = include_str!("../src/handlers/capacity.rs");

#[test]
fn a_sheet_failure_falls_back_to_a_real_d1_read() {
    let sheet = HELPER
        .find("crate::sheets::get_attendees_for_event(")
        .expect("normal read first");
    let d1 = HELPER
        .find("crate::db::attendees::get_attendees_by_event(d1, &config.id)")
        .expect("D1 re-read on a sheet failure");
    assert!(sheet < d1);
    assert!(
        HELPER.contains("return Err(sheet_err);"),
        "without D1 the sheet error still fails the request"
    );
    assert!(
        HELPER[d1..].contains(".map_err(") && HELPER[d1..].contains(")?;"),
        "a D1 failure on top must propagate, not read as empty"
    );
}

#[test]
fn both_registration_checks_use_the_helper() {
    assert!(
        SIGNUP.contains("super::attendees::registration_attendees("),
        "signup.rs must read attendees through registration_attendees"
    );
    assert!(
        CAPACITY.contains("crate::handlers::capacity::count_tracks_for_cap("),
        "capacity.rs must count through handlers::capacity"
    );
    for (name, src) in [("signup.rs", SIGNUP), ("capacity.rs", CAPACITY)] {
        assert!(
            !src.contains("sheets::get_attendees_for_event("),
            "{name} must not call the sheet-falling-back read directly"
        );
    }
}

#[test]
fn the_count_trusts_an_empty_d1_when_the_sheet_fails() {
    let empty = COUNT
        .find("Ok(None) => {")
        .expect("a branch for an event with no D1 rows");
    let branch = &COUNT[empty
        ..COUNT[empty..]
            .find("Err(e) => e,")
            .map_or(COUNT.len(), |i| empty + i)];
    assert!(
        branch.contains("Err(e) =>") && branch.contains("Ok(TrackCounts::default())"),
        "with no D1 rows, an unreadable sheet must count none, not fail registration"
    );
}

#[test]
fn the_unique_index_backstop_is_still_mapped_to_already_registered() {
    assert!(
        SIGNUP.contains("e.to_ascii_uppercase().contains(\"UNIQUE\")"),
        "the D1 unique violation must still read as already-registered"
    );
}
