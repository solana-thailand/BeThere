//! Course rows from D1 into the shapes the site renders (.plans/045 R4.4):
//! held episodes counted per course, upcoming ones kept on the page, and
//! the recording reduced to a YouTube id.

use event_checkin_worker::courses::{fold_course, fold_courses};
use serde_json::json;

#[test]
fn courses_count_held_episodes_and_keep_their_dates() {
    let now = 10_000;
    let rows = vec![
        json!({"id": "latent", "title": "Latent", "description": "", "start_ms": 100, "end_ms": 200}),
        json!({"id": "rtm", "title": "RTM", "description": "d", "start_ms": 1000, "end_ms": 2000}),
        json!({"id": "rtm", "title": "RTM", "description": "d", "start_ms": 3000, "end_ms": 4000}),
        json!({"id": "rtm", "title": "RTM", "description": "d", "start_ms": 20000, "end_ms": 30000}),
    ];
    let courses = fold_courses(&rows, now);
    assert_eq!(courses.len(), 2);
    assert_eq!((courses[0].id.as_str(), courses[0].held), ("latent", 1));
    assert_eq!(courses[1].held, 2, "the upcoming RTM is not held yet");
    assert_eq!(courses[1].held_starts_ms, vec![1000, 3000]);
}

#[test]
fn a_course_page_keeps_every_episode_and_only_youtube_ids() {
    let rows = vec![
        json!({"id": "rtm", "title": "RTM", "description": "", "slug": "rtm-1", "name": "RTM #1", "start_ms": 1, "end_ms": 2, "video_url": "https://www.youtube.com/watch?v=gzFU1NvC3aw"}),
        json!({"id": "rtm", "title": "RTM", "description": "", "slug": "rtm-2", "name": "RTM #2", "start_ms": 3, "end_ms": 4, "video_url": "https://example.com/x.mp4"}),
    ];
    let c = fold_course(&rows).unwrap();
    assert_eq!(c.episodes.len(), 2);
    assert_eq!(c.episodes[0].video, "gzFU1NvC3aw");
    assert_eq!(c.episodes[1].video, "", "not a YouTube link: no player");
    assert!(fold_course(&[]).is_none());
}
