//! Meals ordered against registrations (.plans/045 R4.11): `fold_meals` sums
//! per event, counts only the on-site track, never goes below 0 per event,
//! and is `None` until an organizer enters a count.

use event_checkin_domain::models::public_stats::{MealRow, Meals, fold_meals};

fn row(event: &str, ordered: u64, pt: &str, n: u64, came: u64) -> MealRow {
    MealRow {
        event_id: event.into(),
        ordered,
        participation_type: pt.into(),
        n,
        checked_in: came,
    }
}

#[test]
fn nothing_entered_is_none() {
    assert_eq!(fold_meals(&[]), None);
    assert_eq!(fold_meals(&[row("e1", 0, "in-person", 40, 30)]), None);
}

#[test]
fn per_event_sums_on_site_only_and_floors_at_zero() {
    let rows = [
        // 50 on site (walk-ins count), 10 online: 50 registered, ordered 35
        row("e1", 35, "in-person", 45, 33),
        row("e1", 35, "walkin", 5, 5),
        row("e1", 35, "online", 10, 0),
        // ordered more than registered: 0 here, not -10
        row("e2", 30, "in-person", 20, 18),
    ];
    assert_eq!(
        fold_meals(&rows),
        Some(Meals {
            events: 2,
            registered: 70,
            came: 56,
            ordered: 65,
            not_ordered: 15,
        })
    );
}

#[test]
fn an_event_with_no_registrations_still_counts_its_order() {
    let m = fold_meals(&[row("e1", 12, "", 0, 0)]).unwrap();
    assert_eq!(
        (m.events, m.ordered, m.registered, m.not_ordered),
        (1, 12, 0, 0)
    );
}

#[test]
fn old_payloads_without_meals_still_parse() {
    let json = r#"{"measured_at":"x","events_held":1,"onsite_registrations":0,"online_registrations":0,"door_scans":0,"deposits_handled_count":0,"deposits_handled_thb":0,"deposit_payers":0,"deposit_payers_came":0}"#;
    let s: event_checkin_domain::models::public_stats::PublicStats =
        serde_json::from_str(json).unwrap();
    assert_eq!(s.meals, None);
}
