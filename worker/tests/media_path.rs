//! `/media/*` (the landing film) is routed to the Worker so a `Range` request
//! gets a `206`: iOS Safari plays video only from a server that answers
//! ranges, and Workers static assets do not (`src/media.rs`).

use event_checkin_worker::media::{MEDIA_PREFIX, media_content_type};

#[test]
fn films_and_posters_are_served_with_their_type() {
    assert_eq!(media_content_type("/media/why-th.mp4"), Some("video/mp4"));
    assert_eq!(media_content_type("/media/why-en.jpg"), Some("image/jpeg"));
    assert_eq!(
        media_content_type("/media/why-th.vtt"),
        Some("text/vtt; charset=utf-8")
    );
}

#[test]
fn anything_else_under_media_is_not() {
    for path in [
        "/media/",
        "/media/.mp4",
        "/media/../index.html",
        "/media/a/b.mp4",
        "/media/why-th.mp4.br",
        "/media/why-th.html",
        "/media/why%20th.mp4",
        "/why-th.mp4",
        "/api/media/why-th.mp4",
    ] {
        assert_eq!(media_content_type(path), None, "{path}");
    }
}

#[test]
fn the_route_is_worker_first() {
    const WRANGLER: &str = include_str!("../wrangler.toml");
    let line = WRANGLER
        .lines()
        .find(|l| l.trim_start().starts_with("run_worker_first"))
        .expect("run_worker_first moved");
    assert!(
        line.contains(&format!("\"{MEDIA_PREFIX}*\"")),
        "run_worker_first does not route {MEDIA_PREFIX}* to the Worker"
    );
}
