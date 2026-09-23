//! A cached slug is a locator, never an answer.
//!
//! Plan 028 W11. `resolve_event_by_slug` used to read and parse the whole KV
//! `events` index on every public event request. Each isolate now remembers
//! slug → event id. The cache has no TTL and no invalidation, so a hit is safe
//! only because it is checked against the config it points at: a renamed,
//! reassigned or deleted slug must fall back to the index scan instead of
//! serving the old event. This guard keeps that check in place.

use std::{fs, path::Path};

const READER: &str = "event_store/read.rs";

fn source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    let body = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} is unreadable: {e} — was the module moved?"));
    // Drop `//` comments so prose naming an old call is not read as a call.
    body.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn function_body<'a>(code: &'a str, signature: &str) -> &'a str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("{READER}: `{signature}` is gone — was it renamed?"));
    let rest = &code[start..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn a_cached_slug_hit_is_checked_against_the_config() {
    let code = source(READER);
    let body = function_body(&code, "async fn config_for_cached_slug(");
    assert!(
        body.contains("config.slug == slug"),
        "{READER}: config_for_cached_slug must serve a cached id only when the \
         config it loads still carries the slug (plan 028 W11)"
    );
}

#[test]
fn slug_resolution_tries_the_cache_before_the_index() {
    let code = source(READER);
    let body = function_body(&code, "pub async fn resolve_event_by_slug(");
    let cached = body
        .find("config_for_cached_slug(")
        .unwrap_or_else(|| panic!("{READER}: resolve_event_by_slug no longer uses the slug cache"));
    let index = body
        .find("get_event_index(")
        .unwrap_or_else(|| panic!("{READER}: resolve_event_by_slug no longer scans the index"));
    assert!(
        cached < index,
        "{READER}: the slug cache must be consulted before the index scan"
    );
}
