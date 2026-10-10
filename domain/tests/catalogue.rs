//! The past-events catalogue (.plans/045 R4.3) and the empty state's
//! cadence line computed from it.

use event_checkin_domain::models::catalogue::{CATALOGUE, PastEvent, Series, cadence, episodes};

fn ev(series: Series, ep: u32, day: i64) -> PastEvent {
    PastEvent {
        slug: "x",
        name: "x",
        series,
        ep,
        start_ms: day * 86_400_000,
        video: "",
    }
}

#[test]
fn road_to_mainnet_ran_six_times_about_every_four_weeks() {
    let c = cadence(&CATALOGUE, Series::RoadToMainnet).expect("six episodes");
    assert_eq!(c.times, 6);
    // gaps 28, 27, 29, 35, 42 days: median 29 → 4 weeks
    assert_eq!(c.every_weeks, 4);
    assert_eq!(c.last.ep, 6);
    assert!(c.last.name.contains("#6"));
}

#[test]
fn catalogue_is_ordered_numbered_and_recorded() {
    assert!(CATALOGUE.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    for series in Series::COURSES {
        let eps = episodes(&CATALOGUE, series);
        let numbers: Vec<u32> = eps.iter().map(|e| e.ep).collect();
        assert_eq!(
            numbers,
            (1..=eps.len() as u32).collect::<Vec<_>>(),
            "{series:?}"
        );
        assert!(
            eps.iter().all(|e| e.video.len() == 11),
            "{series:?}: a recording each"
        );
    }
}

#[test]
fn median_needs_three_and_takes_the_middle_gap() {
    assert_eq!(
        cadence(
            &[ev(Series::LatentSpace, 1, 0), ev(Series::LatentSpace, 2, 7)],
            Series::LatentSpace
        ),
        None
    );
    // gaps 7, 7, 21 → median 7 → 1 week; unsorted input is fine
    let three = [
        ev(Series::LatentSpace, 4, 35),
        ev(Series::LatentSpace, 1, 0),
        ev(Series::LatentSpace, 2, 7),
        ev(Series::LatentSpace, 3, 14),
    ];
    let c = cadence(&three, Series::LatentSpace).unwrap();
    assert_eq!((c.times, c.every_weeks, c.last.ep), (4, 1, 4));
    // other series do not count
    assert_eq!(cadence(&three, Series::RoadToMainnet), None);
}
