//! Regression guard: A1 ranges must not interpolate a raw sheet name.
//!
//! Google's A1 grammar accepts a bare sheet name only when it is a plain
//! identifier. `sheet_name` and `staff_sheet_name` are free-text fields on the
//! event form, so `format!("{sheet_name}!A2:R")` produced `Attendee List!A2:R`
//! for any organiser who put a space in the tab name — which the API rejects
//! with `INVALID_ARGUMENT`. Nearly every Sheets call in the worker is detached
//! best-effort work whose errors are logged and dropped, so the tab just
//! silently never filled in.
//!
//! `crate::sheets::a1::sheet_ref` is the fix, and it only helps if it is used
//! everywhere. The pattern that reintroduces the bug is textual and easy to
//! copy from a neighbouring line, so it is checked textually.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test a1_range_guard
//! ```

use std::fs;
use std::path::{Path, PathBuf};

const WORKER_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Interpolations that may not be followed by the `!` of an A1 range.
///
/// `sheet_ref` is the sanctioned one and is deliberately absent.
const BANNED_RANGE_PREFIXES: &[&str] = &["{sheet_name}!", "{staff_sheet_name}!"];

#[test]
fn a1_ranges_do_not_interpolate_a_raw_sheet_name() {
    let src = PathBuf::from(WORKER_ROOT).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(
        files.len() > 50,
        "expected to scan the whole worker source tree, found only {} files under {} — \
         the walker is broken and this guard would pass vacuously",
        files.len(),
        src.display()
    );

    let mut violations = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file).expect("read worker source file");
        for (n, line) in source.lines().enumerate() {
            // Skip comments — the modules that document this rule quote the
            // banned pattern in order to name it.
            if line.trim_start().starts_with("//") {
                continue;
            }
            for pat in BANNED_RANGE_PREFIXES {
                if line.contains(pat) {
                    violations.push(format!(
                        "{}:{}: `{pat}` — {}",
                        relative_path(file),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{} A1 range(s) interpolate a raw sheet name:\n{}\n\nBind \
         `let sheet_ref = a1::sheet_ref(sheet_name);` and interpolate `{{sheet_ref}}` \
         instead. Use the raw name only for cache keys, gid lookups, and comparisons \
         against titles returned by the API.",
        violations.len(),
        violations.join("\n")
    );
}

/// Every use of the raw name that survives is in one of those sanctioned roles.
///
/// Without this, the guard above could be satisfied by deleting the Sheets code
/// entirely, and `sheet_ref` could fall out of use without anything noticing.
#[test]
fn the_sheets_module_actually_uses_sheet_ref() {
    let src = PathBuf::from(WORKER_ROOT).join("src").join("sheets");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);

    let uses = files
        .iter()
        .filter(|f| !f.ends_with("a1.rs"))
        .filter(|f| {
            fs::read_to_string(f)
                .expect("read worker source file")
                .contains("a1::sheet_ref(")
        })
        .count();

    assert!(
        uses >= 5,
        "only {uses} file(s) under src/sheets call `a1::sheet_ref` — the quoting was \
         removed or routed around, and `a1_ranges_do_not_interpolate_a_raw_sheet_name` \
         would still pass"
    );
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("dir entry").path()).collect();
    paths.sort();
    for path in paths {
        match path.is_dir() {
            true => collect_rs_files(&path, out),
            false => {
                if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }
}

fn relative_path(path: &Path) -> String {
    path.strip_prefix(WORKER_ROOT)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
