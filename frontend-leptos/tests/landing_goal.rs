//! The goal (.plans/043 L10): the bar never lies about its numbers, the data
//! file the page loads has the shape it reads, and the globe stays off the
//! first load.

use event_checkin_frontend::pages::landing::goal::{
    GLOBE_DATA_URL, GLOBE_JS_URL, GlobeSummary, goal_bar_percent,
};

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn bar_is_visible_never_full_past_the_figures() {
    assert_eq!(goal_bar_percent(0, 0), 0.0);
    // 13 of 692 is 1.9 %: drawn as it is.
    assert!((goal_bar_percent(13, 692) - 1.878).abs() < 0.01);
    // Too thin to see is drawn at the floor; the label carries the figure.
    assert_eq!(goal_bar_percent(1, 692), 1.5);
    assert_eq!(goal_bar_percent(900, 692), 100.0);
}

#[test]
fn committed_data_parses_and_is_dated() {
    let text = read("globe/globe-data.json");
    let summary: GlobeSummary = serde_json::from_str(&text).unwrap();
    assert!(summary.solana_events > 0);
    let parts: Vec<&str> = summary.measured_at.split('-').collect();
    assert_eq!(parts.len(), 3, "{}", summary.measured_at);
    // What the globe reads, per scripts/globe_data_import.py.
    let full: serde_json::Value = serde_json::from_str(&text).unwrap();
    for key in ["home", "events", "countries", "land", "source"] {
        assert!(!full[key].is_null(), "{key}");
    }
    // Names and dots only (rule 5): no Luma ids, no links.
    assert!(
        !text.contains("lu.ma") && !text.contains("luma.com/"),
        "links in data"
    );
    for c in full["countries"].as_object().unwrap().values() {
        for e in c["events"].as_array().unwrap() {
            assert_eq!(e.as_array().unwrap().len(), 4, "{e}");
        }
    }
}

#[test]
fn globe_is_lazy_and_served_from_the_copied_dir() {
    let index = read("index.html");
    assert!(index.contains(r#"<link data-trunk rel="copy-dir" href="globe" />"#));
    assert!(
        !index.contains("globe.js\""),
        "globe.js must not be a first-load tag"
    );
    assert!(GLOBE_DATA_URL.starts_with("/globe/"));
    assert!(GLOBE_JS_URL.starts_with("/globe/globe.js?v="));
    // The bridge globe.js promises is the one goal.rs calls.
    assert!(read("globe/globe.js").contains("window.bethereGlobe = { mount: mount }"));
    assert!(read("src/pages/landing/goal.rs").contains("\"bethereGlobe\""));
}
