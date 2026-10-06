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

/// The page asks for `?v=N` (hero.rs `FILM_REV`) and the worker keys its
/// cache on `v=N` (media.rs `CACHE_KEY_VERSION`): a re-render bumps both, or
/// browsers keep the old film for a day while the edge serves the new one.
#[test]
fn film_revision_matches_the_cache_key() {
    let root = env!("CARGO_MANIFEST_DIR");
    let read = |rel: &str| std::fs::read_to_string(format!("{root}/{rel}")).unwrap();
    let quoted = |src: &str, decl: &str| {
        let rest = &src[src.find(decl).unwrap_or_else(|| panic!("{decl}")) + decl.len()..];
        rest.split('"').nth(1).unwrap().to_string()
    };
    let worker = quoted(&read("src/media.rs"), "const CACHE_KEY_VERSION");
    let page = quoted(
        &read("../frontend-leptos/src/pages/landing/hero.rs"),
        "const FILM_REV",
    );
    assert_eq!(page, worker);
}
