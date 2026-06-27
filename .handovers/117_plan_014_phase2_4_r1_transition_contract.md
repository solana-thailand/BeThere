# Handover 117 — Plan 014 Phase 2.4 R1 Escrow Transition Contract Test

**Branch**: `feature/014_phase2_4_r1_transition_contract` (from `develop` @ `a78f712`)
**Phase**: Plan 014 Phase 2.4 R1 (positive follow-up to the Phase 2.4 type-state audit)
**Scope**: Pin the worker's runtime `EscrowStatus` transition allowlist as a regression guard.
**Risk profile**: Test-only change + one enabling one-word visibility flip (`mod` → `pub mod`) that is behaviorally a no-op for the WASM cdylib target.

---

## 1. What Happened

The Phase 2.4 type-state audit (handover 116) concluded the plan's type-state FSM was structurally wrong for the codebase but identified one **genuine monetary-correctness risk worth addressing**: allowlist drift between the two independent copies of the runtime `EscrowStatus` transition guard in `worker/src/event_store/write.rs`.

This handover implements the audit's **R1 recommendation**: a worker-level contract test that pins the 5 legal × 20 illegal transition matrix and catches drift between the two copies.

### The drift risk, concretely

The runtime transition allowlist lives in **two independent `matches!` blocks** inside `write.rs`:

| Function | Location | Called by (production paths) |
|---|---|---|
| `update_event` (async, DB-backed) | `write.rs:502-506` | `handlers/deposit/escrow/status.rs:251` (confirm_escrow_init_handler), `handlers/events/poster.rs:117,184` (upload/delete poster) |
| `apply_update` (pure, no IO) | `write.rs:768-772` | `handlers/events/update.rs:115` (the main `PUT /events/{id}` handler — primary UI-driven path from `frontend-leptos/src/pages/escrow_init.rs`) |

`update_event` does **not** delegate to `apply_update`. Each function has its own copy of the 5-arm allowlist. If someone edits one copy but not the other, the two production paths enforce different state machines — and there is no compile-time signal.

### The audit-first verification before writing

Before writing the test, the following was confirmed against the actual source:

1. **Both copies are byte-identical** (5 legal arms, same order, same error format).
2. **`EscrowStatus`** is `Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default` but **NOT `Copy`** — this shaped the test's iteration pattern (must use references, not deref-move).
3. **`EventConfig`** does NOT derive `Default`; the minimal JSON proven by `worker/tests/serde_contract.rs:382` was reused.
4. **`event_store`** was declared `mod event_store;` (private) in `worker/src/lib.rs:11` — integration tests could not reach `apply_update`.
5. **The 5 legal transitions** exactly match the doc comment on `EscrowStatus` (`None → Initialized → Deactivated → Closed`, plus `Cancelled → None`).

### The enabling visibility change

The audit's R1 said "Risk: low. Test-only change; no production code touched." That was almost right — it did not account for `event_store` being a private module. To call `apply_update` from `worker/tests/`, the module declaration was changed from `mod` to `pub mod`:

```worker/src/lib.rs#L12-17
// Public so that `worker/tests/escrow_transition_contract.rs` (Plan 014
// Phase 2.4 R1) can call `apply_update` directly. Behaviorally a no-op:
// the worker compiles to a cdylib (WASM) with no downstream Rust consumer
// other than integration tests. The `event_store` module's own items were
// already `pub`; only the module declaration was private.
pub mod event_store;
```

This is the **only** production code change. It is behaviorally a no-op:
- The worker compiles to a `cdylib` (WASM) — no downstream Rust consumer exists.
- The `event_store` module's items (`apply_update`, `update_event`, etc.) were already `pub`; only the module wrapper was private.
- The existing `pub use write::{apply_update, ...}` in `mod.rs` was already in place; the module-visibility flip is what unblocked the path.

The alternative (text-only source scan, no behavioral call) was rejected because it is strictly weaker — it catches text drift but cannot catch a logic bug like an inverted `if !valid` condition. The behavioral call is what makes this a real monetary-correctness guard.

### The two-layer test design

The test file `worker/tests/escrow_transition_contract.rs` has two layers, each derived from a single canonical source of truth:

**Canonical constants** (defined once at the top of the file):
- `LEGAL_TRANSITIONS: &[(EscrowStatus, EscrowStatus)]` — the 5 legal pairs.
- `ALL_STATUSES: &[EscrowStatus]` — the 5 variants in canonical order.

**Layer 1 — Behavioral contract** (calls the real `apply_update`):
- `all_legal_transitions_succeed_and_mutate_config`: drives all 5 legal pairs, asserts `Ok(())` AND `config.escrow_status == target`.
- `all_illegal_transitions_fail_with_exact_error_format`: drives all 20 illegal pairs (5×5 minus 5 legal), asserts `Err("invalid escrow status transition: {source} → {target}")` with the exact U+2192 arrow format.
- `illegal_transition_leaves_config_unchanged`: spot-checks 4 illegal pairs, asserts rejected transitions do not mutate `config.escrow_status` (guards against assignment-before-check bugs).

**Layer 2 — Source-scan drift guard** (reads `write.rs` as raw text):
- `each_canonical_arm_appears_exactly_twice_in_source`: for each of the 5 canonical arm-strings (e.g., `(EscrowStatus::None, EscrowStatus::Initialized)`), asserts it appears **exactly 2×** — once per function copy. If someone edits one copy, count drops to 1 and the guard fires.
- `total_arm_count_is_exactly_ten`: asserts the pattern `(EscrowStatus::` appears exactly 10× (5 arms × 2 copies). If someone adds a 6th transition to either copy, count rises to 11+ and the guard fires.
- `error_format_string_appears_exactly_twice_in_source`: asserts `invalid escrow status transition:` appears exactly 2×. Catches error-message drift between the two copies.

**Self-tests** (6 tests): pin the well-formedness of the canonical constants (5 legal, 5 statuses, 25 total, 20 illegal, no duplicates, all references valid, `Cancelled → None` is the unique exit from `Cancelled`, arm-string format matches real source, simulated arm-removal would trigger the drift guard).

### Live-injection verification (non-vacuity proof)

Both layers were verified to actually FIRE by temporary source mutations, then reverted:

**Injection 1 — remove one arm from `update_event`'s copy** (simulate drift):
- `each_canonical_arm_appears_exactly_twice_in_source` FAILED with: `arm (EscrowStatus::Cancelled, EscrowStatus::None) ... found 1` — **precisely identified the drifted arm**.
- `total_arm_count_is_exactly_ten` FAILED with: `found 9`.
- Layer 1 behavioral tests PASSED (correctly — `apply_update`'s copy was untouched; this is the exact scenario where Layer 1 alone would miss the bug but Layer 2 catches it).

**Injection 2 — add an illegal `None → Closed` transition to `apply_update`'s copy** (simulate a loosened allowlist):
- `all_illegal_transitions_fail_with_exact_error_format` FAILED with: `illegal transition None → Closed must produce exact error ... left: Ok(()) right: Err(...)` — **the exact monetary-correctness bug**.
- `illegal_transition_leaves_config_unchanged` FAILED: the now-allowed `None → Closed` mutated the config.
- `total_arm_count_is_exactly_ten` FAILED: `found 11`.

Both injections confirm the guard is non-vacuous. After revert, `git diff worker/src/event_store/write.rs` was empty (production logic untouched, exactly as the audit required).

---

## 2. Changes (1 new test file, 1 enabling one-word edit)

### New: `worker/tests/escrow_transition_contract.rs` (~470 lines)

12 tests in 3 groups (behavioral, source-scan, self-tests). Two-layer design derived from a single canonical `LEGAL_TRANSITIONS` constant. Full doc-comment header explaining the drift risk, what the guard allows/forbids, the audit baseline, and how to run it.

### Edited: `worker/src/lib.rs` (one word: `mod` → `pub mod`)

`event_store` module made public for test accessibility. 6-line comment explains the rationale. Behaviorally a no-op (cdylib target, items were already `pub`).

### NOT changed (deliberately)

- `worker/src/event_store/write.rs` — **zero diff**. The two allowlist copies were NOT refactored to share a single source. The audit's R1 explicitly scoped this as "test-only change; no production code touched." Eliminating the duplication (making `update_event` delegate to `apply_update` for the escrow-status portion) is a separate refactor that would need its own audit and is out of scope.
- No domain types changed.
- No frontend changes.

---

## 3. Validation Done

### Tests

```
cargo test -p event-checkin-worker --test escrow_transition_contract
→ 12 passed; 0 failed
```

Full worker crate (regression check):

```
cargo test -p event-checkin-worker
→ src/lib.rs unittests:        144 passed
→ deterministic_monetary_code:  23 passed
→ do_claim_lock:                15 passed
→ escrow_transition_contract:   12 passed  (NEW)
→ serde_contract:               33 passed
→ total:                       227 passed; 0 failed
```

Worker crate test count: **215 → 227** (+12).

### Clippy

```
cargo clippy -p event-checkin-worker --tests
→ 0 warnings for escrow_transition_contract.rs
```

(Initial run flagged 2 `needless_range_loop` warnings in `no_duplicate_legal_transitions`; fixed by switching from index-based to `enumerate().skip()` iterator pattern.)

### Live-injection verification

Both layers confirmed non-vacuous (see §1). After revert, `write.rs` diff is empty.

---

## 4. Plan / Code / Test Locations

| Artifact | Path |
|---|---|
| New test file | `worker/tests/escrow_transition_contract.rs` |
| Enabling edit | `worker/src/lib.rs:12-17` (`mod` → `pub mod event_store`) |
| Pinned allowlist copy 1 | `worker/src/event_store/write.rs:502-506` (in `update_event`) |
| Pinned allowlist copy 2 | `worker/src/event_store/write.rs:768-772` (in `apply_update`) |
| Domain enum | `domain/src/models/event.rs:115-129` (`EscrowStatus`) |
| Audit doc (R1 recommendation) | `.plans/014_phase2_4_typestate_audit.md:334-353` |
| Prior handover (audit) | `.handovers/116_plan_014_phase2_4_typestate_audit.md` |

Run command:
```sh
cargo test -p event-checkin-worker --test escrow_transition_contract
```

---

## 5. Reflections

### What went well

- **The two-layer design proved its worth during live injection.** Injection 1 (drift) was caught ONLY by Layer 2; Injection 2 (loosened allowlist) was caught by BOTH layers but Layer 1 gave the precise monetary-correctness signal (`Ok(())` vs expected `Err`). Neither layer alone is sufficient; together they cover both failure modes.
- **Canonical-constant-driven design.** Defining `LEGAL_TRANSITIONS` once and deriving both the behavioral expectations and the source-scan arm-strings from it means there is exactly one place to update when the allowlist legitimately changes.
- **Reusing the proven minimal-JSON pattern** from `serde_contract.rs:382` for `EventConfig` construction avoided guessing about serde defaults.
- **Audit-first discipline caught the visibility issue before runtime.** The compilation step revealed `event_store` was private BEFORE any test ran, allowing a clean decision about the enabling edit rather than a debugging session.

### What was harder than expected

- **`EscrowStatus` not being `Copy`** caused 20+ compilation errors on the first iteration. The const arrays `LEGAL_TRANSITIONS` and `ALL_STATUSES` cannot be moved out of, so all iteration had to be by reference and all helpers (`arm_str`, `is_legal`, `make_config`, `make_request`) had to take `&EscrowStatus`. The first draft used `for &(source, target) in LEGAL_TRANSITIONS` (deref-move) throughout. Fixed by switching everything to reference-based iteration. The final code is arguably cleaner for it (no unnecessary cloning), but it cost an extra compile-fix cycle.
- **The `edit_file` tool leaked the file-path line into the file content** on the first create, causing a parse error. Caught immediately by `cargo test --no-run` and fixed. No data loss.
- **The audit's claim "test-only change; no production code touched" was slightly optimistic.** It did not account for `event_store` being a private module. The enabling `mod` → `pub mod` edit is technically a production code change, even if behaviorally a no-op. Flagged transparently in the lib.rs comment and in this handover rather than hidden.

### Where the result differs from the audit's R1

- **The audit said "attempts the transition via `update_event`".** This test uses `apply_update` (the pure function) instead, because `update_event` is async and requires `kv`/`d1` bindings that are not trivially mockable in a worker integration test. `apply_update` contains an identical copy of the allowlist and is the primary UI-driven production path (called by `handlers/events/update.rs:115` for `PUT /events/{id}`). The audit's own handover 116 §8 acknowledged "`update_event` (or `apply_update`)" — so this is within the audit's accepted scope.
- **The audit did not mention the second copy in `apply_update`.** The audit's R1 said "the runtime transition allowlist" (singular) but the audit body documented two copies. The test's Layer 2 explicitly pins both copies and the drift between them — a strict superset of what R1 literally asked for.

---

## 6. Remaining Work

### Plan 014 status after this handover

| Phase | Status | Notes |
|---|---|---|
| **Phase 1** (wire format) | ✅ CONCLUDED | |
| **Phase 2.1** (SSOT audit) | ✅ CONCLUDED | Handover 113 |
| **Phase 2.2** (move predicates) | 🟡 PARTIAL | R1 done (114), R2 done (115), only R3 open |
| **Phase 2.3** (CI dup-check) | ✅ CONCLUDED | Handover 112; scope gap closed by R1 (114) |
| **Phase 2.4** (type-state escrow) | ✅ CONCLUDED | Audit: handover 116. R1: **this handover (117)** |
| **Phase 2.5** (no standalone WASM) | 🟡 Open | Doc-only; mostly satisfied by neg-result #1 |
| **Phase 3** (policy traits) | ✅ CONCLUDED | |
| **Phase 4.1** (profile) | 🟡 Blocked | Needs 200-attendee staged event |
| **Phase 4.3** (I/O wins) | ✅ CONCLUDED | |
| **Phase 4.4** (no-SIMD doc) | 🟡 Blocked | Depends on 4.1 |
| **Phase 5.3** (deterministic guard) | ✅ CONCLUDED | |
| **Phase 5.4** (zero-alloc audit) | ✅ CONCLUDED | |
| **Phase 5.5** (feature-flag discipline) | 🟡 Open | Mostly satisfied |

### Other open Plan 014 work (priority order)

1. **Phase 2.2 R3** — Substantive mirror-type merge decision for `EventFormat`/`EscrowStatus`/etc. Design decision; needs dedicated session.
2. **Phase 2.5** — Documentation-only. Mostly satisfied; could be marked `[x]` with an inline pointer.
3. **Phase 4.1** — Profile a staged 200-attendee event end-to-end. Blocked on infrastructure.
4. **Phase 4.4** — Document the "no SIMD" decision. Blocked on 4.1.
5. **Phase 5.5** — Feature-flag discipline documentation. Mostly satisfied.

### Future refactor (not Phase 2.4, but related)

The two allowlist copies in `write.rs` (`update_event` L502-506 and `apply_update` L768-772) are genuine duplication. The clean fix is to make `update_event` delegate to `apply_update` for the escrow-status portion (or extract a shared `validate_escrow_transition(from, to) -> Result<()>` helper). This was deliberately NOT done in this PR because:
- The audit's R1 scoped this as "test-only."
- The refactor would touch production monetary-code paths and needs its own review.
- The new Layer 2 drift guard makes the duplication safe — any drift is now caught at CI time.

If the duplication is later eliminated, the Layer 2 source-scan tests (`each_canonical_arm_appears_exactly_twice_in_source`, `total_arm_count_is_exactly_ten`) will need their count expectations adjusted from 2 → 1 and 10 → 5 respectively. The self-test `simulated_arm_removal_would_fail_drift_guard` already documents the count logic.

### Immediate operator actions

- [ ] **Push** `develop` to `origin` — `git push origin develop`. After this handover's branch is merged, `develop` will be **9+ commits ahead** of `origin/develop`.
- [ ] **Deploy** via `develop → main → deploy.sh`. **Requires explicit operator confirmation.** No schema changes; rollback is `wrangler rollback`.

### Housekeeping

- [ ] After merge, delete the feature branch: `git branch -d feature/014_phase2_4_r1_transition_contract`
- [ ] Older feature branches still pending deletion (from handovers 113-116): `feature/014_ssot_audit`, `feature/014_phase2_2_guard_widen`, `feature/014_phase2_2_r2_deposit_from_str`, `feature/014_phase2_4_audit`.

### Explicitly deferred (do NOT do without confirmation)

- ❌ **Phase 2.4 R3** (type-state FSM as written) — explicitly recommended AGAINST by the audit.
- ❌ **Phase 2.4 R4** (typed `DepositLifecycle` enum) — explicitly recommended AGAINST.
- ❌ **Phase 2.2 R3** — design decision, needs dedicated session.
- ❌ **Deploy** — needs explicit operator confirmation.
- ❌ **Phase 4.1 / 4.4** — blocked on infrastructure.
- ❌ **Eliminating the allowlist duplication** — out of scope for R1; needs dedicated refactor PR.

---

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md`
- Phase 2.4 audit: `.plans/014_phase2_4_typestate_audit.md`
- Negative results log: `.plans/014_negative_results.md`
- Prior handovers: 111 (Phase 5.3 deterministic guard), 112 (Phase 2.3 SSOT mirror), 113 (Phase 2.1 SSOT audit), 114 (Phase 2.2 R1 guard widen), 115 (Phase 2.2 R2 DepositMethod FromStr), 116 (Phase 2.4 type-state audit).

---

## 8. How to Dev / Test

### Run the new contract test

```sh
cd /Users/ozone/event-checkin
cargo test -p event-checkin-worker --test escrow_transition_contract
# → 12 passed; 0 failed
```

### Confirm production logic is untouched

```sh
git diff develop -- worker/src/event_store/write.rs
# → empty
```

### Confirm the enabling edit is the only production change

```sh
git diff develop -- worker/src/lib.rs
# → shows `mod event_store;` → `pub mod event_store;` with 6-line comment
```

### Verify Layer 2 catches drift (live injection)

```sh
# Temporarily comment out one arm in update_event's allowlist (~L505):
#   | (EscrowStatus::Cancelled, EscrowStatus::None)
cargo test -p event-checkin-worker --test escrow_transition_contract
# → each_canonical_arm_appears_exactly_twice_in_source FAILS (found 1, expected 2)
# → total_arm_count_is_exactly_ten FAILS (found 9, expected 10)
# → Layer 1 behavioral tests still pass (apply_update copy untouched)

# REVERT the change before committing.
```

### Verify Layer 1 catches a loosened allowlist (live injection)

```sh
# Temporarily add an illegal arm to apply_update's allowlist (~L771):
#   | (EscrowStatus::None, EscrowStatus::Closed)
cargo test -p event-checkin-worker --test escrow_transition_contract
# → all_illegal_transitions_fail_with_exact_error_format FAILS
#     (None → Closed returned Ok(()) instead of Err)
# → illegal_transition_leaves_config_unchanged FAILS
# → total_arm_count_is_exactly_ten FAILS (found 11)

# REVERT the change before committing.
```

### Adding a new legal transition (legitimate change workflow)

If a new `EscrowStatus` variant or transition is legitimately added:

1. Update `domain/src/models/event.rs` (`EscrowStatus` enum).
2. Update **both** allowlist copies in `worker/src/event_store/write.rs` (`update_event` L502-506 AND `apply_update` L768-772).
3. Update `LEGAL_TRANSITIONS` and/or `ALL_STATUSES` in `worker/tests/escrow_transition_contract.rs`.
4. If the arm count changes (e.g., 5 → 6 transitions), update `total_arm_count_is_exactly_ten`'s expected count (10 → 12) and the test name.
5. Run `cargo test -p event-checkin-worker --test escrow_transition_contract` — all 12 tests must pass.

### Relationship to Phase 2.3 SSOT guard (handover 112)

This test follows the same pattern as the frontend SSOT mirror guard (`frontend-leptos/tests/ssot_mirror_audit.rs`) and the Phase 5.3 deterministic monetary code guard (`worker/tests/deterministic_monetary_code.rs`): a canonical allowlist + behavioral/source-scan assertions + self-tests for non-vacuity. The three guards together form the project's "pin the invariants the compiler cannot enforce" discipline.

### Relationship to Phase 2.4 audit (handover 116)

This implements R1 (the only positive recommendation from the audit). R2 (document the three-layer state-machine architecture) was already satisfied by the audit document itself. R3 (do NOT type-state the escrow lifecycle) and R4 (do NOT add `DepositLifecycle`) are negative recommendations — they require no action. Phase 2.4 is now fully concluded.