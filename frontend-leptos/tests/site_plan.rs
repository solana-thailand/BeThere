//! The organizers' planning tile (.plans/045): the room it draws is the
//! guide's numbers, and typing out of range never changes it.

use event_checkin_domain::models::facts::expected_came;
use event_checkin_frontend::pages::site::plan::{
    PLAN_DEFAULT, PLAN_MAX, PLAN_MIN, parse_registered, plan_seats,
};

#[test]
fn room_has_one_chair_per_registrant_and_the_guide_solid() {
    for v in [PLAN_MIN, 9, PLAN_DEFAULT, 61, PLAN_MAX] {
        let (seats, w, h) = plan_seats(v);
        assert_eq!(seats.len() as u32, v);
        let came = seats.iter().filter(|s| s.came).count() as u32;
        assert_eq!(came, expected_came(v), "{v}");
        // the solid ones first, the empty seats at the end
        assert!(seats.windows(2).all(|p| p[0].came || !p[1].came));
        assert!(seats.iter().all(|s| s.x < w && s.y < h));
    }
    // 15 to a row up to 60, 20 above
    assert_eq!(plan_seats(60).1, 15.0 * 30.0);
    assert_eq!(plan_seats(61).1, 20.0 * 30.0);
}

#[test]
fn only_in_range_input_counts() {
    assert_eq!(parse_registered("60"), Some(60));
    assert_eq!(parse_registered(" 45.4 "), Some(45));
    assert_eq!(parse_registered("0"), None);
    assert_eq!(parse_registered("201"), None);
    assert_eq!(parse_registered(""), None);
    assert_eq!(parse_registered("e"), None);
}
