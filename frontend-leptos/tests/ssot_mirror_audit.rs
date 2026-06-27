//! Plan 014 Phase 2.3 — Forward-looking mirror-types audit (SSOT discipline).
//!
//! The Phase 2.1/2.2 SSOT migration is still open: `frontend-leptos/src/api/types.rs`
//! deliberately mirrors three domain types (`CheckInStatus`, `DepositMethod`,
//! `QrGenerationStatus`) because the frontend mirror types carry defensive
//! `#[serde(default)]` attributes and UI helpers (`as_str()`, `label()`) that
//! the domain SSOT intentionally lacks. See `frontend-leptos/Cargo.toml` and
//! handover 108 (Plan 014 Phase 2.0) for the full rationale.
//!
//! This file is NOT a remediation tool — the existing mirror types are
//! intentional and deferred to a separate plan. It is a **forward-looking
//! regression guard** that catches the case where a NEW business predicate
//! (`is_*`, `can_*`, `has_*`, `should_*`, `requires_*`, `allows_*`) is added
//! to the frontend mirror file without being documented in the allowlist.
//!
//! ## What this guard catches
//!
//! The realistic regression vector:
//! 1. A developer adds a new business predicate to a domain model, e.g.
//!    `EventConfig::is_early_bird_eligible(now_ms: i64) -> bool`.
//! 2. The frontend needs the same logic, so the developer re-implements it as
//!    a method on the mirror type in `api/types.rs` instead of calling into
//!    the domain SSOT.
//! 3. The two implementations drift over time (one gets a bug fix, the other
//!    doesn't) and the frontend silently makes wrong decisions.
//!
//! This guard forces step 2 to either (a) add the new mirror predicate to the
//! allowlist with a documented reason, or (b) question whether the predicate
//! should be mirrored at all. The discipline is the value — the allowlist is
//! the artifact.
//!
//! ## What this guard deliberately does NOT catch
//!
//! - **Inline re-implementations.** A line like
//!   `let is_checked_in = attendee.checked_in_at.is_some();` in
//!   `frontend-leptos/src/pages/admin.rs` re-implements
//!   `Attendee::is_checked_in()` inline, not as a named method. Detecting this
//!   requires semantic analysis of boolean expressions, not text scanning.
//!   Documented as a known gap in `.plans/014_ssot_audit.md`.
//! - **Mirror types outside `api/types.rs`.** The audit found that
//!   `api/types.rs` is the only file with the `/// Mirrors domain::...` doc
//!   comment pattern. If a second mirror-types file appears, this guard's
//!   scope constant (`MIRROR_FILES`) must be updated.
//! - **UI helper methods** (`as_str()`, `label()`, `css_class()`). These are
//!   explicitly part of the mirror types' value-add and are NOT business
//!   predicates. The guard only looks at `is_*`/`can_*`/`has_*`/etc.
//!
//! ## Audit baseline (2026-06-27)
//!
//! The audit-first pass found:
//!
//! - Domain exports 18 business predicates across 5 model types
//!   (`Attendee`, `EventConfig`, `EventFormat`, `EscrowStatus`, `DepositStatus`,
//!   plus `AppError` and `ColumnMapping` utility predicates).
//! - Frontend mirror file `api/types.rs` re-implements exactly ONE business
//!   predicate: `CheckInStatus::is_approved()` mirroring
//!   `domain::models::attendee::CheckInStatus` (delegated via
//!   `Attendee::is_approved()`).
//! - The other 17 domain predicates are not mirrored in the frontend.
//!
//! ## Run
//!
//! ```sh
//! cargo test --test ssot_mirror_audit
//! ```

use std::fs;
use std::path::{Path, PathBuf};

/// Root of the `frontend-leptos` crate. `CARGO_MANIFEST_DIR` points here
/// when this integration test runs.
const FRONTEND_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"));

/// Root of the workspace, resolved as the parent of `frontend-leptos/`.
/// Used to reach `domain/src/` for the SSOT baseline scan.
fn workspace_root() -> PathBuf {
    Path::new(FRONTEND_ROOT)
        .parent()
        .expect("frontend-leptos should have a parent directory (the workspace root)")
        .to_path_buf()
}

/// Frontend source files that contain mirror-type definitions. Each file in
/// this list is scanned for business-predicate methods (`is_*`, `can_*`,
/// `has_*`, etc.) that mirror domain logic.
///
/// **Adding a file here is a conscious decision.** If a new mirror-types file
/// appears in the frontend, it must be added to this list or the guard's
/// coverage silently drops.
const MIRROR_FILES: &[&str] = &["src/api/types.rs"];

/// Business-predicate naming prefixes. A method whose name starts with one of
/// these prefixes is considered a business predicate (a function that encodes
/// a domain rule, not a UI helper).
///
/// This mirrors the naming convention used across `domain::models`. Methods
/// like `as_str()`, `label()`, `css_class()` are UI helpers and are excluded
/// by construction (they don't match any prefix here).
const PREDICATE_PREFIXES: &[&str] = &["is_", "can_", "has_", "should_", "requires_", "allows_"];

/// Manifest of business predicates that are INTENTIONALLY mirrored in the
/// frontend. Each entry records:
///
/// - `method_name`: the predicate name as it appears in the frontend mirror
///   file (e.g. `"is_approved"`).
/// - `domain_source`: where the canonical implementation lives in `domain`
///   (e.g. `"domain::models::attendee::Attendee::is_approved"`). Informational;
///   used in the failure message so a future reader can find the SSOT.
/// - `reason`: why the mirror exists instead of delegating to domain. Must be
///   non-empty — `"unspecified"` is rejected by the manifest self-check.
///
/// ## When to add an entry
///
/// Only when a NEW predicate is added to a mirror file AND the decision is to
/// keep the mirror rather than delegate to `domain`. If you find yourself
/// adding an entry, first ask: could the frontend call the domain method
/// directly instead? If yes, prefer delegation — that's the SSOT discipline.
struct AllowedMirrorPredicate {
    method_name: &'static str,
    domain_source: &'static str,
    reason: &'static str,
}

/// The current allowlist. Sourced from the 2026-06-27 audit baseline.
///
/// To add an entry: append a struct literal, fill in all three fields with a
/// non-empty string, and re-run the test. The manifest self-check will verify
/// your entry is well-formed.
const ALLOWED_MIRROR_PREDICATES: &[AllowedMirrorPredicate] = &[AllowedMirrorPredicate {
    method_name: "is_approved",
    domain_source: "domain::models::attendee::Attendee::is_approved",
    reason: "frontend CheckInStatus is a mirror type with #[serde(default)] \
                 for safe partial-JSON deserialization. is_approved gates UI \
                 state (scanner tone, ticket hero variant) and must match \
                 domain's Approved|CheckedIn membership. Delegation deferred \
                 until Phase 2.1 SSOT migration merges the two types.",
}];

// ---------------------------------------------------------------------------
// Layer 1 — Manifest well-formedness
// ---------------------------------------------------------------------------

#[test]
fn manifest_entries_are_well_formed() {
    assert!(
        !ALLOWED_MIRROR_PREDICATES.is_empty(),
        "allowlist is empty — if the audit found zero mirror predicates, \
         document that explicitly in the test doc comment and keep this check"
    );

    let mut seen: Vec<&str> = Vec::new();
    for entry in ALLOWED_MIRROR_PREDICATES {
        assert!(
            !entry.method_name.is_empty(),
            "manifest entry has empty method_name"
        );
        assert!(
            !entry.domain_source.is_empty(),
            "manifest entry `{}` has empty domain_source",
            entry.method_name
        );
        assert!(
            !entry.reason.is_empty() && entry.reason != "unspecified",
            "manifest entry `{}` has empty or placeholder reason — every \
             mirror must document why it exists",
            entry.method_name
        );
        assert!(
            is_predicate_name(entry.method_name),
            "manifest entry `{}` does not start with a known predicate prefix \
             ({:?}) — is this actually a business predicate?",
            entry.method_name,
            PREDICATE_PREFIXES
        );
        assert!(
            !seen.contains(&entry.method_name),
            "manifest has duplicate entry for `{}`",
            entry.method_name
        );
        seen.push(entry.method_name);
    }
}

// ---------------------------------------------------------------------------
// Layer 2 — Mirror-file scan against the manifest
// ---------------------------------------------------------------------------

#[test]
fn frontend_mirror_predicates_are_all_in_manifest() {
    let mirror_predicates = collect_mirror_predicates();
    assert!(
        !mirror_predicates.is_empty(),
        "mirror scan returned zero predicates — either the audit baseline \
         changed (zero mirrors now exist) or MIRROR_FILES / the scan logic \
         is stale. Re-audit before adjusting this test."
    );

    let allowed_names: Vec<&str> = ALLOWED_MIRROR_PREDICATES
        .iter()
        .map(|e| e.method_name)
        .collect();

    let undeclared: Vec<String> = mirror_predicates
        .iter()
        .filter(|p| !allowed_names.contains(&p.as_str()))
        .cloned()
        .collect();

    assert!(
        undeclared.is_empty(),
        "frontend mirror file contains a business predicate not in the \
         allowlist.\n\n\
         Phase 2.3 (forward-looking SSOT guard) requires every mirrored \
         business predicate to be documented with a reason. Either:\n  \
         (a) delegate to the domain SSOT method instead of mirroring (preferred), or\n  \
         (b) add the predicate to ALLOWED_MIRROR_PREDICATES in this test with a \
         non-empty reason explaining why the mirror exists.\n\n\
         Undeclared mirror predicates:\n{}",
        undeclared
            .iter()
            .map(|n| format!("  - `{n}`"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---------------------------------------------------------------------------
// Layer 3 — Manifest drift (entries must still exist in the mirror file)
// ---------------------------------------------------------------------------

#[test]
fn manifest_entries_still_exist_in_mirror_file() {
    // Catches the case where a mirror predicate is REMOVED from the frontend
    // but the allowlist entry is left behind. A stale entry is harmless
    // functionally but rots the manifest's value as documentation.
    let mirror_predicates = collect_mirror_predicates();

    let stale: Vec<&str> = ALLOWED_MIRROR_PREDICATES
        .iter()
        .map(|e| e.method_name)
        .filter(|name| !mirror_predicates.iter().any(|p| p.as_str() == *name))
        .collect();

    assert!(
        stale.is_empty(),
        "allowlist contains entries for predicates that no longer exist in \
         the mirror file. The manifest has drifted — remove the stale entries \
         so the allowlist reflects reality.\n\n\
         Stale entries:\n{}",
        stale
            .iter()
            .map(|n| format!("  - `{n}`"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---------------------------------------------------------------------------
// Layer 4 — SSOT baseline (domain predicates are reachable & non-empty)
// ---------------------------------------------------------------------------

#[test]
fn domain_predicate_baseline_is_nonempty() {
    // Catches the case where domain's predicate files move or are reorganised
    // and the SSOT baseline becomes silently empty. If this fails, the audit
    // assumption (domain exports ~18 business predicates) needs revisiting.
    let domain_predicates = collect_domain_predicates();
    assert!(
        !domain_predicates.is_empty(),
        "domain predicate scan returned zero results — either \
         `domain/src/models/` moved or the scan logic is stale. The SSOT \
         baseline must be non-empty for this guard to make sense."
    );

    // Sanity bound: domain had 18 predicates at the 2026-06-27 audit. If it
    // drops below ~10, something structural changed (e.g. predicates moved to
    // a non-scanned module) and the guard needs re-scoping.
    assert!(
        domain_predicates.len() >= 10,
        "domain predicate count ({}) is below the audit baseline (~18, \
         floor 10). If predicates moved to a different module, update \
         DOMAIN_PREDICATE_PATHS.",
        domain_predicates.len()
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Paths under `domain/src/` that hold business predicates. Used to establish
/// the SSOT baseline (Layer 4). If domain reorganises its model modules, this
/// list needs updating.
const DOMAIN_PREDICATE_PATHS: &[&str] = &[
    "domain/src/models/attendee.rs",
    "domain/src/models/event.rs",
    "domain/src/models/deposit.rs",
    "domain/src/models/error.rs",
];

/// True iff `name` starts with one of the [`PREDICATE_PREFIXES`].
fn is_predicate_name(name: &str) -> bool {
    PREDICATE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Extract a method name from a Rust source line like
/// `    pub fn is_approved(&self) -> bool {`. Returns `None` for lines that
/// don't declare a predicate-shaped method.
///
/// Conservative: requires `pub fn ` prefix (after trim), a predicate-shaped
/// name, and `&self` or `&mut self` in the signature (so free functions are
/// not flagged — only methods on types).
fn extract_predicate_method_name(line: &str) -> Option<String> {
    let trimmed = line.trim_start();

    // Must be a `pub fn` declaration (not a call site).
    let rest = trimmed.strip_prefix("pub fn ")?;

    // Extract the identifier up to the first non-identifier byte.
    let name_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    let name = &rest[..name_end];

    if !is_predicate_name(name) {
        return None;
    }

    // Must be a method (takes &self or &mut self), not a free function.
    // This deliberately excludes associated functions like
    // `pub fn is_valid_event_id(id: &str) -> bool` — those are utility
    // functions, not model predicates.
    let signature_end = trimmed.find('{').or_else(|| trimmed.find(';'))?;
    let signature = &trimmed[..signature_end];
    if !signature.contains("&self") && !signature.contains("&mut self") {
        return None;
    }

    Some(name.to_string())
}

/// Recursively collect every business-predicate method declared in the
/// configured mirror files. Returns a sorted, deduplicated list.
fn collect_mirror_predicates() -> Vec<String> {
    let root = Path::new(FRONTEND_ROOT);
    let mut out: Vec<String> = Vec::new();

    for &file_rel in MIRROR_FILES {
        let path = root.join(file_rel);
        if !path.is_file() {
            panic!(
                "configured MIRROR_FILES entry `{}` does not exist at {}. \
                 Update the constant to match the current frontend layout.",
                file_rel,
                path.display()
            );
        }
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        for line in source.lines() {
            if let Some(name) = extract_predicate_method_name(line)
                && !out.contains(&name)
            {
                out.push(name);
            }
        }
    }

    out.sort();
    out
}

/// Collect every business-predicate method declared under the configured
/// `domain/src/` paths. Returns a sorted, deduplicated list. Used only for the
/// SSOT-baseline sanity check (Layer 4) — the actual mirror detection compares
/// against the allowlist, not against this list directly.
fn collect_domain_predicates() -> Vec<String> {
    let root = workspace_root();
    let mut out: Vec<String> = Vec::new();

    for &file_rel in DOMAIN_PREDICATE_PATHS {
        let path = root.join(file_rel);
        if !path.is_file() {
            panic!(
                "configured DOMAIN_PREDICATE_PATHS entry `{}` does not exist at {}. \
                 Update the constant or re-scope the audit.",
                file_rel,
                path.display()
            );
        }
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        for line in source.lines() {
            if let Some(name) = extract_predicate_method_name(line) {
                let full = format!("{file_rel}::{name}");
                if !out.contains(&full) {
                    out.push(full);
                }
            }
        }
    }

    out.sort();
    out
}

// ---------------------------------------------------------------------------
// Self-tests — prove the pattern logic catches real violations.
//
// Lives at the end of the file so clippy's `items_after_test_module` lint is
// satisfied (no non-test items after this module).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod self_tests {
    use super::*;

    #[test]
    fn predicate_prefix_check_accepts_known_predicates() {
        assert!(is_predicate_name("is_approved"));
        assert!(is_predicate_name("can_check_in"));
        assert!(is_predicate_name("has_verified_deposit"));
        assert!(is_predicate_name("should_refund"));
        assert!(is_predicate_name("requires_deposit"));
        assert!(is_predicate_name("allows_refund"));
    }

    #[test]
    fn predicate_prefix_check_rejects_ui_helpers() {
        // UI helpers that mirror types legitimately carry — they must NOT
        // trigger the guard.
        assert!(!is_predicate_name("as_str"));
        assert!(!is_predicate_name("label"));
        assert!(!is_predicate_name("css_class"));
        assert!(!is_predicate_name("display_name"));
    }

    #[test]
    fn predicate_prefix_check_rejects_unrelated_methods() {
        assert!(!is_predicate_name("new"));
        assert!(!is_predicate_name("from_str"));
        assert!(!is_predicate_name("parse"));
        assert!(!is_predicate_name("default"));
    }

    #[test]
    fn simulated_undeclared_mirror_would_fail_the_guard() {
        // The canonical regression scenario: someone adds a new predicate to
        // the mirror file. The scan would find it; if it's not in the
        // allowlist, the guard fires. Verify the set logic.
        let allowed: Vec<&str> = ALLOWED_MIRROR_PREDICATES
            .iter()
            .map(|e| e.method_name)
            .collect();

        // Simulate finding an undeclared predicate.
        let simulated_findings = ["is_approved", "is_early_bird_eligible"];
        let undeclared: Vec<&str> = simulated_findings
            .iter()
            .copied()
            .filter(|p| !allowed.contains(p))
            .collect();

        assert_eq!(
            undeclared,
            ["is_early_bird_eligible"],
            "undeclared predicate must be flagged by the set difference"
        );
    }

    #[test]
    fn allowlist_covers_current_audit_baseline() {
        // The 2026-06-27 audit found exactly one mirror predicate. If that
        // changes (more mirrors added and documented), update this test.
        // If it changes without documentation, the Layer 2 test will catch it.
        let mirror_predicates = collect_mirror_predicates();
        assert_eq!(
            mirror_predicates.len(),
            1,
            "audit baseline expected 1 mirror predicate, found {}. \
             If you added a new mirror predicate, update this baseline test \
             AND add the predicate to ALLOWED_MIRROR_PREDICATES.",
            mirror_predicates.len()
        );
        assert_eq!(
            mirror_predicates[0], "is_approved",
            "audit baseline expected the one mirror predicate to be `is_approved`"
        );
    }
}
