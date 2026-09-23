//! The asset-first responses carry the same security headers as the Worker.
//!
//! Once `run_worker_first` became an array (`7ed1c07`), every HTML navigation
//! is served from `[assets]` and never passes through `security_headers_layer`.
//! Staging served `/` and `/ticket/*` with no CSP, HSTS or X-Frame-Options.
//! `frontend-leptos/_headers` now repeats `SECURITY_HEADERS` under `/*`; this
//! pins the two together so a CSP edit on one side cannot silently skip the
//! other.

use event_checkin_worker::SECURITY_HEADERS;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The `(lowercase name, value)` pairs of one `_headers` rule block.
fn headers_block(file: &str, rule: &str) -> BTreeMap<String, String> {
    let mut in_block = false;
    let mut out = BTreeMap::new();
    for line in file.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') && !line.is_empty() {
            in_block = line.trim() == rule;
            continue;
        }
        if in_block && let Some((name, value)) = line.trim().split_once(':') {
            out.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    out
}

fn read_headers_file() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend-leptos/_headers");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn assets_catch_all_mirrors_worker_security_headers() {
    let block = headers_block(&read_headers_file(), "/*");
    let expected: BTreeMap<String, String> = SECURITY_HEADERS
        .iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect();
    assert_eq!(
        block, expected,
        "frontend-leptos/_headers `/*` must list exactly SECURITY_HEADERS \
         (worker/src/middleware/headers.rs)"
    );
}

#[test]
fn every_security_header_value_fits_on_one_headers_line() {
    for (name, value) in SECURITY_HEADERS {
        assert!(!value.contains('\n'), "{name} spans lines");
        assert!(!value.contains("  "), "{name} has a doubled space");
    }
}
