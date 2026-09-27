//! A brand-new event has no D1 attendee rows, so the attendee read behind
//! registration falls back to the event's sheet. An unreachable sheet used to
//! 500 the very first registration, from both the duplicate check and the
//! capacity check. Both now go through `register::attendees`, which re-reads
//! D1 when the sheet fails and trusts it, empty or not. The D1
//! UNIQUE(event_id, lower(email)) insert stays the real duplicate guard.

const HELPER: &str = include_str!("../src/handlers/register/attendees.rs");
const SIGNUP: &str = include_str!("../src/handlers/register/signup.rs");
const CAPACITY: &str = include_str!("../src/handlers/register/capacity.rs");

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
    for (name, src) in [("signup.rs", SIGNUP), ("capacity.rs", CAPACITY)] {
        assert!(
            src.contains("super::attendees::registration_attendees("),
            "{name} must read attendees through registration_attendees"
        );
        assert!(
            !src.contains("crate::sheets::get_attendees_for_event(")
                && !src.contains("sheets::get_attendees_for_event("),
            "{name} must not call the sheet-falling-back read directly"
        );
    }
}

#[test]
fn the_unique_index_backstop_is_still_mapped_to_already_registered() {
    assert!(
        SIGNUP.contains("e.to_ascii_uppercase().contains(\"UNIQUE\")"),
        "the D1 unique violation must still read as already-registered"
    );
}
