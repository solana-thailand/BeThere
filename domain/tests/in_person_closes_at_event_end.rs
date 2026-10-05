//! The in-person track closes when the event ends; the online track does not.

use event_checkin_domain::models::event::EventConfig;

const END: i64 = 1_800_000_000_000;
const DAY: i64 = 86_400_000;

fn event(end: i64, time_tba: bool) -> EventConfig {
    let mut config = EventConfig::from_global_config(
        "Ends",
        "",
        "",
        end - 3 * 3_600_000,
        end,
        "sheet",
        "Attendees",
        "staff",
        "",
        "",
        "",
        "",
        vec![],
        vec![],
        "",
        "",
    );
    config.time_tba = time_tba;
    config
}

#[test]
fn open_until_the_end_closed_from_it() {
    let e = event(END, false);
    assert!(
        !e.in_person_registration_closed(END - 1),
        "during the event"
    );
    assert!(e.in_person_registration_closed(END), "at the end");
    assert!(e.in_person_registration_closed(END + DAY), "after it");
}

#[test]
fn a_tba_event_closes_at_the_end_of_its_day() {
    let e = event(END, true);
    assert!(!e.in_person_registration_closed(END + DAY - 1));
    assert!(e.in_person_registration_closed(END + DAY));
}

#[test]
fn an_unset_end_never_closes() {
    assert!(!event(0, false).in_person_registration_closed(i64::MAX));
}
