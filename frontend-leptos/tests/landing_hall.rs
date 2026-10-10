//! The payers' hall (.plans/045 R4.2): one chair per payer of the facts
//! table, solid for those who came, blocks in table order.

use event_checkin_domain::models::facts::{LADDER, ladder_total};
use event_checkin_frontend::pages::landing::hall::hall_seats;

#[test]
fn hall_total_is_the_ladder_total() {
    let (seats, _, _) = hall_seats(&LADDER);
    let (paid, came) = ladder_total();
    assert_eq!(seats.len() as u32, paid, "one chair per payer");
    assert_eq!(
        seats.iter().filter(|s| s.came).count() as u32,
        came,
        "solid = came"
    );
    assert_eq!((paid, came), (94, 90));
}

#[test]
fn each_event_sits_together_with_its_no_shows_last() {
    let (seats, _, _) = hall_seats(&LADDER);
    for (g, row) in LADDER.iter().enumerate() {
        let block: Vec<bool> = seats
            .iter()
            .filter(|s| s.group == g)
            .map(|s| s.came)
            .collect();
        assert_eq!(block.len() as u32, row.paid, "{}", row.event);
        let first_gone = block.iter().position(|c| !c).unwrap_or(block.len());
        assert!(
            block[first_gone..].iter().all(|c| !c),
            "{}: no-shows at the end",
            row.event
        );
        assert_eq!(first_gone as u32, row.came, "{}", row.event);
    }
    let groups: Vec<usize> = seats.iter().map(|s| s.group).collect();
    assert!(
        groups.windows(2).all(|w| w[0] <= w[1]),
        "blocks keep table order"
    );
}

#[test]
fn the_drawing_fits_its_seats() {
    let (seats, w, h) = hall_seats(&LADDER);
    assert!(seats.iter().all(|s| s.x < w && s.y < h));
}
