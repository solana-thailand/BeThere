//! CSS class-name audit — guards the `class="..."` ↔ `style.css` contract.
//!
//! The frontend styles elements with bare string literals (`class="pe-card"`),
//! which no compiler checks. Two failure modes follow from that, and this file
//! guards both directions:
//!
//! 1. **Dead CSS.** A rule survives in the stylesheets after the markup that used
//!    it is deleted. This is not hypothetical: a sweep in 2026-09 removed 260
//!    such classes (316 rule blocks, 12.6% of the file). 104 of them were
//!    `landing-*` rules orphaned by a landing-page redesign whose Rust
//!    components were deleted while their CSS was not.
//! 2. **Unstyled markup.** A class is applied in Rust that no rule defines, so
//!    the element silently renders unstyled. The same sweep found 108 of these;
//!    30 were then fixed in `campaigns_page.rs` and the files sharing its form
//!    and table vocabulary, and the remaining 78 are baselined in
//!    `KNOWN_UNSTYLED` below.
//!
//! ## Why two different "used" sets
//!
//! The two directions have opposite failure costs, so they use deliberately
//! different extraction strategies:
//!
//! - **Dead-CSS direction** uses `used_broad()`: every whitespace-separated
//!   token of every string literal in `src/`. This *over*-counts (it sweeps up
//!   plenty of non-class strings), which is exactly right here — over-counting
//!   usage can only cause a dead class to be missed, never cause a live class
//!   to be falsely reported dead. Deleting a live rule is the expensive
//!   mistake; a missed dead rule costs nothing.
//! - **Unstyled direction** uses `used_in_class_position()`: only literals that
//!   sit syntactically in a `class` position. This *under*-counts, which is
//!   again the safe side — an unresolvable dynamic class is simply not checked,
//!   rather than falsely reported as unstyled.
//!
//! ## Extraction rules learned from real false positives
//!
//! - **Glued interpolation.** `format!("attendee-checkbox{}", ...)` yields the
//!   literal token `attendee-checkbox{}`. Splitting on whitespace alone drops
//!   the real class name and reports it dead. Both extractors therefore also
//!   emit the prefix before `{`. This bug reached a generated delete-list
//!   before it was caught; `attendee-checkbox` and `quiz-option-radio` were
//!   both nearly deleted while live.
//! - **Whole-class interpolation.** Every `format!` in class position in this
//!   crate either separates its interpolation with a space (so the interpolated
//!   value is a complete class name, reachable as a literal elsewhere — e.g.
//!   `BadgeInfo::css_class` is `&'static str`) or glues it to literal-only
//!   arguments. No class name is assembled from a non-literal fragment. If that
//!   ever changes — say `format!("badge-{kind}")` — the dead-CSS direction will
//!   start producing false positives and this comment is the place to start.
//!
//! ## What this guard deliberately does NOT catch
//!
//! - **Element/id/attribute selectors.** Only `.class` selectors are audited.
//! - **Whether a rule has any visual effect.** A class defined as an empty
//!   block, or fully overridden by a later rule, counts as "defined".
//! - **Classes injected via `inner_html`.** The SVG blobs passed to
//!   `inner_html` in this crate carry no classes today; if that changes, the
//!   raw-string scan in `used_broad()` covers the dead direction, but the
//!   unstyled direction will not see them.
//! - **CSS-only classes referenced solely by other CSS** (e.g. a `.a .b` where
//!   `.b` is never emitted by Rust). Such a selector can never match, and the
//!   dead direction correctly reports `.b` — that is intended, not a gap.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Classes applied in Rust that no CSS rule defines, as of the 2026-09 audit.
///
/// These predate the guard and are baselined so it can gate *new* regressions.
/// They are not all benign — `btn-ghost` and the `stat-tile-*` family read like
/// markup expecting styling that simply is not there, exactly as the 30 classes
/// already burned down from this list did (`table` rendered at browser default
/// inside a dark theme; `btn-secondary` was a stale name for the existing
/// `.btn-outline`). The discipline of not growing this list is what the guard
/// enforces; `known_unstyled_baseline_has_no_stale_entries` makes sure it can
/// only shrink.
const KNOWN_UNSTYLED: &[&str] = &[
    "access-logistics-card",
    "admin-actions-divider",
    "admin-content-inner",
    "admin-dep-credit-used",
    "admin-escrow",
    "admin-event-select",
    "admin-events-page",
    "admin-info-card",
    "admin-info-card-body",
    "admin-info-card-header",
    "admin-modal-backdrop",
    "admin-modal-body",
    "admin-modal-card",
    "admin-modal-checkbox-row",
    "admin-modal-close",
    "admin-modal-footer",
    "admin-modal-header",
    "admin-section",
    "admin-section-actions",
    "admin-section-subtitle",
    "admin-section-title",
    "admin-sync-errors",
    "badge-error",
    "badge-info-xs",
    "badge-muted",
    "btn-ghost",
    "dashboard-btn-toggle",
    "dashboard-state-loading",
    "dep2-info-note",
    "dep2-qr-polling",
    "dev-campaign-claim-btn",
    "dev-profile-back-btn",
    "dpad-down",
    "dpad-left",
    "dpad-right",
    "dpad-up",
    "event-card-body",
    "event-card-cta",
    "event-card-date",
    "event-card-image",
    "event-card-image-placeholder",
    "event-card-img",
    "event-card-location",
    "event-card-tagline",
    "event-card-title",
    "events-grid",
    "events-grid-3",
    "form-hint",
    "form-section-icon-community",
    "form-select-sm",
    "hide-mobile",
    "hint-text",
    "landing-nav-right",
    "landing-user-avatar",
    "landing-user-badge",
    "loading-spinner",
    "manual-input",
    "match-row-left",
    "match-row-right",
    "nfc-card",
    "nfc-page-container",
    "nfc-pulse-ring",
    "panel-title",
    "pe-field-hint",
    "powered-badge",
    "radio-group",
    "radio-label",
    "recap-body",
    "scanner-hint",
    "scanner-settings-btn-active",
    "sol-dot",
    "spinner-md",
    "stat-tile",
    "stat-tile-label",
    "stat-tile-value",
    "ticket-action-card--rollover",
    "u-mb-xs",
    "u-mt-sm",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// All stylesheets, concatenated in the order `index.html` links them.
///
/// `style.css` was split into `styles/style-NN-*.css` in 2026-09; the parts
/// concatenate byte-for-byte to the original, and the numeric prefix is the
/// load order. Reading them in sorted order therefore reproduces the exact
/// cascade the browser sees.
fn css_sources() -> String {
    let dir = crate_root().join("styles");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "css"))
        .collect();
    assert!(
        !files.is_empty(),
        "no stylesheets found in {}",
        dir.display()
    );
    files.sort();
    files
        .iter()
        .map(|p| fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())))
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_sources() -> String {
    fn walk(dir: &Path, out: &mut String) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for path in entries {
            match path.is_dir() {
                true => walk(&path, out),
                false => {
                    if path.extension().is_some_and(|e| e == "rs") {
                        out.push_str(
                            &fs::read_to_string(&path)
                                .unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
                        );
                        out.push('\n');
                    }
                }
            }
        }
    }
    let mut out = String::new();
    walk(&crate_root().join("src"), &mut out);
    out
}

fn is_class_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '-'
}

fn is_class_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Push every class-shaped token in `lit`, splitting on whitespace and
/// truncating each token at `{` so `format!` prefixes survive (see module docs).
fn push_tokens(lit: &str, out: &mut BTreeSet<String>) {
    for raw in lit.split_whitespace() {
        let tok = raw.split('{').next().unwrap_or("");
        let ok =
            !tok.is_empty() && tok.starts_with(is_class_start) && tok.chars().all(is_class_char);
        if ok {
            out.insert(tok.to_string());
        }
    }
}

/// Every `.class` name appearing in any selector prelude of `css`.
fn css_classes(css: &str) -> BTreeSet<String> {
    // strip /* ... */ comments
    let mut clean = String::with_capacity(css.len());
    let bytes: Vec<char> = css.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] == '/' && bytes.get(i + 1) == Some(&'*') {
            true => {
                i += 2;
                while i < bytes.len() && !(bytes[i] == '*' && bytes.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i += 2;
            }
            false => {
                clean.push(bytes[i]);
                i += 1;
            }
        }
    }

    let mut out = BTreeSet::new();
    let mut prelude = String::new();
    for c in clean.chars() {
        match c {
            '{' | '}' => {
                // `@media`/`@supports` preludes carry no class selectors
                if !prelude.trim_start().starts_with('@') {
                    extract_selector_classes(&prelude, &mut out);
                }
                prelude.clear();
            }
            _ => prelude.push(c),
        }
    }
    out
}

fn extract_selector_classes(sel: &str, out: &mut BTreeSet<String>) {
    let chars: Vec<char> = sel.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '.' && chars.get(i + 1).is_some_and(|&c| is_class_start(c)) {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && is_class_char(chars[end]) {
                end += 1;
            }
            out.insert(chars[start..end].iter().collect());
            i = end;
            continue;
        }
        i += 1;
    }
}

/// Over-counting used-set: tokens from every string literal (normal and raw)
/// plus every `class:name` directive. Drives the dead-CSS direction.
fn used_broad(rs: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let chars: Vec<char> = rs.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // raw string: r#"..."#
        if chars[i] == 'r' && chars.get(i + 1) == Some(&'#') && chars.get(i + 2) == Some(&'"') {
            let start = i + 3;
            let mut end = start;
            while end < chars.len() && !(chars[end] == '"' && chars.get(end + 1) == Some(&'#')) {
                end += 1;
            }
            push_tokens(
                &chars[start..end.min(chars.len())]
                    .iter()
                    .collect::<String>(),
                &mut out,
            );
            i = end + 2;
            continue;
        }
        if chars[i] == '"' {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '"' {
                // skip escapes
                end += if chars[end] == '\\' { 2 } else { 1 };
            }
            push_tokens(
                &chars[start..end.min(chars.len())]
                    .iter()
                    .collect::<String>(),
                &mut out,
            );
            i = end + 1;
            continue;
        }
        i += 1;
    }
    for name in directive_classes(rs) {
        out.insert(name);
    }
    out
}

/// `class:some-name=...` Leptos directives.
fn directive_classes(rs: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (idx, _) in rs.match_indices("class:") {
        let rest = &rs[idx + "class:".len()..];
        let name: String = rest.chars().take_while(|&c| is_class_char(c)).collect();
        if !name.is_empty() && name.starts_with(is_class_start) {
            out.insert(name);
        }
    }
    out
}

/// Under-counting used-set: only literals syntactically in a `class` position.
/// Drives the unstyled-markup direction.
fn used_in_class_position(rs: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for pat in ["class=\"", "class=format!("] {
        for (idx, _) in rs.match_indices(pat) {
            let rest = &rs[idx + pat.len()..];
            // for format!, advance to the template literal's opening quote
            let body = match pat.ends_with('(') {
                true => match rest.find('"') {
                    Some(q) => &rest[q + 1..],
                    None => continue,
                },
                false => rest,
            };
            match body.find('"') {
                Some(close) => push_tokens(&body[..close], &mut out),
                None => continue,
            }
        }
    }
    for name in directive_classes(rs) {
        out.insert(name);
    }
    out
}

#[test]
fn no_dead_css_classes() {
    let css = css_sources();
    let rs = rust_sources();
    let defined = css_classes(&css);
    let used = used_broad(&rs);

    let dead: Vec<&String> = defined.difference(&used).collect();
    assert!(
        dead.is_empty(),
        "{} CSS class(es) defined in style.css are never referenced by any string \
         literal in src/. Delete the rules, or if a class is applied in a way this \
         scan cannot see, document it in the module comment of tests/css_class_audit.rs.\n\
         Dead classes: {dead:#?}",
        dead.len(),
    );
}

#[test]
fn no_unstyled_class_names() {
    let css = css_sources();
    let rs = rust_sources();
    let defined = css_classes(&css);
    let used = used_in_class_position(&rs);
    let baseline: BTreeSet<String> = KNOWN_UNSTYLED.iter().map(|s| s.to_string()).collect();

    let unstyled: Vec<&String> = used
        .difference(&defined)
        .filter(|c| !baseline.contains(*c))
        .collect();
    assert!(
        unstyled.is_empty(),
        "{} class name(s) are applied in src/ but have no rule in style.css, so the \
         element renders unstyled. Add the rule, fix the typo, or — only if the class \
         is a deliberate non-visual hook — add it to KNOWN_UNSTYLED.\n\
         Unstyled: {unstyled:#?}",
        unstyled.len(),
    );
}

/// The baseline is debt, not decoration: an entry that no longer appears in the
/// markup, or that has since gained a rule, must be removed so the list shrinks
/// as the debt is paid rather than silently masking future regressions.
#[test]
fn known_unstyled_baseline_has_no_stale_entries() {
    let css = css_sources();
    let rs = rust_sources();
    let defined = css_classes(&css);
    let used = used_in_class_position(&rs);

    let stale: Vec<&&str> = KNOWN_UNSTYLED
        .iter()
        .filter(|c| !used.contains(**c) || defined.contains(**c))
        .collect();
    assert!(
        stale.is_empty(),
        "{} KNOWN_UNSTYLED entr(ies) are stale — the class is either no longer applied \
         in src/ or now has a CSS rule. Remove them from the list.\n\
         Stale: {stale:#?}",
        stale.len(),
    );
}

#[test]
fn extractors_handle_glued_interpolation() {
    // regression: `format!("attendee-checkbox{}", ..)` must yield the bare prefix
    let rs = r#"view! { <div class=format!("attendee-checkbox{}", extra) /> }"#;
    assert!(used_in_class_position(rs).contains("attendee-checkbox"));
    assert!(used_broad(rs).contains("attendee-checkbox"));
}

#[test]
fn extractors_read_selectors_and_directives() {
    let css = "@media (max-width: 480px) { .a, .b .c { color: red } }\n.d:hover::before { top: 0 }";
    let got = css_classes(css);
    for want in ["a", "b", "c", "d"] {
        assert!(got.contains(want), "missing {want} in {got:?}");
    }
    assert!(directive_classes("<div class:is-open=sig />").contains("is-open"));
}
