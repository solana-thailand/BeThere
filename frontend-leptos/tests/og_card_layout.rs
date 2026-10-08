//! The per-event share card's pure layout (`src/utils/og_card.rs`,
//! `.issues/183`). The canvas measures text in the browser; here a line
//! "fits" when it has at most N characters, which is enough to pin the
//! wrapping rules. The real render can only be checked in a browser.

use event_checkin_frontend::utils::og_card::{
    ART, ArtSource, BAR, CardEvent, OG_HEIGHT, OG_WIDTH, TEXT_MAX_W, TEXT_X, TITLE_LINE_H,
    TITLE_MAX_LINES, TITLE_Y, WHEN_Y, art_source, card_text, clusters, cover_crop, wrap_text,
};

fn max_chars(n: usize) -> impl Fn(&str) -> bool {
    move |s: &str| s.chars().count() <= n
}

#[test]
fn short_text_stays_on_one_line() {
    assert_eq!(
        wrap_text("Rust Night", 3, &max_chars(20)),
        vec!["Rust Night"]
    );
    assert!(wrap_text("   ", 3, &max_chars(20)).is_empty());
    assert!(wrap_text("anything", 0, &max_chars(20)).is_empty());
}

#[test]
fn words_wrap_at_spaces() {
    assert_eq!(
        wrap_text("Solana Builders Night Bangkok", 3, &max_chars(15)),
        vec!["Solana Builders", "Night Bangkok"]
    );
}

#[test]
fn overflow_ends_in_an_ellipsis_that_fits() {
    let fits = max_chars(10);
    let lines = wrap_text("one two three four five six seven eight", 2, &fits);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].ends_with('…'), "{lines:?}");
    assert!(lines.iter().all(|l| fits(l)), "{lines:?}");
}

#[test]
fn thai_without_spaces_breaks_between_clusters() {
    // No spaces, as Thai is written. Tone marks and vowels above/below must
    // stay with their consonant.
    let title = "งานพบปะนักพัฒนาโซลานาที่กรุงเทพมหานคร";
    let fits = max_chars(8);
    let lines = wrap_text(title, 3, &fits);
    assert!(lines.len() > 1, "{lines:?}");
    assert!(lines.iter().all(|l| fits(l)), "{lines:?}");
    for line in &lines {
        let first = line.chars().next().expect("non-empty line");
        assert!(
            !matches!(first, '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}'),
            "line starts with a floating mark: {line:?}"
        );
    }
    // Nothing is lost when it fits in three lines.
    let roomy = wrap_text(title, 3, &max_chars(20));
    assert_eq!(roomy.concat(), title);
}

#[test]
fn clusters_keep_marks_with_their_base() {
    assert_eq!(clusters("นี่"), vec!["นี่"]);
    assert_eq!(clusters("ที่นี่"), vec!["ที่", "นี่"]);
    assert_eq!(clusters("e\u{0301}a"), vec!["e\u{0301}", "a"]);
    // A ZWJ emoji sequence is one unit.
    assert_eq!(clusters("a👩\u{200D}💻b").len(), 3);
    assert_eq!(clusters("abc").concat(), "abc");
}

#[test]
fn a_word_wider_than_the_line_is_cut() {
    let fits = max_chars(5);
    let lines = wrap_text("Supercalifragilistic", 2, &fits);
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| fits(l)), "{lines:?}");
    assert!(lines[1].ends_with('…'));
}

#[test]
fn card_text_fills_title_date_and_place() {
    let event = CardEvent {
        name: "Rust Night".into(),
        slug: "rust-night".into(),
        start_ms: 1_791_802_800_000,
        time_tba: false,
        location: "  Bangkok  ".into(),
        poster_url: String::new(),
    };
    let text = card_text(&event, &max_chars(30), &max_chars(40));
    assert_eq!(text.title, vec!["Rust Night"]);
    assert_eq!(text.when, "Mon 12 Oct 2026 · 18:00 (UTC+7)");
    assert_eq!(text.place.as_deref(), Some("Bangkok"));

    let bare = CardEvent {
        time_tba: true,
        ..Default::default()
    };
    let text = card_text(&bare, &max_chars(30), &max_chars(40));
    assert_eq!(text.title, vec!["BeThere event"]);
    assert_eq!(text.when, "Date to be announced");
    assert_eq!(text.place, None);
}

#[test]
fn the_title_never_exceeds_its_lines() {
    let event = CardEvent {
        name: "word ".repeat(40),
        ..Default::default()
    };
    let text = card_text(&event, &max_chars(12), &max_chars(40));
    assert_eq!(text.title.len(), TITLE_MAX_LINES);
}

#[test]
fn only_our_own_poster_is_drawn() {
    assert_eq!(
        art_source("/api/storage/posters/ev-1"),
        ArtSource::Uploaded("/api/storage/posters/ev-1".into())
    );
    // External art would taint the canvas and block toBlob.
    assert_eq!(
        art_source("https://cdn.example/p.png"),
        ArtSource::Generative
    );
    assert_eq!(art_source(""), ArtSource::Generative);
}

#[test]
fn cover_crop_fills_without_distortion() {
    // A tall 1000×2000 poster into the 4:5 box: full width, centred crop.
    let (sx, sy, sw, sh) = cover_crop(1000.0, 2000.0, 320.0, 400.0);
    assert_eq!((sx, sw), (0.0, 1000.0));
    assert!((sw / sh - 320.0 / 400.0).abs() < 1e-9);
    assert!((sy - (2000.0 - sh) / 2.0).abs() < 1e-9);
    // A wide one: full height.
    let (sx, sy, sw, sh) = cover_crop(1600.0, 400.0, 320.0, 400.0);
    assert_eq!((sy, sh), (0.0, 400.0));
    assert!((sx - (1600.0 - sw) / 2.0).abs() < 1e-9);
    // Degenerate sizes never divide by zero.
    assert_eq!(cover_crop(0.0, 0.0, 320.0, 400.0), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn the_layout_stays_inside_the_card() {
    let (w, h) = (f64::from(OG_WIDTH), f64::from(OG_HEIGHT));
    assert_eq!((OG_WIDTH, OG_HEIGHT), (1200, 630));
    assert!(ART.x + ART.w <= w && ART.y + ART.h <= h);
    assert!(
        (ART.w / ART.h - 0.8).abs() < 1e-9,
        "the art keeps the posters' 4:5"
    );
    assert!(TEXT_X + TEXT_MAX_W < ART.x, "text runs into the art");
    let last_title = TITLE_Y + TITLE_LINE_H * (TITLE_MAX_LINES as f64 - 1.0);
    assert!(
        last_title < WHEN_Y - 40.0,
        "the title's last line hits the date"
    );
    assert!(BAR.y + BAR.h <= h);
}
