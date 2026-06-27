//! Plan 014 Phase 5.3 — Deterministic-not-stochastic guard for monetary code.
//!
//! katgpt-rs has a hard "sigmoid, never softmax" rule. Our equivalent for
//! monetary code: **deterministic, never stochastic**. No RNG in policy
//! decisions, no probabilistic gates on refunds / claims / deposits.
//!
//! This file encodes the discipline as a regression guard. It is split into
//! two layers:
//!
//! 1. **Dependency guard** — asserts `event-checkin-worker/Cargo.toml` does
//!    not pull in a direct RNG crate (`rand`, `fastrand`, `getrandom`,
//!    `rand_core`, `rand_chacha`). Without the dependency, no monetary module
//!    can call into the RNG ecosystem even if a developer tries. This is the
//!    strongest layer because it blocks the most common regression vector.
//!
//! 2. **Source-scan guard** — recursively scans every `.rs` file under the
//!    monetary module tree for forbidden direct RNG patterns
//!    (`rand::thread_rng`, `OsRng`, `Math::random`, `getRandomValues`,
//!    `gen_range`, etc.). Catches the case where randomness enters via an
//!    indirect path (Web Crypto, JS bridge, a transitive dep exposed by
//!    something else). Belt-and-suspenders for layer 1.
//!
//! ## What this guard deliberately allows
//!
//! - `Uuid::now_v7()` — identifier generation, not a decision. UUID v7 is
//!   timestamp-prefixed and monotonic; its random tail is collision-avoidance,
//!   not a policy input. Used for claim tokens, lock IDs, correlation IDs.
//! - `chrono::Utc::now()` — wall-clock time, not randomness.
//! - Deterministic hashes (FNV-1a, BLAKE3, SHA-256, HMAC-SHA256) — these are
//!   pure functions of their input, not RNG.
//! - Deterministic shuffles (e.g. `frontend-leptos`'s reverse-based puzzle
//!   shuffle) — out of scope for this test (frontend-only), and deterministic
//!   by construction.
//!
//! ## What this guard deliberately forbids
//!
//! Any direct call into an RNG source inside a monetary module. The list of
//! forbidden patterns lives in [`FORBIDDEN_RNG_PATTERNS`] and can only be
//! shortened (never silently extended) by editing this file. If a legitimate
//! need ever arises, the right response is to document the exception in this
//! file's doc comment AND in `.plans/014_negative_results.md`, not to remove
//! the guard.
//!
//! ## Audit baseline (2026-06-27, blake3 1.8.5, uuid 1.x)
//!
//! The audit-first pass confirmed: no `rand` / `fastrand` / `getrandom`
//! dependency anywhere in the workspace; every monetary decision path
//! (refund verify, claim lock acquisition, escrow status, deposit verify)
//! already uses purely deterministic business rules. This test exists to
//! keep it that way.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test deterministic_monetary_code
//! ```

use std::fs;
use std::path::{Path, PathBuf};

/// Root of the worker crate, resolved from `CARGO_MANIFEST_DIR` (which points
/// at `worker/` when this integration test runs).
const WORKER_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"));

/// Directories under `worker/src/` that constitute the monetary decision tree.
///
/// Every file under these trees is scanned. New files dropped into any of
/// these directories are automatically covered — no allowlist edits needed.
const MONETARY_DIRS: &[&str] = &[
    "src/claim",
    "src/solana_escrow",
    "src/escrow_indexer",
    "src/handlers/deposit",
];

/// Individual monetary source files that live outside the directory groups
/// above but still touch money (claim tokens, deposit wallet resolution,
/// escrow index lookups). Kept explicit so the scope is auditable.
const MONETARY_FILES: &[&str] = &[
    "src/handlers/claim.rs",
    "src/handlers/escrow_index.rs",
    "src/handlers/checkin.rs",
    "src/handlers/register.rs",
    "src/handlers/walkin.rs",
    "src/handlers/wallet.rs",
];

/// Direct RNG call patterns that have no business in a monetary decision.
///
/// Each entry is a substring search against the raw source. Patterns are
/// specific enough to avoid catching deterministic lookalikes:
///
/// - `rand::` — the `rand` crate's module prefix. Never appears in
///   deterministic code.
/// - `fastrand::` — the `fastrand` crate's module prefix.
/// - `thread_rng` — `rand::thread_rng()`, the most common RNG entrypoint.
/// - `OsRng` / `StdRng` / `SmallRng` — explicit RNG structs from `rand`.
/// - `ChaCha8Rng` / `ChaCha12Rng` / `ChaCha20Rng` — concrete ChaCha RNGs.
/// - `RngCore` / `CryptoRng` — the `rand_core` traits. Pulling these in means
///   a transitive RNG surface has been added.
/// - `from_entropy` / `seed_from` — RNG seeding entrypoints.
/// - `Math::random` — JS `Math.random()`, the wasm bridge RNG.
/// - `getRandomValues` — Web Crypto `crypto.getRandomValues()` (WebIDL name).
/// - `get_random_values` — the snake_case Rust binding
///   (`web_sys::crypto::get_random_values_with_buffer`). Both forms are
///   forbidden so the guard fires whether the developer copy-pastes JS docs
///   or uses the Rust binding directly.
/// - `gen_range` — `Rng::gen_range`, the most common "pick a number" call.
/// - `fill_bytes` — `RngCore::fill_bytes`, the bulk-randomness call.
///
/// `Uuid::now_v7` is intentionally NOT in this list — it is an identifier
/// generator, not a decision input.
const FORBIDDEN_RNG_PATTERNS: &[&str] = &[
    "rand::",
    "fastrand::",
    "thread_rng",
    "OsRng",
    "StdRng",
    "SmallRng",
    "ChaCha8Rng",
    "ChaCha12Rng",
    "ChaCha20Rng",
    "RngCore",
    "CryptoRng",
    "from_entropy",
    "seed_from",
    "Math::random",
    "getRandomValues",
    "get_random_values",
    "gen_range",
    "fill_bytes",
];

/// Cargo dependencies whose mere presence in `worker/Cargo.toml` is a
/// violation, regardless of how they are used. The dependency itself opens
/// the RNG surface; scanning source for call sites is secondary.
///
/// Matched as `dep_name` appearing in a `[dependencies]`-style line. The
/// check is intentionally conservative — it does not parse TOML fully, but
/// any of these names appearing in a `name = "version"` or
/// `name = { version = ... }` form is a clear violation. False positives are
/// not realistic: these crate names are unique.
const FORBIDDEN_DEPS: &[&str] = &[
    "rand",
    "fastrand",
    "getrandom",
    "rand_core",
    "rand_chacha",
    "rand_pcg",
    "rand_xorshift",
];

// ---------------------------------------------------------------------------
// Layer 1 — Cargo manifest dependency guard
// ---------------------------------------------------------------------------

#[test]
fn worker_cargo_manifest_has_no_direct_rng_dependency() {
    let cargo_path = Path::new(WORKER_ROOT).join("Cargo.toml");
    let manifest = fs::read_to_string(&cargo_path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", cargo_path.display()));

    // Walk the manifest line by line so we can attribute any violation to a
    // specific line number in the failure message.
    let mut violations: Vec<(String, &str)> = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim_start();
        for &dep in FORBIDDEN_DEPS {
            if is_dep_declaration(trimmed, dep) {
                violations.push((line.to_string(), dep));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "worker/Cargo.toml declares a forbidden RNG dependency.\n\
         Phase 5.3 (deterministic-not-stochastic) forbids any direct RNG crate in\n\
         monetary code. If you genuinely need randomness in a non-monetary path,\n\
         document it in .plans/014_negative_results.md and reconsider the guard.\n\
         Violations:\n{}",
        violations
            .iter()
            .map(|(line, dep)| format!("  - `{dep}` declared at: {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Detect whether a manifest line declares a dependency named `dep`.
///
/// The line is trimmed defensively inside the helper, so callers may pass
/// either the raw or the pre-trimmed form. Recognises both declaration
/// shapes:
///   - `rand = "0.8"`
///   - `rand = { version = "0.8", features = ["..."] }`
///
/// Returns false for substrings inside comments, other crate names, or
/// feature flags. The check is conservative on purpose: a positive match
/// always warrants a human review, never a silent pass.
fn is_dep_declaration(line: &str, dep: &str) -> bool {
    // Trim defensively so the helper accepts raw or pre-trimmed input.
    let trimmed_line = line.trim_start();
    // Skip comment lines outright.
    if trimmed_line.starts_with('#') {
        return false;
    }
    // The line must start with the dep name followed by ` = `.
    let prefix = format!("{dep} = ");
    let prefix_braced = format!("{dep} =");
    if !(trimmed_line.starts_with(&prefix) || trimmed_line.starts_with(&prefix_braced)) {
        return false;
    }
    // Reject obvious false positives like `rand_extended = ` by requiring the
    // character right after the dep name to be ` `, `=`, or end-of-string.
    let after = trimmed_line[dep.len()..].trim_start();
    after.starts_with('=') || after.is_empty()
}

// ---------------------------------------------------------------------------
// Layer 2 — Monetary-module source-scan guard
// ---------------------------------------------------------------------------

#[test]
fn monetary_modules_contain_no_direct_rng_calls() {
    let files = collect_monetary_source_files();
    assert!(
        !files.is_empty(),
        "monetary file scan returned zero files — the directory constants in this \
         test are stale and need updating to point at the current monetary module tree"
    );

    let mut violations: Vec<(PathBuf, &str, usize, String)> = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", file.display()));

        for (line_no, line) in source.lines().enumerate() {
            for &pattern in FORBIDDEN_RNG_PATTERNS {
                if line.contains(pattern) {
                    violations.push((file.clone(), pattern, line_no + 1, line.to_string()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "monetary module contains a forbidden RNG call.\n\
         Phase 5.3 (deterministic-not-stochastic) forbids any direct RNG usage in\n\
         policy / refund / claim / deposit / escrow decisions. If you have a\n\
         legitimate identifier-generation use, `Uuid::now_v7()` is allowed — use\n\
         that instead. Otherwise document the exception in\n\
         .plans/014_negative_results.md before adjusting this guard.\n\
         Violations:\n{}",
        violations
            .iter()
            .map(|(path, pattern, line_no, line)| {
                format!(
                    "  - {}:{line_no}: pattern `{pattern}`\n      {line}",
                    path.display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---------------------------------------------------------------------------
// Layer 3 — Scope sanity check
// ---------------------------------------------------------------------------

#[test]
fn monetary_module_scope_is_nonempty_and_existing() {
    // Catches the silent-regression case where a refactor moves monetary code
    // out of the directories listed above and the source-scan guard becomes a
    // no-op. If the directory list ever needs to change, this test forces a
    // conscious edit rather than a silent drop in coverage.
    let files = collect_monetary_source_files();
    assert!(
        files.iter().any(|p| p.to_string_lossy().contains("claim")),
        "scope sanity: no `claim` file found — has the claim module moved?"
    );
    assert!(
        files
            .iter()
            .any(|p| p.to_string_lossy().contains("deposit")),
        "scope sanity: no `deposit` file found — has the deposit module moved?"
    );
    assert!(
        files.iter().any(|p| p.to_string_lossy().contains("escrow")),
        "scope sanity: no `escrow` file found — has the escrow module moved?"
    );
    assert!(
        files.len() >= 10,
        "scope sanity: expected at least ~10 monetary files, found {}. \
         If the module tree was reorganised, update MONETARY_DIRS / MONETARY_FILES.",
        files.len()
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively collect every `.rs` file under the configured monetary
/// directory roots, plus the explicit monetary files. The result is sorted
/// so failure messages are deterministic across runs.
fn collect_monetary_source_files() -> Vec<PathBuf> {
    let root = Path::new(WORKER_ROOT);
    let mut out: Vec<PathBuf> = Vec::new();

    for &dir_rel in MONETARY_DIRS {
        let dir = root.join(dir_rel);
        if dir.is_dir() {
            walk_rs_files(&dir, &mut out);
        } else {
            // A configured directory that does not exist is itself a red flag
            // — the scope constants are stale and coverage has silently
            // dropped. Surface it as a panic during the collection rather
            // than as a silently-empty scan.
            panic!(
                "configured MONETARY_DIRS entry `{}` does not exist at {}. \
                 Update the constant to match the current module tree.",
                dir_rel,
                dir.display()
            );
        }
    }

    for &file_rel in MONETARY_FILES {
        let file = root.join(file_rel);
        if !file.is_file() {
            panic!(
                "configured MONETARY_FILES entry `{}` does not exist at {}. \
                 Update the constant to match the current module tree.",
                file_rel,
                file.display()
            );
        }
        out.push(file);
    }

    out.sort();
    out.dedup();
    out
}

/// Depth-first walk that appends every `.rs` file under `dir` to `out`.
fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            panic!("could not read monetary directory {}: {e}", dir.display())
        }
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Self-tests — verify the guard itself catches synthetic violations.
//
// A regression guard that never fires is worthless. These tests exercise the
// helper logic against synthetic inputs so we have proof the patterns are
// specific enough to catch real RNG introductions while rejecting
// deterministic lookalikes (comments, identifier generators, feature flags).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod self_tests {
    use super::*;

    // --- is_dep_declaration: positive cases (must catch) -------------------

    #[test]
    fn dep_declaration_detects_simple_version_form() {
        assert!(is_dep_declaration("rand = \"0.8\"", "rand"));
    }

    #[test]
    fn dep_declaration_detects_braced_form() {
        assert!(is_dep_declaration(
            "rand = { version = \"0.8\", features = [\"small_rng\"] }",
            "rand"
        ));
    }

    #[test]
    fn dep_declaration_detects_with_indented_whitespace() {
        // Manifest entries under `[dependencies]` are typically not indented,
        // but the helper receives the post-trim line, so test that path.
        assert!(is_dep_declaration("  fastrand = \"2\"", "fastrand"));
    }

    // --- is_dep_declaration: negative cases (must NOT catch) ---------------

    #[test]
    fn dep_declaration_rejects_comment_lines() {
        assert!(!is_dep_declaration("# rand = \"0.8\" (forbidden)", "rand"));
    }

    #[test]
    fn dep_declaration_rejects_substring_match_extended_name() {
        // `rand_extended` must not match the `rand` rule.
        assert!(!is_dep_declaration("rand_extended = \"0.1\"", "rand"));
    }

    #[test]
    fn dep_declaration_rejects_substring_match_prefixed_name() {
        // `my_rand` must not match the `rand` rule.
        assert!(!is_dep_declaration("my_rand = \"0.1\"", "rand"));
    }

    #[test]
    fn dep_declaration_rejects_feature_flag_arrays() {
        // A feature list mentioning "rand" is not a dependency declaration.
        assert!(!is_dep_declaration(
            "features = [\"rand\", \"serde\"]",
            "rand"
        ));
    }

    #[test]
    fn dep_declaration_rejects_unrelated_dep_with_similar_prefix() {
        // `rand_core` is itself forbidden, but `rand_core` should not match
        // the `rand` rule (it must match the `rand_core` rule instead).
        assert!(!is_dep_declaration("rand_core = \"0.6\"", "rand"));
        assert!(is_dep_declaration("rand_core = \"0.6\"", "rand_core"));
    }

    // --- FORBIDDEN_RNG_PATTERNS: positive cases (must catch) ---------------

    #[test]
    fn forbidden_patterns_catch_rand_crate_usage() {
        let line = "    let n: u32 = rand::thread_rng().gen_range(0..100);";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(
            matched.contains(&"rand::"),
            "expected `rand::` to match: {matched:?}"
        );
        assert!(
            matched.contains(&"thread_rng"),
            "expected `thread_rng` to match: {matched:?}"
        );
        assert!(
            matched.contains(&"gen_range"),
            "expected `gen_range` to match: {matched:?}"
        );
    }

    #[test]
    fn forbidden_patterns_catch_os_rng() {
        let line = "    let mut rng = OsRng;";
        assert!(FORBIDDEN_RNG_PATTERNS.iter().any(|p| line.contains(p)));
    }

    #[test]
    fn forbidden_patterns_catch_chacha_rng() {
        let line = "    let rng = ChaCha20Rng::from_entropy();";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(matched.contains(&"ChaCha20Rng"));
        assert!(matched.contains(&"from_entropy"));
    }

    #[test]
    fn forbidden_patterns_catch_js_math_random() {
        let line = "    let r = js_sys::Math::random();";
        assert!(
            FORBIDDEN_RNG_PATTERNS.iter().any(|p| line.contains(p)),
            "expected `Math::random` to match the JS bridge RNG line"
        );
    }

    #[test]
    fn forbidden_patterns_catch_web_crypto_getrandomvalues() {
        // Both forms must be caught: the WebIDL name (camelCase, used in JS
        // docs and copy-pasted code) and the snake_case Rust binding that
        // `web_sys` exposes.
        let rust_binding_line = "    web_sys::crypto()?.get_random_values_with_buffer(&mut buf);";
        let js_doc_line = "    // calls crypto.getRandomValues under the hood";

        let rust_matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| rust_binding_line.contains(p))
            .collect();
        assert!(
            rust_matched.contains(&"get_random_values"),
            "snake_case Rust binding must be caught: {rust_matched:?}"
        );

        let js_matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| js_doc_line.contains(p))
            .collect();
        assert!(
            js_matched.contains(&"getRandomValues"),
            "camelCase WebIDL name must be caught: {js_matched:?}"
        );
    }

    #[test]
    fn forbidden_patterns_catch_rngcore_trait() {
        let line = "    fn fill(&mut self, dest: &mut [u8]) -> Result<(), RngCore::Error>;";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(matched.contains(&"RngCore"));
        assert!(
            !matched.contains(&"fill_bytes"),
            "fill_bytes should not match this line"
        );
    }

    // --- FORBIDDEN_RNG_PATTERNS: negative cases (must NOT catch) -----------

    #[test]
    fn forbidden_patterns_do_not_catch_uuid_now_v7() {
        // The explicitly-allowed identifier generator.
        let line = "    let claim_token = Uuid::now_v7().to_string();";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(
            matched.is_empty(),
            "Uuid::now_v7 must NOT match any forbidden pattern, but matched: {matched:?}"
        );
    }

    #[test]
    fn forbidden_patterns_do_not_catch_chrono_now() {
        let line = "    let now = chrono::Utc::now().to_rfc3339();";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(
            matched.is_empty(),
            "chrono::Utc::now must NOT match any forbidden pattern: {matched:?}"
        );
    }

    #[test]
    fn forbidden_patterns_do_not_catch_blake3_or_sha256() {
        let line = "    let h = blake3::hash(&payload);";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(matched.is_empty(), "blake3 must NOT match: {matched:?}");

        let line = "    let h = sha2::Sha256::digest(&payload);";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(matched.is_empty(), "sha2 must NOT match: {matched:?}");
    }

    #[test]
    fn forbidden_patterns_do_not_catch_deterministic_fnv() {
        // The deterministic FNV-1a hash used for on-chain event IDs.
        let line = "    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(
            matched.is_empty(),
            "FNV offset basis must NOT match: {matched:?}"
        );
    }

    #[test]
    fn forbidden_patterns_do_not_catch_deterministic_shuffle_words() {
        // Comments / function names about deterministic shuffles must not
        // false-positive on the word "shuffle" alone (we deliberately did
        // not add "shuffle" to the forbidden list because deterministic
        // shuffles are allowed).
        let line = "    // Deterministic shuffle: reverse the order";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| line.contains(p))
            .collect();
        assert!(
            matched.is_empty(),
            "deterministic-shuffle comment must NOT match: {matched:?}"
        );
    }

    // --- End-to-end: simulate a violation reaching the scanner -------------

    #[test]
    fn simulated_violation_line_triggers_every_expected_pattern() {
        // This is the canonical regression scenario: someone adds
        // `rand = "0.8"` to Cargo.toml and writes this in a refund handler.
        // The dependency guard catches the manifest line; the source-scan
        // guard catches the call site. Both layers must fire.
        let manifest_line = "rand = \"0.8\"";
        assert!(is_dep_declaration(manifest_line, "rand"));

        let source_line = "    let fee_pct: u32 = rand::thread_rng().gen_range(0..100);";
        let matched: Vec<&str> = FORBIDDEN_RNG_PATTERNS
            .iter()
            .copied()
            .filter(|p| source_line.contains(p))
            .collect();
        assert!(matched.contains(&"rand::"));
        assert!(matched.contains(&"thread_rng"));
        assert!(matched.contains(&"gen_range"));
        assert!(
            matched.len() >= 3,
            "expected at least 3 patterns to match the simulated violation, got {matched:?}"
        );
    }
}
