//! The scanner looks an attendee up in the event selected in the scanner, not
//! in the server's active event. The attendee id is global (`.issues/153`):
//! without `event_id` the lookup could read one event while the check-in is
//! written to another.

const PAGE: &str = include_str!("../src/pages/scanner/page.rs");
const LOGIC: &str = include_str!("../src/pages/scanner/logic.rs");

#[test]
fn every_scanner_lookup_passes_the_selected_event() {
    let calls: Vec<&str> = PAGE.split("process_attendee_id(").skip(1).collect();
    assert_eq!(calls.len(), 3, "camera, ?scan= and manual entry");
    for call in calls {
        let args = call.split(')').next().unwrap_or_default();
        assert!(
            args.contains("active_event_id.get_untracked("),
            "a process_attendee_id call does not pass the selected event: {args}"
        );
    }
}

#[test]
fn the_lookup_forwards_the_event_instead_of_none() {
    assert!(!LOGIC.contains("get_attendee(&attendee_id, None)"));
    assert!(!LOGIC.contains("lookup_and_classify(attendee_id, None"));
    assert!(LOGIC.contains("lookup_and_classify(attendee_id, event_id, "));
}
