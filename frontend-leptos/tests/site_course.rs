//! The course page (.plans/045 R4.4): which episode it opens, that the
//! player loads only on request from the no-cookie host, that the courses
//! come from the worker (campaigns), not a list in the code.

use event_checkin_domain::models::course::CourseEpisode;
use event_checkin_frontend::pages::site::course::current_episode;

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn ep(slug: &str, end_ms: i64) -> CourseEpisode {
    CourseEpisode {
        slug: slug.into(),
        end_ms,
        ..Default::default()
    }
}

#[test]
fn opens_the_asked_then_the_first_unwatched_held_episode() {
    let now = 100;
    let eps = vec![ep("a", 10), ep("b", 20), ep("c", 30), ep("next", 200)];
    let w = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(current_episode(&eps, Some(2), &[], now), Some(1));
    assert_eq!(
        current_episode(&eps, Some(99), &[], now),
        Some(0),
        "unknown ep"
    );
    assert_eq!(current_episode(&eps, Some(0), &[], now), Some(0));
    assert_eq!(current_episode(&eps, None, &w(&["a", "b"]), now), Some(2));
    // all held ones watched: the first (the upcoming one is not "unwatched")
    assert_eq!(
        current_episode(&eps, None, &w(&["a", "b", "c"]), now),
        Some(0)
    );
    assert_eq!(current_episode(&[], None, &[], now), None);
}

#[test]
fn player_waits_for_a_click_and_uses_the_no_cookie_host() {
    let src = read("src/pages/site/course.rs");
    assert!(src.contains("https://www.youtube-nocookie.com/embed/"));
    assert!(!src.contains("https://www.youtube.com/"));
    let iframe = src.find("<iframe").unwrap();
    let gate = src.find("(false, false, true, true) =>").unwrap();
    assert!(gate < iframe, "the iframe renders only once playing");
}

#[test]
fn courses_come_from_the_worker() {
    for f in [
        "src/pages/site/course.rs",
        "src/pages/site/learn.rs",
        "src/pages/site/open_events.rs",
    ] {
        let src = read(f);
        assert!(
            !src.contains("catalogue"),
            "{f}: no course list in the code"
        );
    }
    assert!(read("src/pages/site/courses_data.rs").contains("/api/public/courses"));
}
