//! The course page (.plans/045 R4.4): which episode it opens, that the
//! player loads only on request from the no-cookie host, and that past
//! events link to their course instead of off-site.

use event_checkin_domain::models::catalogue::{Series, course_episodes};
use event_checkin_frontend::pages::site::course::current_episode;

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn opens_the_asked_then_the_first_unwatched_episode() {
    let eps = course_episodes(Series::RoadToMainnet);
    let slug = |i: usize| eps[i].slug.to_string();
    assert_eq!(current_episode(&eps, Some(4), &[]), Some(3));
    assert_eq!(current_episode(&eps, Some(99), &[]), Some(0), "unknown ep");
    assert_eq!(current_episode(&eps, None, &[slug(0), slug(1)]), Some(2));
    let all: Vec<String> = (0..eps.len()).map(slug).collect();
    assert_eq!(
        current_episode(&eps, None, &all),
        Some(0),
        "all watched: the first"
    );
    assert_eq!(current_episode(&[], None, &[]), None);
}

#[test]
fn player_waits_for_a_click_and_uses_the_no_cookie_host() {
    let src = read("src/pages/site/course.rs");
    assert!(src.contains("https://www.youtube-nocookie.com/embed/"));
    assert!(!src.contains("https://www.youtube.com/"));
    let iframe = src.find("<iframe").unwrap();
    let gate = src.find("(true, true) =>").unwrap();
    assert!(gate < iframe, "the iframe renders only once playing");
}

#[test]
fn past_events_link_to_their_course() {
    let learn = read("src/pages/site/learn.rs");
    assert!(learn.contains("format!(\"/events/{slug}?ep={}\", e.ep)"));
    assert!(!learn.contains("youtube.com/watch"));
}
