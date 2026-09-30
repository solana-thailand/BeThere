//! Inline text colours use palette tokens, not the old slate greys
//! (`.plans/037` §5, 2026-09-29).
//!
//! Inline `color: #94a3b8` and friends predate the Night Edition palette.
//! They never follow a token change, so text drifted from the rest of the
//! page. Each has a token: [`BANNED`] names it. This keeps them from coming
//! back.
//!
//! Only the `color` property is checked (`border-color`, `background-color`
//! and SVG `fill`/`stroke` are not). Brand colours (Solana green/purple,
//! wallet and Google marks) and data colours (transaction kinds) are not in
//! [`BANNED`] and stay allowed.

use std::fs;
use std::path::{Path, PathBuf};

/// Hardcoded text colour → the token that replaces it.
const BANNED: &[(&str, &str)] = &[
    ("#94a3b8", "var(--text-muted)"),
    ("#64748b", "var(--text-muted)"),
    ("#cbd5e1", "var(--text-secondary)"),
    ("#fff", "var(--text-primary)"),
    ("#ffffff", "var(--text-primary)"),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rs_files(path: &Path, out: &mut Vec<PathBuf>) {
    match path.is_dir() {
        true => {
            let mut entries: Vec<_> = fs::read_dir(path)
                .expect("read dir")
                .map(|e| e.expect("dir entry").path())
                .collect();
            entries.sort();
            for entry in entries {
                rs_files(&entry, out);
            }
        }
        false if path.extension().is_some_and(|e| e == "rs") => out.push(path.to_path_buf()),
        false => {}
    }
}

/// Every hex value assigned to a bare `color` property in `text`.
fn text_colours(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find("color:") {
        let start = from + pos;
        from = start + "color:".len();
        let prefixed = start > 0 && {
            let prev = bytes[start - 1];
            prev == b'-' || prev == b'_' || prev.is_ascii_alphanumeric()
        };
        if prefixed {
            continue;
        }
        let value = text[from..].trim_start();
        let Some(hex) = value.strip_prefix('#') else {
            continue;
        };
        let digits: String = hex.chars().take_while(char::is_ascii_hexdigit).collect();
        found.push(format!("#{}", digits.to_ascii_lowercase()));
    }
    found
}

fn banned_token(colour: &str) -> Option<&'static str> {
    BANNED
        .iter()
        .find(|(hex, _)| *hex == colour)
        .map(|(_, token)| *token)
}

#[test]
fn matcher_finds_bare_color_only() {
    let sample =
        r#"style="color:#94A3B8; border-color:#fff; background-color: #fff; color: #FFF;""#;
    assert_eq!(text_colours(sample), ["#94a3b8", "#fff"]);
    assert_eq!(
        text_colours("color: var(--text-muted)"),
        Vec::<String>::new()
    );
}

#[test]
fn planted_slate_grey_is_caught() {
    let planted = r#"<p style="font-size: 0.8rem; color: #64748b;">"#;
    let hits: Vec<_> = text_colours(planted)
        .into_iter()
        .filter_map(|c| banned_token(&c))
        .collect();
    assert_eq!(hits, ["var(--text-muted)"]);
}

#[test]
fn no_hardcoded_neutral_text_colours_in_sources() {
    let mut files = Vec::new();
    rs_files(&root().join("src"), &mut files);
    assert!(files.len() > 50, "scanned too few files: {}", files.len());

    let mut offenders = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("read source");
        for (line_no, line) in text.lines().enumerate() {
            for colour in text_colours(line) {
                if let Some(token) = banned_token(&colour) {
                    let rel = file.strip_prefix(root()).unwrap_or(file).display();
                    offenders.push(format!("{rel}:{}: color {colour} → {token}", line_no + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "hardcoded neutral text colours; use the palette token:\n{}",
        offenders.join("\n")
    );
}
