//! ORGANIZERS on the admin event card shows who, not a bare count
//! (.plans/037 §4).

use event_checkin_frontend::pages::events_page::organizers_label;

fn emails(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("org{i}@example.com")).collect()
}

#[test]
fn none_is_a_dash() {
    assert_eq!(organizers_label(&[]), "—");
}

#[test]
fn up_to_two_are_listed() {
    assert_eq!(organizers_label(&emails(1)), "org1@example.com");
    assert_eq!(
        organizers_label(&emails(2)),
        "org1@example.com, org2@example.com"
    );
}

#[test]
fn more_than_two_name_two_and_count_the_rest() {
    assert_eq!(
        organizers_label(&emails(4)),
        "org1@example.com, org2@example.com, +2 more"
    );
}
