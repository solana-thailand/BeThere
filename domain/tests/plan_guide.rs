//! The organizers' planning guide applies RTM #1's rate (25 of 45 came).

use event_checkin_domain::models::facts::{ROOM, expected_came, room_total};

#[test]
fn room_total_is_rtm1() {
    assert_eq!(room_total(), (45, 25));
    assert_eq!(room_total().0, ROOM.iter().map(|g| g.n).sum::<u32>());
}

#[test]
fn guide_rounds_rtm1_rate_half_up() {
    // The prototype's default: 60 registered → ≈33.
    assert_eq!(expected_came(60), 33);
    assert_eq!(expected_came(45), 25);
    assert_eq!(expected_came(1), 1);
    assert_eq!(expected_came(9), 5);
    assert_eq!(expected_came(200), 111);
    // never more than registered
    assert!((1..=200).all(|v| expected_came(v) <= v));
}
