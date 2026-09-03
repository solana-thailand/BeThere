//! Plan 014 Phase 2.4 R1 — EscrowStatus transition allowlist contract test.
//!
//! katgpt-rs demands that monetary state machines be unambiguous. Our
//! equivalent: **the escrow lifecycle allowlist must be pinned and drift-proof**.
//!
//! The Phase 2.4 type-state audit (handover 116) found that the runtime
//! transition allowlist for `EscrowStatus` lived in TWO independent copies
//! inside `worker/src/event_store/write.rs`:
//!
//!   1. `update_event` (async, DB-backed) — called by escrow-init confirmation,
//!      poster upload/delete handlers.
//!   2. `apply_update` (pure, no IO) — called by the main `PUT /events/{id}`
//!      handler, which is the primary UI-driven path for escrow status changes.
//!
//! The two have since been collapsed: `update_event` loads the config, calls
//! `apply_update`, and persists. That removes the drift risk this file was
//! written for, so Layer 2 now pins the opposite property — the allowlist
//! appears exactly ONCE, and a reintroduced second copy fails the guard.
//!
//! The allowlist enumerates 5 legal transitions:
//!
//!   None → Initialized
//!   Initialized → Deactivated
//!   Deactivated → Closed
//!   Closed → None
//!   Cancelled → None
//!
//! All other 20 pairs (including self-transitions) are rejected with:
//!   `"invalid escrow status transition: {source} → {target}"`
//!
//! This file encodes the discipline as a regression guard with two layers:
//!
//! ## Layer 1 — Behavioral contract (exhaustive 25-case matrix)
//!
//! Drives `apply_update` — the pure, IO-free function — with every
//! (source, target) pair in the 5×5 cartesian product. Asserts the 5 legal
//! transitions succeed (and mutate `config.escrow_status`), and the 20
//! illegal transitions produce the exact error format and leave the config
//! unchanged.
//!
//! ## Layer 2 — Source-scan drift guard
//!
//! Reads `worker/src/event_store/write.rs` as raw text and asserts that:
//!   - Each of the 5 canonical arm-strings appears exactly 1×. If a second
//!     copy of the allowlist is reintroduced, or an arm is edited, the
//!     count drops to 1 and the guard fires.
//!   - The total `(EscrowStatus::` arm-pattern count is exactly 10
//!     (5 arms × 2 functions). If someone adds a 6th transition to either
//!     copy, the count rises to 11+ and the guard fires.
//!   - The error format string appears exactly 2× (once per function).
//!
//! ## What this guard deliberately allows
//!
//! - Adding a new `EscrowStatus` variant — as long as BOTH function copies
//!   and `LEGAL_TRANSITIONS` in this file are updated in the same diff.
//! - Changing the error message wording — as long as both copies are updated
//!   and the error-format count stays at 2.
//!
//! ## What this guard deliberately forbids
//!
//! - Silent drift between the two copies of the allowlist.
//! - Adding a transition to one copy without the other.
//! - Changing the allowlist without updating the canonical list in this file.
//!
//! ## Audit baseline (2026-06-27)
//!
//! The Phase 2.4 audit confirmed: 5 legal transitions, 20 illegal, exact
//! error format with U+2192 arrow, two copies in write.rs. This test pins
//! all of those properties.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test escrow_transition_contract
//! ```
//!
//! ## Note on visibility
//!
//! This test calls `apply_update` via `event_checkin_worker::event_store::apply_update`.
//! If the `event_store` module is not `pub` in `worker/src/lib.rs`, this test
//! will fail to compile. The fix is a one-word change (`mod` → `pub mod`) in
//! `lib.rs` — this exposes an already-internal module to tests without
//! changing any production behavior.

use std::fs;
use std::path::Path;

use event_checkin_domain::models::event::{EscrowStatus, EventConfig, UpdateEventRequest};
use event_checkin_worker::event_store::apply_update;

// ================================================================================================
// Canonical source of truth
// ================================================================================================

/// The 5 legal `EscrowStatus` transitions.
///
/// This is the single source of truth for what the allowlist should contain.
/// Both the behavioral test (Layer 1) and the source-scan drift guard
/// (Layer 2) derive their expectations from this constant. If you add or
/// remove a transition, update this list AND both copies in
/// `worker/src/event_store/write.rs`.
const LEGAL_TRANSITIONS: &[(EscrowStatus, EscrowStatus)] = &[
    (EscrowStatus::None, EscrowStatus::Initialized),
    (EscrowStatus::Initialized, EscrowStatus::Deactivated),
    (EscrowStatus::Deactivated, EscrowStatus::Closed),
    (EscrowStatus::Closed, EscrowStatus::None),
    (EscrowStatus::Cancelled, EscrowStatus::None),
];

/// All 5 `EscrowStatus` variants, in canonical order.
///
/// Used to enumerate the full 5×5 = 25 (source, target) cartesian product.
const ALL_STATUSES: &[EscrowStatus] = &[
    EscrowStatus::None,
    EscrowStatus::Initialized,
    EscrowStatus::Deactivated,
    EscrowStatus::Closed,
    EscrowStatus::Cancelled,
];

// ================================================================================================
// Helpers
// ================================================================================================

/// Render an `EscrowStatus` variant as it appears in the `matches!` arms of
/// `write.rs` — e.g., `EscrowStatus::None`.
///
/// Used to construct arm-strings for the source-scan drift guard. Takes a
/// reference because `EscrowStatus` is `Clone` but not `Copy`; the const
/// arrays in this file cannot be moved out of.
fn arm_str(s: &EscrowStatus) -> &'static str {
    match s {
        EscrowStatus::None => "EscrowStatus::None",
        EscrowStatus::Initialized => "EscrowStatus::Initialized",
        EscrowStatus::Deactivated => "EscrowStatus::Deactivated",
        EscrowStatus::Closed => "EscrowStatus::Closed",
        EscrowStatus::Cancelled => "EscrowStatus::Cancelled",
    }
}

/// Whether a (source, target) pair is in the legal transitions allowlist.
///
/// Takes references because the caller iterates over `&EscrowStatus` from the
/// const arrays (`EscrowStatus` is not `Copy`).
fn is_legal(from: &EscrowStatus, to: &EscrowStatus) -> bool {
    LEGAL_TRANSITIONS.iter().any(|(f, t)| f == from && t == to)
}

/// Construct a minimal `EventConfig` with the given `escrow_status`.
///
/// Uses the proven minimal JSON from `serde_contract.rs` (which tests the
/// same struct's serde defaults), then overrides `escrow_status`. All other
/// fields are either empty strings, zero, or their serde defaults.
///
/// `escrow_address` is empty (default), which causes `apply_update` to skip
/// the SEC-002 escrow-critical field lock — ensuring the test cleanly
/// reaches the `escrow_status` transition check.
fn make_config(status: &EscrowStatus) -> EventConfig {
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":""}"#;
    let mut config: EventConfig =
        serde_json::from_str(json).expect("minimal EventConfig JSON must parse");
    config.escrow_status.clone_from(status);
    config
}

/// Construct an `UpdateEventRequest` that requests a transition to `target`.
///
/// All other fields are `None` (default), ensuring `apply_update` processes
/// ONLY the `escrow_status` field and reaches the transition check without
/// triggering any other validation (SEC-002 lock, SEC-003 cap, etc.).
fn make_request(target: &EscrowStatus) -> UpdateEventRequest {
    UpdateEventRequest {
        escrow_status: Some((*target).clone()),
        ..Default::default()
    }
}

// ================================================================================================
// Layer 1 — Behavioral contract (exhaustive 25-case matrix on apply_update)
// ================================================================================================

#[test]
fn all_legal_transitions_succeed_and_mutate_config() {
    for (source, target) in LEGAL_TRANSITIONS {
        // source, target: &EscrowStatus (EscrowStatus is Clone but not Copy).
        // Helpers take references; assertions compare &EscrowStatus == &EscrowStatus.
        let mut config = make_config(source);
        let req = make_request(target);

        let result = apply_update(&mut config, &req);

        assert!(
            result.is_ok(),
            "legal transition {:?} → {:?} should succeed, got Err({})",
            source,
            target,
            result.unwrap_err()
        );
        assert_eq!(
            &config.escrow_status, target,
            "after legal transition {:?} → {:?}, config.escrow_status must be the target",
            source, target
        );
    }
}

#[test]
fn all_illegal_transitions_fail_with_exact_error_format() {
    let mut illegal_count = 0usize;

    for source in ALL_STATUSES {
        for target in ALL_STATUSES {
            if is_legal(source, target) {
                continue;
            }
            illegal_count += 1;

            let mut config = make_config(source);
            let req = make_request(target);

            let result = apply_update(&mut config, &req);

            let expected_error =
                format!("invalid escrow status transition: {} → {}", source, target);

            assert_eq!(
                result,
                Err(expected_error.clone()),
                "illegal transition {:?} → {:?} must produce exact error {:?}",
                source,
                target,
                expected_error
            );
        }
    }

    // Sanity: 5 variants × 5 variants = 25 total pairs; 5 legal; 20 illegal.
    assert_eq!(
        illegal_count, 20,
        "expected exactly 20 illegal transitions in the 5×5 matrix, found {illegal_count}"
    );
}

#[test]
fn illegal_transition_leaves_config_unchanged() {
    // Spot-check: a rejected transition must not mutate config.escrow_status.
    // This guards against a bug where the assignment happens before the check.
    let illegal_pairs: [(EscrowStatus, EscrowStatus); 4] = [
        (EscrowStatus::None, EscrowStatus::Closed),
        (EscrowStatus::Initialized, EscrowStatus::None),
        (EscrowStatus::Closed, EscrowStatus::Initialized),
        (EscrowStatus::Cancelled, EscrowStatus::Initialized),
    ];

    for (source, target) in &illegal_pairs {
        let mut config = make_config(source);
        let req = make_request(target);

        let _ = apply_update(&mut config, &req);

        assert_eq!(
            &config.escrow_status, source,
            "rejected transition {:?} → {:?} must not mutate config.escrow_status",
            source, target
        );
    }
}

// ================================================================================================
// Layer 2 — Source-scan drift guard
// ================================================================================================

/// Root of the worker crate, resolved from `CARGO_MANIFEST_DIR`.
const WORKER_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"));

/// Relative path to the file containing the allowlist.
const WRITE_RS_REL: &str = "src/event_store/write/update.rs";

/// Read `worker/src/event_store/write.rs` as a string.
fn read_write_rs() -> String {
    let path = Path::new(WORKER_ROOT).join(WRITE_RS_REL);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

#[test]
fn each_canonical_arm_appears_exactly_once_in_source() {
    // The allowlist has one home: `apply_update`. A count of 2 means a second
    // copy was reintroduced (the shape this file was written for); a count of
    // 0 means an arm was edited or removed.
    let source = read_write_rs();

    for (from, to) in LEGAL_TRANSITIONS {
        let arm = format!("({}, {})", arm_str(from), arm_str(to));
        let count = source.matches(&arm).count();
        assert_eq!(
            count, 1,
            "arm `{arm}` must appear exactly 1× in write.rs (in apply_update); \
             found {count}. More than one means the allowlist was duplicated \
             again; none means you added/removed a transition, in which case \
             update LEGAL_TRANSITIONS in this test to match."
        );
    }
}

#[test]
fn total_arm_count_matches_the_allowlist() {
    // One `(EscrowStatus::` occurrence per legal transition. Adding a 6th
    // transition, or duplicating the `matches!`, changes this count.
    //
    // The pattern `(EscrowStatus::` is specific enough that it only matches
    // the `matches!` arm tuples — it does not match the standalone
    // `matches!(s, EscrowStatus::None)` in the is_escrow_reset check (which
    // lacks the leading paren).
    let source = read_write_rs();
    let total = source.matches("(EscrowStatus::").count();

    assert_eq!(
        total,
        LEGAL_TRANSITIONS.len(),
        "expected one `(EscrowStatus::` arm-pattern occurrence per legal \
         transition in write.rs; found {total}. A change here means a \
         transition was added or removed, or the allowlist was duplicated. \
         Update LEGAL_TRANSITIONS in this test to match."
    );
}

#[test]
fn error_format_string_appears_exactly_once_in_source() {
    // One home for the error wording, matching the one home for the allowlist.
    let source = read_write_rs();
    let error_prefix = "invalid escrow status transition:";
    let count = source.matches(error_prefix).count();

    assert_eq!(
        count, 1,
        "error format string `{error_prefix}` must appear exactly 1× in \
         write.rs (in apply_update); found {count}."
    );
}

// ================================================================================================
// Self-tests
// ================================================================================================

mod self_tests {
    use super::*;

    #[test]
    fn canonical_constants_are_well_formed() {
        // LEGAL_TRANSITIONS must have exactly 5 entries.
        assert_eq!(
            LEGAL_TRANSITIONS.len(),
            5,
            "LEGAL_TRANSITIONS must contain exactly 5 transitions"
        );
        // ALL_STATUSES must have exactly 5 entries.
        assert_eq!(
            ALL_STATUSES.len(),
            5,
            "ALL_STATUSES must contain exactly 5 variants"
        );
        // The cartesian product is 5 × 5 = 25.
        let total_pairs = ALL_STATUSES.len() * ALL_STATUSES.len();
        assert_eq!(total_pairs, 25);
        // Legal + illegal = total.
        let illegal = total_pairs - LEGAL_TRANSITIONS.len();
        assert_eq!(illegal, 20);
    }

    #[test]
    fn no_duplicate_legal_transitions() {
        // Each (source, target) pair must be unique — no duplicates.
        // Iterate by reference (EscrowStatus is not Copy). Using
        // enumerate-skip instead of indexing avoids needless_range_loop.
        for (i, a) in LEGAL_TRANSITIONS.iter().enumerate() {
            for (j, b) in LEGAL_TRANSITIONS.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a, b,
                    "duplicate legal transition found at indices {i} and {j}"
                );
            }
        }
    }

    #[test]
    fn legal_transitions_use_only_known_statuses() {
        // Every status referenced in LEGAL_TRANSITIONS must be in ALL_STATUSES.
        for (from, to) in LEGAL_TRANSITIONS {
            assert!(
                ALL_STATUSES.contains(from),
                "LEGAL_TRANSITIONS references {:?} which is not in ALL_STATUSES",
                from
            );
            assert!(
                ALL_STATUSES.contains(to),
                "LEGAL_TRANSITIONS references {:?} which is not in ALL_STATUSES",
                to
            );
        }
    }

    #[test]
    fn cancelled_has_exactly_one_outgoing_transition() {
        // Cancelled → None is the only legal exit from Cancelled.
        // This pins the "cancellation is terminal until reset" invariant.
        // Collect targets as references (EscrowStatus is not Copy — avoid
        // deref-move out of the const slice).
        let cancelled_targets: Vec<&EscrowStatus> = LEGAL_TRANSITIONS
            .iter()
            .filter_map(|(from, to)| (*from == EscrowStatus::Cancelled).then_some(to))
            .collect();
        assert_eq!(
            cancelled_targets,
            vec![&EscrowStatus::None],
            "Cancelled must have exactly one outgoing transition: → None"
        );
    }

    #[test]
    fn arm_str_format_matches_real_source() {
        // Live injection: verify that arm_str produces text that actually
        // appears in write.rs. If the source format changes (e.g., someone
        // aliases the import), this catches it.
        let source = read_write_rs();
        let first = &LEGAL_TRANSITIONS[0];
        let first_arm = format!("({}, {})", arm_str(&first.0), arm_str(&first.1));
        assert!(
            source.contains(&first_arm),
            "arm_str output `{first_arm}` does not appear in write.rs. \
             The arm format may have changed — update arm_str to match."
        );
    }

    #[test]
    fn simulated_arm_removal_would_fail_drift_guard() {
        // Verify the set logic: if the arm is removed from the source, the
        // count drops to 0 and the guard fires.
        let source = read_write_rs();
        let first = &LEGAL_TRANSITIONS[0];
        let arm = format!("({}, {})", arm_str(&first.0), arm_str(&first.1));

        // Simulate removing the occurrence (replace the match with spaces).
        let simulated = source.replacen(&arm, &" ".repeat(arm.len()), 1);
        let count = simulated.matches(&arm).count();
        assert_eq!(
            count, 0,
            "after simulated removal, count must be 0 (would trigger the \
             drift guard)"
        );
    }
}

// ---------------------------------------------------------------------------
// Layer 3 — writer-set guard (plan 022 §7)
// ---------------------------------------------------------------------------

/// The allowlist only protects the transition if `apply_update` is the *only*
/// place that assigns `escrow_status`. Every event write ultimately persists a
/// whole `EventConfig` (`save_event_config` / the D1 `upsert_event`), so a
/// handler that loads a config, mutates the field and saves would bypass the
/// allowlist entirely while compiling and passing every test above — the
/// recurring defect shape plan 022 exists for.
///
/// Today the only assignment is in `apply_update`; the creation paths
/// (`write/create.rs`, `write/seed.rs`, `duplicate.rs`) hard-code
/// `EscrowStatus::None` in a struct literal, which cannot be a transition.
#[test]
fn apply_update_is_the_only_writer_of_escrow_status() {
    const LICENSED: &str = "src/event_store/write/update.rs";

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();
    walk(&root.join("src"), &mut |path, contents| {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == LICENSED {
            return;
        }
        for line in contents.lines() {
            let l = line.trim_start();
            // `==` is a comparison, not a write; the SQL `escrow_status =
            // excluded.escrow_status` has no leading dot on the left-hand side.
            if l.starts_with("//") {
                continue;
            }
            if let Some(rest) = l.split(".escrow_status =").nth(1)
                && !rest.starts_with('=')
            {
                offenders.push(format!("{rel}: {}", l.trim()));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "these sites assign `escrow_status` outside `apply_update`, so they \
         bypass the 5-transition allowlist while still persisting through \
         `save_event_config` / `upsert_event`: {offenders:?}"
    );
}

fn walk(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    for entry in std::fs::read_dir(dir)
        .expect("worker/src must be readable")
        .flatten()
    {
        let path = entry.path();
        match path.is_dir() {
            true => walk(&path, f),
            false if path.extension().is_some_and(|e| e == "rs") => {
                let contents =
                    std::fs::read_to_string(&path).expect("source file must be readable");
                f(&path, &contents);
            }
            false => {}
        }
    }
}
