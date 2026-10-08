//! `/e/{slug}` carries the event's own social tags (`.issues/183` Part 1).
//!
//! Crawlers read only the HTML the Worker sends. These pin the splice that
//! writes the event into the stock head: which tags change, that every value
//! is escaped, the image order, the fallback, and that two events get two
//! different heads (a splice that served the active event would read green on
//! a one-event probe, as `/api/public/ticket/{id}` once did).
//!
//! The stock head is the committed `frontend-leptos/index.html`, not
//! `dist/index.html`, which is a stub until `build.sh` runs.

use std::path::Path;

use event_checkin_domain::models::event::EventConfig;
use event_checkin_worker::og_meta::{
    ImageChoice, MAX_DESCRIPTION_CHARS, OgMeta, PosterSource, RasterKind, SPLICE_TARGETS, TagAttr,
    choose_image, classify_poster, find_tag, html_escape, is_shareable, meta_for_event, page_slug,
    splice,
};

const ORIGIN: &str = "https://bethere-staging.solana-thailand.workers.dev";

fn stock_html() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../frontend-leptos/index.html");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn event(id: &str, slug: &str, name: &str) -> EventConfig {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": name,
        "slug": slug,
        "tagline": "Bring a laptop",
        "link": "",
        "status": "active",
        "event_start_ms": 1_791_802_800_000_i64,
        "event_end_ms": 1_791_810_000_000_i64,
        "sheet_id": "s",
        "sheet_name": "Attendees",
        "staff_sheet_name": "staff",
        "location": "Bangkok",
        "created_at": "",
        "updated_at": ""
    }))
    .expect("EventConfig JSON must parse")
}

fn tag_value(html: &str, attr: TagAttr, key: &str) -> Option<String> {
    find_tag(html, attr, key).map(|span| html[span.content].to_string())
}

/// The stock page carries every tag the splice rewrites, exactly once. If
/// this fails, `index.html` changed and every event page would fall back to
/// the stock card.
#[test]
fn stock_head_carries_every_target_once() {
    let html = stock_html();
    for (attr, key, _) in SPLICE_TARGETS {
        assert!(
            find_tag(&html, attr, key).is_some(),
            "frontend-leptos/index.html lost (or duplicated) the {key} tag the /e/{{slug}} splice rewrites"
        );
    }
    // The stock image is the 1200×630 card (plan 042 0.6).
    assert_eq!(
        tag_value(&html, TagAttr::Property, "og:image").as_deref(),
        Some("https://bethere.solana-thailand.workers.dev/og-image.png")
    );
}

#[test]
fn splice_changes_only_the_expected_tags() {
    let html = stock_html();
    let meta = meta_for_event(
        &event("ev-1", "rust-night", "Rust Night"),
        ORIGIN,
        ImageChoice::Card,
    );
    let out = splice(&html, &meta).expect("splice");

    let get = |key: &str, attr| tag_value(&out, attr, key).expect(key);
    assert_eq!(get("og:title", TagAttr::Property), "Rust Night · BeThere");
    assert_eq!(get("twitter:title", TagAttr::Name), "Rust Night · BeThere");
    assert_eq!(
        get("og:url", TagAttr::Property),
        format!("{ORIGIN}/e/rust-night")
    );
    assert_eq!(
        get("og:description", TagAttr::Property),
        "Mon 12 Oct 2026 · 18:00 (UTC+7) · Bangkok. Bring a laptop"
    );
    assert_eq!(
        get("og:description", TagAttr::Property),
        get("twitter:description", TagAttr::Name)
    );
    assert_eq!(
        get("og:image", TagAttr::Property),
        format!("{ORIGIN}/api/storage/og/ev-1")
    );
    assert_eq!(
        get("twitter:image", TagAttr::Name),
        format!("{ORIGIN}/api/storage/og/ev-1")
    );
    assert_eq!(get("og:image:type", TagAttr::Property), "image/png");
    assert_eq!(get("og:image:width", TagAttr::Property), "1200");
    assert_eq!(get("og:image:height", TagAttr::Property), "630");

    // Everything outside the target tags is byte-for-byte the stock page.
    let strip = |page: &str| -> String {
        let mut spans: Vec<_> = SPLICE_TARGETS
            .iter()
            .filter_map(|(attr, key, _)| find_tag(page, *attr, key))
            .map(|s| s.content)
            .collect();
        spans.sort_by_key(|r| r.start);
        let mut rest = String::new();
        let mut at = 0;
        for r in spans {
            rest.push_str(&page[at..r.start]);
            at = r.end;
        }
        rest.push_str(&page[at..]);
        rest
    };
    assert_eq!(strip(&html), strip(&out));
    // Tags outside the set keep their stock values.
    for key in ["og:site_name", "og:type", "twitter:card", "twitter:creator"] {
        let attr = match key.starts_with("og:") {
            true => TagAttr::Property,
            false => TagAttr::Name,
        };
        assert_eq!(
            tag_value(&html, attr, key),
            tag_value(&out, attr, key),
            "{key}"
        );
    }
}

#[test]
fn every_value_is_escaped() {
    let html = stock_html();
    let mut ev = event("ev-x", "x", "<script>alert(1)</script> \"Tom\" & 'Jerry'");
    ev.tagline = "a > b & \"c\"".into();
    let out = splice(&html, &meta_for_event(&ev, ORIGIN, ImageChoice::Card)).expect("splice");
    assert!(!out.contains("<script>alert"), "raw script tag leaked");
    assert!(!out.contains("\"Tom\""), "raw quote leaked");
    let title = tag_value(&out, TagAttr::Property, "og:title").expect("title");
    assert_eq!(
        title,
        "&lt;script&gt;alert(1)&lt;/script&gt; &quot;Tom&quot; &amp; &#39;Jerry&#39; · BeThere"
    );
    assert_eq!(html_escape("&amp;"), "&amp;amp;");
    // The alt carries the name too, escaped.
    let alt = tag_value(&out, TagAttr::Property, "og:image:alt").expect("alt");
    assert!(alt.starts_with("&lt;script&gt;"), "{alt}");
}

#[test]
fn two_events_get_two_heads() {
    let html = stock_html();
    let a = splice(
        &html,
        &meta_for_event(&event("ev-a", "alpha", "Alpha"), ORIGIN, ImageChoice::Card),
    )
    .expect("a");
    let mut beta = event("ev-b", "beta", "Beta");
    beta.tagline = "Bring a friend".into();
    let b = splice(&html, &meta_for_event(&beta, ORIGIN, ImageChoice::Card)).expect("b");
    for (attr, key, _) in SPLICE_TARGETS {
        if matches!(key, "og:image:type" | "og:image:width" | "og:image:height") {
            continue;
        }
        assert_ne!(tag_value(&a, attr, key), tag_value(&b, attr, key), "{key}");
    }
}

#[test]
fn stock_image_keeps_the_stock_image_tags() {
    let html = stock_html();
    let out = splice(
        &html,
        &meta_for_event(&event("ev-1", "s", "S"), ORIGIN, ImageChoice::Stock),
    )
    .expect("splice");
    for (attr, key) in [
        (TagAttr::Property, "og:image"),
        (TagAttr::Property, "og:image:type"),
        (TagAttr::Property, "og:image:width"),
        (TagAttr::Property, "og:image:alt"),
        (TagAttr::Name, "twitter:image"),
        (TagAttr::Name, "twitter:image:alt"),
    ] {
        assert_eq!(
            tag_value(&html, attr, key),
            tag_value(&out, attr, key),
            "{key}"
        );
    }
    assert_eq!(
        tag_value(&out, TagAttr::Property, "og:title").as_deref(),
        Some("S · BeThere")
    );
}

#[test]
fn a_poster_of_unknown_size_drops_width_and_height() {
    let html = stock_html();
    let image = ImageChoice::Poster {
        url: format!("{ORIGIN}/api/storage/posters/ev-1"),
        kind: RasterKind::Jpeg,
    };
    let out = splice(
        &html,
        &meta_for_event(&event("ev-1", "s", "S"), ORIGIN, image),
    )
    .expect("splice");
    assert_eq!(
        tag_value(&out, TagAttr::Property, "og:image").as_deref(),
        Some(format!("{ORIGIN}/api/storage/posters/ev-1").as_str())
    );
    assert_eq!(
        tag_value(&out, TagAttr::Property, "og:image:type").as_deref(),
        Some("image/jpeg")
    );
    assert!(!out.contains("og:image:width"));
    assert!(!out.contains("og:image:height"));
    assert_eq!(
        tag_value(&out, TagAttr::Property, "og:image:alt").as_deref(),
        Some("Poster for S")
    );
}

#[test]
fn a_page_without_the_targets_is_left_alone() {
    let meta = OgMeta {
        title: "t".into(),
        description: "d".into(),
        url: "u".into(),
        image: None,
    };
    assert_eq!(splice("<!doctype html><title>stub</title>", &meta), None);
    // A duplicated tag is ambiguous: no splice.
    let html = stock_html();
    let doubled = html.replacen(
        "<meta property=\"og:site_name\"",
        "<meta property=\"og:title\" content=\"x\" /><meta property=\"og:site_name\"",
        1,
    );
    assert_eq!(splice(&doubled, &meta), None);
}

#[test]
fn image_order_is_poster_then_card_then_stock() {
    let poster = Some(("p".to_string(), RasterKind::Png));
    assert_eq!(
        choose_image(poster.clone(), true),
        ImageChoice::Poster {
            url: "p".into(),
            kind: RasterKind::Png
        }
    );
    assert_eq!(choose_image(None, true), ImageChoice::Card);
    assert_eq!(choose_image(None, false), ImageChoice::Stock);
}

#[test]
fn only_raster_posters_are_usable() {
    assert_eq!(classify_poster(""), PosterSource::None);
    assert_eq!(
        classify_poster("/api/storage/posters/ev-9"),
        PosterSource::Stored {
            event_id: "ev-9".into()
        }
    );
    assert_eq!(
        classify_poster("https://cdn.example/a/Poster.JPG?v=2"),
        PosterSource::External {
            url: "https://cdn.example/a/Poster.JPG?v=2".into(),
            kind: RasterKind::Jpeg
        }
    );
    assert_eq!(
        classify_poster("https://cdn.example/p.png"),
        PosterSource::External {
            url: "https://cdn.example/p.png".into(),
            kind: RasterKind::Png
        }
    );
    for unusable in [
        "https://cdn.example/p.svg",
        "https://cdn.example/p.webp",
        "http://cdn.example/p.png",
        "https://cdn.example/poster",
        "/elsewhere/p.png",
        "/api/storage/posters/../slips/x",
        "https://cdn.example/p.png\"><script>",
    ] {
        assert_eq!(
            classify_poster(unusable),
            PosterSource::Unusable,
            "{unusable}"
        );
    }
}

#[test]
fn drafts_archived_and_private_events_are_not_shared() {
    let mut ev = event("ev-1", "s", "S");
    assert!(is_shareable(&ev));
    for status in ["draft", "archived"] {
        ev.status = serde_json::from_value(serde_json::json!(status)).expect("status");
        assert!(!is_shareable(&ev), "{status}");
    }
    ev.status = serde_json::from_value(serde_json::json!("completed")).expect("status");
    assert!(is_shareable(&ev));
    ev.visibility = serde_json::from_value(serde_json::json!("private")).expect("visibility");
    assert!(!is_shareable(&ev));
}

#[test]
fn event_paths_yield_their_slug() {
    assert_eq!(page_slug("/e/rust-night").as_deref(), Some("rust-night"));
    assert_eq!(page_slug("/e/rust-night/").as_deref(), Some("rust-night"));
    assert_eq!(page_slug("/e/caf%C3%A9").as_deref(), Some("café"));
    for no in ["/e/", "/e", "/e/a/b", "/discover", "/e/%20"] {
        assert_eq!(page_slug(no), None, "{no}");
    }
}

#[test]
fn long_descriptions_are_cut() {
    let mut ev = event("ev-1", "s", "S");
    ev.tagline = String::new();
    ev.description = "ยาว ".repeat(200);
    let meta = meta_for_event(&ev, ORIGIN, ImageChoice::Stock);
    assert!(meta.description.chars().count() <= MAX_DESCRIPTION_CHARS);
    assert!(meta.description.ends_with('…'));
    // Newlines in organizer text never reach the attribute.
    ev.description = "line one\nline two".into();
    let meta = meta_for_event(&ev, ORIGIN, ImageChoice::Stock);
    assert!(
        meta.description.ends_with("line one line two"),
        "{}",
        meta.description
    );
}

fn worker_source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The card the splice names is served, and only a checked PNG gets stored.
#[test]
fn card_route_and_upload_check_are_wired() {
    let routes = worker_source("src/handlers/mod.rs");
    assert!(
        routes.contains("\"/storage/og/{event_id}\", get(crate::storage::serve_og_card)"),
        "GET /api/storage/og/{{event_id}} is not routed"
    );
    let upload = worker_source("src/handlers/events/poster.rs");
    for needle in [
        "check_og_png(&bytes)",
        "storage::og_card_key(event_id)",
        "store_og_card(&state, &claims, &event.id",
    ] {
        assert!(upload.contains(needle), "poster.rs lost `{needle}`");
    }
    // The role check runs before the card branch.
    let role_at = upload.find("resolve_user_role").expect("role check");
    let card_at = upload.find("UploadKind::Og {").expect("card branch");
    assert!(
        role_at < card_at,
        "the share card branch must sit after the role check"
    );
}

#[test]
fn cards_are_cached_for_an_hour() {
    use event_checkin_worker::storage::{Visibility, og_card_key};
    assert_eq!(
        Visibility::Regenerated.cache_control(),
        "public, max-age=3600"
    );
    assert_eq!(og_card_key("ev-1"), "og/ev-1.png");
}
