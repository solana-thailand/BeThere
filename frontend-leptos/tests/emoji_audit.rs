//! Emoji audit — attendee-facing text uses the SVG icon set, not emoji
//! (handoff F1-c, 2026-09-29).
//!
//! Emoji rendered as UI chrome (📅 before a link, ✓ after a title, 🔴 as a
//! "live" dot) look different on every platform, read as clutter at 390 px, and
//! bypass the icon set that already exists (`src/icons/mod.rs`). A purge
//! removed them from every catalog and attendee page; this keeps them out.
//!
//! ## What is scanned
//!
//! - Every string in `locales/**/*.json`.
//! - String and char literals in the attendee-facing sources ([`ATTENDEE_SOURCES`]).
//!   Comments are skipped: a comment may name the glyph it replaced.
//!
//! ## What is not
//!
//! - Staff pages (admin, scanner, dashboards): out of scope for the purge.
//! - `pages/adventure/`: the quest game draws its map tiles and characters with
//!   emoji. That is sprite art, not decoration.
//! - Typographic arrows (`→ ↗ ←`, U+2190–U+21FF) and bullets (`•`): text, not emoji.

use std::fs;
use std::path::{Path, PathBuf};

/// Attendee-facing sources: a directory means every `.rs` file under it.
const ATTENDEE_SOURCES: &[&str] = &[
    "src/pages/landing",
    "src/pages/public_event",
    "src/pages/deposit",
    "src/pages/ticket",
    "src/pages/claim",
    "src/pages/public",
    "src/pages/login.rs",
    "src/pages/privacy.rs",
    "src/pages/faq.rs",
    "src/pages/data_privacy.rs",
    "src/pages/dev_dashboard.rs",
    "src/pages/dev_profile.rs",
    "src/pages/profile_link_result.rs",
    "src/pages/nfc_checkin.rs",
    "src/wallet_signin.rs",
    "src/components.rs",
    "src/locale.rs",
    "src/api/wallet.rs",
];

/// Whether `c` is an emoji or pictographic symbol as this audit defines it.
fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF // pictographs, emoticons, transport, symbols & pictographs ext.
        | 0x2600..=0x27BF // misc symbols + dingbats (☀ ⚠ ✓ ✕ ✨ ➔ …)
        | 0x2300..=0x23FF // misc technical (⌛ ⏰ …)
        | 0x2B00..=0x2BFF // misc symbols and arrows (⬇ ⭐ …)
        | 0xFE0F          // emoji presentation selector
    )
}

fn emoji_in(text: &str) -> Vec<char> {
    text.chars().filter(|c| is_emoji(*c)).collect()
}

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

/// String and char literal bodies on one line, comments skipped. Line-based on
/// purpose: every literal this crate renders sits on one line.
fn literals(line: &str) -> Vec<String> {
    let code = match line.trim_start().starts_with("//") {
        true => return Vec::new(),
        false => line,
    };
    let mut out = Vec::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => break,
            '"' => {
                let mut lit = String::new();
                while let Some(n) = chars.next() {
                    match n {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => lit.push(n),
                    }
                }
                out.push(lit);
            }
            '\'' => {
                // A char literal is 'x' or '\x'; anything else is a lifetime.
                let mut look = chars.clone();
                let first = look.next();
                let lit = match (first, look.next()) {
                    (Some('\\'), _) => None,
                    (Some(ch), Some('\'')) => Some(ch),
                    _ => None,
                };
                if let Some(ch) = lit {
                    chars.next();
                    chars.next();
                    out.push(ch.to_string());
                }
            }
            _ => {}
        }
    }
    out
}

fn json_strings(value: &serde_json::Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::String(s) => out.push((path.to_string(), s.clone())),
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                json_strings(v, &format!("{path}.{k}"), out);
            }
        }
        serde_json::Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                json_strings(v, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

fn locale_findings() -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root().join("locales")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read locales") {
            let path = entry.expect("entry").path();
            match path.is_dir() {
                true => stack.push(path),
                false if path.extension().is_some_and(|e| e == "json") => files.push(path),
                false => {}
            }
        }
    }
    files.sort();
    let mut findings = Vec::new();
    for file in files {
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).expect("read json")).expect("json");
        let mut strings = Vec::new();
        json_strings(&value, "", &mut strings);
        let rel = file
            .strip_prefix(root())
            .unwrap_or(&file)
            .display()
            .to_string();
        for (key, text) in strings {
            let found = emoji_in(&text);
            if !found.is_empty() {
                findings.push(format!("{rel}{key}: {found:?} in {text:?}"));
            }
        }
    }
    findings
}

fn source_findings() -> Vec<String> {
    let mut files = Vec::new();
    for rel in ATTENDEE_SOURCES {
        let path = root().join(rel);
        assert!(
            path.exists(),
            "ATTENDEE_SOURCES lists a missing path: {rel}"
        );
        rs_files(&path, &mut files);
    }
    let mut findings = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(root())
            .unwrap_or(&file)
            .display()
            .to_string();
        for (n, line) in fs::read_to_string(&file)
            .expect("read rs")
            .lines()
            .enumerate()
        {
            for lit in literals(line) {
                let found = emoji_in(&lit);
                if !found.is_empty() {
                    findings.push(format!("{rel}:{}: {found:?} in {lit:?}", n + 1));
                }
            }
        }
    }
    findings
}

#[test]
fn locales_have_no_emoji() {
    let findings = locale_findings();
    assert!(
        findings.is_empty(),
        "{} locale string(s) carry emoji. Use plain text, and put an <Icon> from \
         src/icons/mod.rs in the view if the glyph carried meaning.\n{findings:#?}",
        findings.len()
    );
}

#[test]
fn attendee_sources_have_no_emoji_literals() {
    let findings = source_findings();
    assert!(
        findings.is_empty(),
        "{} emoji literal(s) on attendee pages. Use an <Icon> from src/icons/mod.rs \
         (or CSS, as for .pe-live-dot).\n{findings:#?}",
        findings.len()
    );
}

/// The detector itself: it must catch the glyphs the purge removed and leave
/// typography and Thai alone. A detector that cannot fail is not a gate.
#[test]
fn detector_catches_emoji_and_spares_text() {
    for glyph in ["📅", "✓", "✅", "🔴", "⬇", "➔", "⚠", "🎟️", "✕", "⭐"] {
        assert!(!emoji_in(glyph).is_empty(), "missed {glyph:?}");
    }
    for text in ["→ ↗ ← ↓", "฿500", "จัดงานของคุณ →", "Nº 00123 •", "#BeThere"]
    {
        assert!(emoji_in(text).is_empty(), "false positive in {text:?}");
    }
    assert_eq!(
        literals(r#"    <span>"✓"</span> // "📅""#),
        vec!["✓".to_string()]
    );
    assert_eq!(literals("    // \"📅\" in a comment"), Vec::<String>::new());
    assert_eq!(literals("let c = '✓';"), vec!["✓".to_string()]);
}
