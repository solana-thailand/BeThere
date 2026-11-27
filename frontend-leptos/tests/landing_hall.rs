//! The payers' hall (.plans/045 R4.2): one chair per payer of the facts
//! table, solid for those who came, blocks in table order.

use event_checkin_domain::models::facts::{LADDER, ladder_total};
use event_checkin_frontend::pages::landing::hall::{hall_seats, hall_seats_by_event};

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

#[test]
fn by_event_rows_give_each_event_its_own_row_with_no_shows_last() {
    let (seats, w, h) = hall_seats_by_event(&LADDER);
    let (paid, came) = ladder_total();
    assert_eq!(seats.len() as u32, paid, "one chair per payer");
    assert_eq!(seats.iter().filter(|s| s.came).count() as u32, came);
    for (g, row) in LADDER.iter().enumerate() {
        let mine: Vec<_> = seats.iter().filter(|s| s.group == g).collect();
        assert_eq!(mine.len() as u32, row.paid, "{}", row.event);
        // One row: every chair of the event at the same height, left to right.
        assert!(
            mine.iter().all(|s| s.y == mine[0].y),
            "{}: one row",
            row.event
        );
        assert!(mine.windows(2).all(|p| p[0].x < p[1].x), "{}", row.event);
        let first_gone = mine.iter().position(|s| !s.came).unwrap_or(mine.len());
        assert_eq!(first_gone as u32, row.came, "{}", row.event);
        assert!(mine[first_gone..].iter().all(|s| !s.came), "{}", row.event);
    }
    // Rows do not share a height, and everything fits the drawing, tail included.
    let mut ys: Vec<f32> = seats.iter().map(|s| s.y).collect();
    ys.dedup();
    assert_eq!(ys.len(), LADDER.len());
    assert!(seats.iter().all(|s| s.x + 22.0 < w && s.y + 30.0 <= h));
}

#[test]
fn the_empty_outlines_are_findable_by_colour_and_arrive_last() {
    let css = std::fs::read_to_string(format!(
        "{}/styles/style-23-landing.css",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("css");
    let at = css
        .find(".lp-hero-c .lp-hall-gone {")
        .expect("outline rule");
    assert!(css[at..].split('}').next().unwrap().contains("--lp-t-warm"));
    // Reduced motion: no animation at all.
    let anim = css
        .find("@media (prefers-reduced-motion: no-preference)")
        .expect("motion rule");
    assert!(css[anim..].contains("lp-seat"));
    let hall = std::fs::read_to_string(format!(
        "{}/src/pages/landing/hall.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("hall.rs");
    assert!(
        hall.contains("came_total + 8 * (next_gone - 1)"),
        "outlines fill in last"
    );
}
