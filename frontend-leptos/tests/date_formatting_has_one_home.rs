//! Event dates must be formatted in one place (`.issues/104`).
//!
//! `js_sys::Date::to_locale_string` with no options renders the browser's
//! default — `8/24/2026, 1:00:00 PM`. Seconds are noise on an event date, and a
//! month-first order is ambiguous to the Thai-majority audience this is written
//! for. That exact call was written **three** times, in three surfaces, and was
//! wrong in all three:
//!
//! - the landing page's upcoming-event card
//! - the landing page's "Your Events" list
//! - (`/discover` and `/feedback` were written later, against the helper)
//!
//! Each was found and fixed separately, days apart, by someone looking at a
//! screenshot. That is the shape of defect this repo keeps producing — a rule
//! copied per call site, diverging silently — so pin it instead of fixing it a
//! fourth time.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under a directory, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => rust_files(&path, out),
            false => {
                if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }
}

#[test]
fn only_utils_formats_dates() {
    let src = crate_root().join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(!files.is_empty(), "no sources found under {src:?}");

    let mut offenders = Vec::new();
    for path in files {
        // The one home. Everything else asks it.
        if path.ends_with("utils/mod.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Prose about the pattern must not trip a rule about code.
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        if code.contains("to_locale_string") {
            offenders.push(path.strip_prefix(crate_root()).unwrap().to_path_buf());
        }
    }

    assert!(
        offenders.is_empty(),
        "date formatting belongs in `utils` — use `format_event_day` or \
         `format_event_datetime` rather than calling `to_locale_string` here. \
         Offenders: {offenders:?}"
    );
}
