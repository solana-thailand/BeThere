//! Courses from campaigns (.plans/045 R4.4): the cadence line, YouTube ids
//! from what organizers paste, course ids.

use event_checkin_domain::models::course::{
    CourseEpisode, CourseSummary, busiest, cadence, is_course_id, youtube_id,
};

const DAY: i64 = 86_400_000;

#[test]
fn road_to_mainnet_dates_give_about_every_four_weeks() {
    // RTM #1–#6 start days (26 Apr … 4 Oct 2026): gaps 28, 27, 29, 35, 42
    let base = 1_777_170_600_000;
    let starts: Vec<i64> = [0, 28, 55, 84, 119, 161]
        .iter()
        .map(|d| base + d * DAY)
        .collect();
    let c = cadence(&starts).unwrap();
    assert_eq!(
        (c.times, c.every_weeks, c.last_ms),
        (6, 4, base + 161 * DAY)
    );
    // order does not matter; under three says nothing
    let mut shuffled = starts.clone();
    shuffled.reverse();
    assert_eq!(cadence(&shuffled), Some(c));
    assert_eq!(cadence(&starts[..2]), None);
}

#[test]
fn youtube_ids_from_what_organizers_paste() {
    for url in [
        "https://www.youtube.com/watch?v=gzFU1NvC3aw",
        "https://www.youtube.com/watch?v=gzFU1NvC3aw&t=42s",
        "https://youtu.be/gzFU1NvC3aw?si=abc",
        "https://www.youtube.com/embed/gzFU1NvC3aw",
        "https://www.youtube.com/live/gzFU1NvC3aw",
        "gzFU1NvC3aw",
    ] {
        assert_eq!(youtube_id(url).as_deref(), Some("gzFU1NvC3aw"), "{url}");
    }
    assert_eq!(youtube_id(""), None);
    assert_eq!(youtube_id("https://example.com/video.mp4"), None);
    assert_eq!(youtube_id("javascript:alert(1)"), None);
}

#[test]
fn course_ids_and_the_busiest_course() {
    assert!(is_course_id("road-to-mainnet"));
    assert!(!is_course_id("Road to Mainnet"));
    assert!(!is_course_id("../x"));
    assert!(!is_course_id(""));
    let c = |id: &str, held| CourseSummary {
        id: id.into(),
        held,
        ..Default::default()
    };
    assert_eq!(
        busiest(&[c("a", 2), c("b", 6), c("c", 3)]).map(|x| x.id.as_str()),
        Some("b")
    );
    let ep = CourseEpisode {
        end_ms: 10,
        ..Default::default()
    };
    assert!(ep.upcoming(9) && !ep.upcoming(10));
}
