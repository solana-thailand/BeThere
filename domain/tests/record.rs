//! `door_history` (Super GOAT SG1), the spec's acceptance: virtual
//! check-ins excluded, staff excluded, merged emails counted once, online
//! excluded, missed events kept as `came: false`, oldest first. (Smoke events
//! never reach it: the SQL takes campaign events only, test_record.py.)

use event_checkin_domain::models::record::{
    Attended, DoorRow, MyRecord, VIRTUAL_CHECKIN, door_checkins, door_history, settled,
};

/// After every fixture event has ended.
const NOW: i64 = 1_000;

fn row(slug: &str, start: i64, came: bool) -> DoorRow {
    DoorRow {
        event_slug: slug.into(),
        event_name: slug.to_uppercase(),
        start_ms: start,
        end_ms: start + 1,
        participation_type: "in_person".into(),
        checked_in: came,
        checked_in_by: if came {
            "door@x.example".into()
        } else {
            String::new()
        },
        crew: false,
    }
}

fn slugs_came(h: &[Attended]) -> Vec<(&str, bool)> {
    h.iter().map(|a| (a.event_slug.as_str(), a.came)).collect()
}

#[test]
fn virtual_checkin_is_not_a_door() {
    let mut r = row("rtm1", 1, true);
    r.checked_in_by = VIRTUAL_CHECKIN.into();
    let h = door_history(&[r], NOW);
    assert_eq!(slugs_came(&h), [("rtm1", false)]);
    assert_eq!(door_checkins(&h), 0);
}

#[test]
fn staff_and_online_rows_are_out_walkins_in() {
    let mut crew = row("rtm1", 1, true);
    crew.crew = true;
    let mut online = row("rtm2", 2, true);
    online.participation_type = "online".into();
    let mut walkin = row("rtm3", 3, true);
    walkin.participation_type = "walkin".into();
    assert_eq!(
        slugs_came(&door_history(&[crew, online, walkin], NOW)),
        [("rtm3", true)]
    );
}

#[test]
fn merged_emails_count_once_and_came_if_any_came() {
    // the same event under two linked emails: one entry, came once
    let h = door_history(
        &[
            row("rtm1", 1, false),
            row("rtm1", 1, true),
            row("rtm2", 2, true),
        ],
        NOW,
    );
    assert_eq!(slugs_came(&h), [("rtm1", true), ("rtm2", true)]);
    assert_eq!(door_checkins(&h), 2);
}

#[test]
fn missed_stays_and_order_is_by_date() {
    let h = door_history(
        &[
            row("rtm3", 30, true),
            row("rtm1", 10, false),
            row("rtm2", 20, true),
        ],
        NOW,
    );
    assert_eq!(
        slugs_came(&h),
        [("rtm1", false), ("rtm2", true), ("rtm3", true)]
    );
    assert_eq!(door_checkins(&h), 2);
}

#[test]
fn record_payload_parses_without_badges() {
    let r: MyRecord = serde_json::from_str(r#"{"events":[],"door_checkins":0}"#).unwrap();
    assert!(r.badges.is_empty());
}

#[test]
fn not_ended_without_a_checkin_is_upcoming_not_missed() {
    let h = door_history(&[row("rtm1", 10, true), row("rtm2", 2_000, false)], NOW);
    assert_eq!(slugs_came(&h), [("rtm1", true), ("rtm2", false)]);
    assert!(!h[0].upcoming && h[1].upcoming);
    assert_eq!(settled(&h), 1);
    // checked in at an event still running: came, not upcoming
    let h = door_history(&[row("rtm3", 2_000, true)], NOW);
    assert!(h[0].came && !h[0].upcoming);
}
