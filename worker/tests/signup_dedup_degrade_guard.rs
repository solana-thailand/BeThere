//! A brand-new event has no D1 attendee rows, so the signup dedup read falls
//! back to the event's sheet. An unreachable sheet used to 500 the very first
//! registration. With D1 present the dedup now degrades to an empty list and
//! the D1 UNIQUE(event_id, lower(email)) insert stays the real guard, so this
//! pins both halves: the degrade exists, and the unique-index backstop that
//! makes it safe is still there.

const SIGNUP: &str = include_str!("../src/handlers/register/signup.rs");

#[test]
fn an_unreadable_sheet_degrades_only_when_d1_is_there() {
    let degrade = SIGNUP
        .find("Err(e) if state.d1.is_some() => {")
        .expect("the dedup read must degrade when D1 can back it");
    let rest = &SIGNUP[degrade..];
    let arm = &rest[..rest.find("}\n").expect("end of arm")];
    assert!(arm.contains("Vec::new()"), "degrade to an empty list");
    assert!(
        SIGNUP[degrade..].contains("failed to check existing registrations"),
        "without D1 the read still fails the request"
    );
}

#[test]
fn the_unique_index_backstop_is_still_mapped_to_already_registered() {
    let degrade = SIGNUP.find("Err(e) if state.d1.is_some() => {").unwrap();
    let backstop = SIGNUP
        .find("e.to_ascii_uppercase().contains(\"UNIQUE\")")
        .expect("the D1 unique violation must still read as already-registered");
    assert!(degrade < backstop, "the dedup read comes before the insert");
}
