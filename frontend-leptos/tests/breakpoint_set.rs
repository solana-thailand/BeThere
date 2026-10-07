//! Width media queries use one breakpoint set (`.plans/037`, 2026-10-08).
//!
//! The stylesheets had grown 13 widths (359, 380, 420, 480, 560, 600, 640,
//! 641, 720, 767, 768, 860, 900). Neighbouring rules disagreed by a few
//! pixels, so a page could be half "phone" and half "tablet" at the same
//! width: at exactly 768px `(max-width: 768px)` and `(min-width: 768px)` both
//! matched. Every width query outside [`EXEMPT`] now uses one of the
//! canonical edges below, and this keeps new ones on the grid.
//!
//! Height, orientation, hover and pointer queries are not checked.

use std::fs;
use std::path::PathBuf;

/// `max-width` edges: small phone, phone, below tablet.
const MAX_WIDTHS: &[u32] = &[359, 480, 767];
/// `min-width` edges: the first pixel above a phone, tablet and up.
const MIN_WIDTHS: &[u32] = &[481, 768];

/// Files not yet moved to the set. The landing page is rewritten by release 3
/// (pull 157); its queries move after that merges (`.plans/037`).
const EXEMPT: &[&str] = &["style-23-landing.css"];

fn styles_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("styles")
}

/// Every `(max-width: Npx)` / `(min-width: Npx)` inside an `@media` prelude,
/// as `(feature, px)`.
fn width_queries(css: &str) -> Vec<(&'static str, u32)> {
    let mut found = Vec::new();
    for line in css.lines() {
        let Some(prelude) = line.trim_start().strip_prefix("@media") else {
            continue;
        };
        for feature in ["max-width", "min-width"] {
            let mut rest = prelude;
            while let Some(pos) = rest.find(feature) {
                rest = &rest[pos + feature.len()..];
                let value = rest.trim_start_matches([':', ' ']);
                let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
                if let Ok(px) = digits.parse() {
                    found.push((feature, px));
                }
            }
        }
    }
    found
}

fn allowed(feature: &str, px: u32) -> bool {
    match feature {
        "max-width" => MAX_WIDTHS.contains(&px),
        "min-width" => MIN_WIDTHS.contains(&px),
        _ => true,
    }
}

#[test]
fn matcher_reads_width_features_in_media_preludes_only() {
    let css = "@media (min-width: 481px) and (max-width: 767px) {\n\
               .a { max-width: 640px; }\n\
               }\n\
               @media (max-height: 500px) and (orientation: landscape) {}\n\
               @media (max-width:720px) {}";
    assert_eq!(
        width_queries(css),
        vec![("max-width", 767), ("min-width", 481), ("max-width", 720)]
    );
}

#[test]
fn off_grid_widths_are_rejected() {
    assert!(allowed("max-width", 767));
    assert!(allowed("min-width", 768));
    assert!(!allowed("max-width", 768), "768 is a min edge; max is 767");
    assert!(!allowed("max-width", 640));
    assert!(!allowed("min-width", 420));
}

#[test]
fn exempt_files_exist() {
    for name in EXEMPT {
        assert!(
            styles_dir().join(name).is_file(),
            "{name} is exempt but gone; drop it from EXEMPT"
        );
    }
}

#[test]
fn width_queries_use_the_breakpoint_set() {
    let mut entries: Vec<_> = fs::read_dir(styles_dir())
        .expect("read styles/")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "css"))
        .collect();
    entries.sort();
    let mut checked = 0;
    let mut off_grid = Vec::new();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if EXEMPT.contains(&name.as_str()) {
            continue;
        }
        let css = fs::read_to_string(&path).expect("read stylesheet");
        for (feature, px) in width_queries(&css) {
            checked += 1;
            if !allowed(feature, px) {
                off_grid.push(format!("{name}: ({feature}: {px}px)"));
            }
        }
    }
    assert!(
        checked > 20,
        "only {checked} width queries found; is the scan broken?"
    );
    assert!(
        off_grid.is_empty(),
        "width queries off the 359/480/767 (max) and 481/768 (min) set:\n{}",
        off_grid.join("\n")
    );
}
