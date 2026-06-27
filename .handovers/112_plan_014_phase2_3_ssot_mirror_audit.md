# Handover 112 — Plan 014 Phase 2.3 Forward-Looking SSOT Mirror Audit

> Branch: `feature/014_ci_dup_check` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.3

## 1. What Happened

Plan 014 Phase 2.3 — the **forward-looking CI check that flags future
business-predicate duplication** across `worker/` and `frontend-leptos/` — is
complete. The plan asked for a grep-based compile-gate against `domain::policy`.
That module was never created (6th consecutive Plan 014 audit miss), so the
guard shipped against the *actual* SSOT — idiomatic business-predicate methods
on `domain::models::*` types — and is scoped to the realistic regression vector
(frontend mirror types).

The audit-first pass found that the existing duplication is **intentional and
already documented**: `frontend-leptos/src/api/types.rs` deliberately mirrors
three domain types (`CheckInStatus`, `DepositMethod`, `QrGenerationStatus`)
because the frontend mirror types carry defensive `#[serde(default)]` and UI
helpers (`as_str()`, `label()`) that the domain SSOT intentionally lacks. The
only mirrored *business predicate* is `CheckInStatus::is_approved()`. This is
deferred to Phase 2.1/2.2 (SSOT migration), not Phase 2.3's job.

The value of Phase 2.3 is therefore the **regression guard** — preventing the
mirror file from silently growing new business predicates without a conscious
decision to either document them or delegate to the SSOT.

### The 6th consecutive audit miss

The plan's premise was structurally wrong in the same pattern as five previous
Phase 014 audits:

| # | Plan's premise | Reality |
|---|---|---|
| 1 | Phase 1.4 EventMetaWire has fixed shape | Variable strings; Pod is 26% larger than JSON |
| 2 | Phase 1.5 DepositStatusWire is 1-day work | Needs base58 conversion + ID length policy (~2 days) |
| 3 | Phase 4.3.1 event-series endpoint is uncached | Already cached at 120s (Plan 013 shipped it) |
| 4 | Phase 4.3.3 quiz does one PUT per answer | `submit_quiz` grades in-memory, writes once |
| 5 | Phase 4.3.4 blockhash valid ~120s | Confuses ring-buffer sizing with `MAX_PROCESSING_AGE` (~60–90s) |
| **6** | **Phase 2.3 — grep against `domain::policy`** | **`domain::policy` was never created; predicates are methods on `domain::models::*`** |

The discipline that has worked every time is: **audit first, ship what survives
the audit, document the miss in the plan inline**. Phase 2.3 followed that
discipline — the guard shipped against reality, not against the plan's
assumption.

### The actual SSOT architecture

The codebase does NOT use a separate `domain::policy` module. Business
predicates live as idiomatic methods on the domain model types:

```
domain::models::attendee::Attendee
  ├── is_approved(&self) -> bool
  ├── is_checked_in(&self) -> bool
  ├── is_in_person(&self) -> bool
  ├── can_check_in(&self) -> Result<(), CheckInError>
  ├── has_verified_deposit(&self) -> bool
  └── is_refund_eligible(&self) -> bool

domain::models::deposit::DepositStatus
  ├── is_refundable_tier(&self, max_refundable: u32) -> bool
  └── is_past_deadline(&self, event_end_ms, deadline_hours, now_ms) -> bool

domain::models::event::EventConfig
  ├── is_registration_open(&self, now_ms) -> bool
  ├── is_refund_eligible(&self, now_ms) -> bool
  ├── has_in_person_capacity(&self, current_count) -> bool
  └── has_online_capacity(&self, current_count) -> bool

(plus EventFormat, EscrowStatus, AppError, ColumnMapping predicates)
```

**18 business predicates total** across 5+ model types. The frontend mirror
file (`api/types.rs`) re-implements exactly **one** of them (`is_approved`).

### Initial hypothesis (proven partially wrong)

I started expecting to find accidental duplication across worker and frontend —
a re-implemented predicate in a handler, a parallel `can_refund` function
somewhere. The audit found:

- **Frontend**: deliberate mirror types (documented, deferred to Phase 2.1/2.2)
- **Worker**: no mirror types — worker links domain normally and uses the SSOT
  methods directly. The only "near miss" was `cleanup.rs` computing
  `refund_deadline_secs = refund_deadline_hours * 3600` inline, but this is a
  units conversion for a KV cleanup cutoff, not a re-implementation of
  `DepositStatus::is_past_deadline` (which takes ms-based args).
- **Inline re-implementations** like `let is_checked_in =
  attendee.checked_in_at.is_some();` exist in the frontend
  (`pages/admin.rs:1429`) but are not detectable by text scanning — they
  require semantic analysis of boolean expressions. Documented as a known gap.

## 2. Changes (1 new file, 1 doc edit)

### New: `frontend-leptos/tests/ssot_mirror_audit.rs` (~490 lines, 9 tests)

Four layers of defense, plus 5 self-tests proving the pattern logic works:

**Layer 1 — Manifest well-formedness (`manifest_entries_are_well_formed`)**
Asserts every entry in `ALLOWED_MIRROR_PREDICATES` has a non-empty
`method_name`, a non-empty `domain_source`, a non-empty non-placeholder
`reason`, starts with a known predicate prefix, and is not duplicated. This
catches a developer adding a placeholder entry like
`{ method_name: "is_foo", reason: "unspecified" }`.

**Layer 2 — Mirror-file scan against the manifest
(`frontend_mirror_predicates_are_all_in_manifest`)**
Recursively scans every file in `MIRROR_FILES` (currently just
`src/api/types.rs`) for `pub fn is_*/can_*/has_*/should_*/requires_*/allows_*`
method declarations on `&self` or `&mut self`. Every predicate found must
appear in the allowlist. Catches the canonical regression vector: a new
predicate added to a mirror type without documentation.

**Layer 3 — Manifest drift (`manifest_entries_still_exist_in_mirror_file`)**
Catches the reverse case: an allowlist entry for a predicate that no longer
exists in the mirror file. Stale entries rot the manifest's value as
documentation. Forces a cleanup when a mirror predicate is removed.

**Layer 4 — SSOT baseline (`domain_predicate_baseline_is_nonempty`)**
Confirms the scan of `domain::models::*` returns ≥10 predicates (audit
baseline was 18, floor is 10). Catches the case where domain reorganises its
model modules and the baseline scan silently empties.

**Self-tests (5 tests under `self_tests` module, at end of file for
`items_after_test_module` clippy satisfaction)**
Proves the pattern logic catches real violations while rejecting deterministic
lookalikes:
- *Positive*: `is_approved`, `can_check_in`, `has_verified_deposit`,
  `should_refund`, `requires_deposit`, `allows_refund` all classify as
  predicates.
- *Negative*: `as_str`, `label`, `css_class`, `display_name` (UI helpers) and
  `new`, `from_str`, `parse`, `default` (unrelated methods) do NOT classify.
- *Simulation*: a synthetic `is_early_bird_eligible` predicate added to a
  simulated findings list is correctly flagged as undeclared.

### The allowlist (the artifact)

```rust
const ALLOWED_MIRROR_PREDICATES: &[AllowedMirrorPredicate] = &[
    AllowedMirrorPredicate {
        method_name: "is_approved",
        domain_source: "domain::models::attendee::Attendee::is_approved",
        reason: "frontend CheckInStatus is a mirror type with \
                 #[serde(default)] for safe partial-JSON deserialization. \
                 is_approved gates UI state (scanner tone, ticket hero \
                 variant) and must match domain's Approved|CheckedIn \
                 membership. Delegation deferred until Phase 2.1 SSOT \
                 migration merges the two types.",
    },
];
```

The allowlist is intentionally a `const` struct array, not a separate file —
it lives next to the test so any change to it shows up in the same diff as the
test logic. Each entry must document *why* the mirror exists instead of
delegating to the SSOT.

### Live injection test (manual verification, not committed)

Before committing, I verified the guard actually fires on a real violation:

1. Injected `pub fn is_early_bird_eligible(&self) -> bool { true }` into the
   `impl CheckInStatus` block in `frontend-leptos/src/api/types.rs`.
2. Ran the guard. It failed loudly with an actionable message:
   ```
   frontend mirror file contains a business predicate not in the allowlist.
   Phase 2.3 (forward-looking SSOT guard) requires every mirrored business
   predicate to be documented with a reason. Either:
     (a) delegate to the domain SSOT method instead of mirroring (preferred), or
     (b) add the predicate to ALLOWED_MIRROR_PREDICATES in this test with a
     non-empty reason explaining why the mirror exists.
   Undeclared mirror predicates:
     - `is_early_bird_eligible`
   ```
3. Restored the file via backup copy. Confirmed clean via `git diff`.
4. Re-ran the guard. All 9 tests passed.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md`

Phase 2.3 marked `[x]` DONE with the audit outcome (6th consecutive miss) and
the corrected scope recorded inline.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Guard tests pass | `cargo test --test ssot_mirror_audit` in `frontend-leptos/` | ✅ 9/9 (4 layers + 5 self-tests) |
| Frontend tests | `cargo test` in `frontend-leptos/` | ✅ 159 tests pass (92 + 55 + 9 + 3), 0 failed |
| Frontend clippy | `cargo clippy --test ssot_mirror_audit` in `frontend-leptos/` | ✅ Zero warnings in the test file (pre-existing lib warnings unchanged) |
| Frontend native | `cargo check --all-targets` in `frontend-leptos/` | ✅ EXIT 0 |
| Frontend wasm32 | `cargo check --target wasm32-unknown-unknown` in `frontend-leptos/` | ✅ EXIT 0 |
| Live injection | Manual: inject `is_early_bird_eligible`, run, revert | ✅ Guard fires with clear message; restored cleanly |
| Workspace tests | `cargo test --workspace` (does not include frontend-leptos) | ✅ 308 tests pass, 0 failed (unchanged from handover 111) |

### Frontend test count growth

- Before: 150 tests in frontend-leptos (92 + 55 + 3 wire)
- After: **159 tests** (+9 from this work: 4 guard layers + 5 self-tests)

## 4. Plan / Code / Test Locations

- **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.3 (now marked
  DONE with audit outcome and corrected scope recorded inline).
- **Guard test**: `frontend-leptos/tests/ssot_mirror_audit.rs` (~490 lines,
  9 tests across 5 modules: manifest well-formedness, mirror-file scan,
  manifest drift, SSOT baseline, self-tests).
- **Allowlist artifact**: `ALLOWED_MIRROR_PREDICATES` const struct array in
  the test file — 1 entry (`is_approved`).
- **Mirror file under guard**: `frontend-leptos/src/api/types.rs` (configured
  in `MIRROR_FILES`).
- **SSOT baseline paths**: `DOMAIN_PREDICATE_PATHS` const (4 files: attendee,
  event, deposit, error model modules).

## 5. Reflections

### What went well

- **The audit-first discipline caught another structural miss.** The plan
  assumed `domain::policy` exists. It doesn't, and never did. Had I shipped a
  grep against `domain::policy` literally, the test would have scanned an empty
  module and reported zero predicates — a silent no-op passing as work done.
  Instead, the audit mapped the *actual* SSOT (18 predicates across 5+ model
  types) and the guard shipped against reality.
- **The live injection test was the highest-value check.** Same lesson as
  Phase 5.3 — a guard that never fires is untested. The injection of
  `is_early_bird_eligible` proved the source-scan walks the configured mirror
  file and surfaces violations with an actionable message (delegate or
  document).
- **The allowlist-as-artifact approach.** Rather than a binary "pass/fail"
  guard, the manifest is a living document. Every mirror predicate has a
  recorded reason. The manifest drift test (Layer 3) keeps the artifact honest
  — if a predicate is removed, the entry must be cleaned up. This turns the
  guard into documentation that doesn't rot.
- **Clippy caught three real issues.** The first version had a `useless_vec`
  warning (`vec!["a", "b"]` → `["a", "b"]`), a collapsible `if let` +
  `if` pattern, and an `items_after_test_module` ordering issue. All three
  were genuine code-quality concerns, not stylistic noise. The clippy fixes
  made the test cleaner and the module ordering clearer.

### What was harder than expected

- **Designing the scope of "business-predicate duplication".** The codebase
  has three distinct forms of potential duplication:
  1. **Named mirror predicates** on mirror types (catchable — this is what the
     guard targets).
  2. **Inline re-implementations** like `let is_checked_in =
     attendee.checked_in_at.is_some();` (not catchable by text scan — requires
     semantic analysis).
  3. **Mirror types themselves** (the `CheckInStatus` enum in frontend
     duplicates `CheckInStatus` in domain, even without any methods).

  The guard targets only (1) — the detectable, valuable subset. (2) and (3)
  are documented as out-of-scope in the test's doc comment. This honest
  scoping avoids overclaiming what the guard catches.
- **The structural challenge of where the test lives.** `frontend-leptos` is
  excluded from the workspace Cargo.toml (it builds in isolation under
  `trunk`), so a workspace-level test can't reach it. The test lives in
  `frontend-leptos/tests/` and reads domain source via a relative path
  (`workspace_root()` = parent of `frontend-leptos/`). This works but couples
  the test to the directory layout — the `DOMAIN_PREDICATE_PATHS` constant
  must be updated if domain reorganises.

### Where the result differs from the plan

The plan said: "Add a CI check that greps `worker/src/` and
`frontend-leptos/src/` for re-implementations of any function exported from
`domain::policy`." Three corrections:

1. **`domain::policy` doesn't exist.** The guard scans against
   `domain::models::*` predicates instead (the actual SSOT).
2. **Worker is out of scope.** The worker links domain normally and uses SSOT
   methods directly — there are no mirror types to guard. The duplication risk
   in worker is inline logic (like `cleanup.rs`'s deadline computation), which
   text scanning can't catch.
3. **The frontend scope is narrower than "any function".** The guard looks
   only for business-predicate methods (`is_*`/`can_*`/etc.) on mirror types
   in the configured mirror file. UI helpers (`as_str`, `label`) are
   explicitly allowed because they're the mirror types' value-add.

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): 🟡 Open — the cross-crate audit that would
  *eventually* eliminate the mirror types this guard watches over. Until 2.1
  lands, the guard's allowlist is the documentation of intent.
- **Phase 2.3** (forward-looking CI dup-check): ✅ **CONCLUDED (this handover)**
- **Phase 2.4** (type-state escrow lifecycle): 🟡 Open — independent of 2.3.
- **Phase 3** (policy traits): ✅ CONCLUDED (trait demoted, entry #7)
- **Phase 4.1** (profile): 🟡 Blocked on infra
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic-not-stochastic lint): ✅ CONCLUDED
- **Phase 5.4** (zero-alloc audit): ✅ CONCLUDED
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied)

### What's next (priority order)

1. **Deploy** the commits now sitting on `develop` (5 from handover 109 + 3
   from handover 110 + 3 from handover 111 + this handover's commits) via
   `develop → main → deploy.sh`. Operator action. The deploy is now
   **14 commits behind** main — significant enough to warrant priority.
2. **Phase 2.1** — SSOT audit. This is the substantive follow-up to 2.3: the
   audit that would decide whether the mirror types stay, get merged into
   domain, or get replaced by delegation. High audit-miss risk per Plan 014
   track record (6 of 6 audits so far found the plan's premises wrong or
   already-satisfied).
3. **Phase 2.4** — type-state escrow lifecycle. The legitimate sibling of
   katgpt-rs's `ConstraintPruner`. Independent of 2.3.

### Guard re-open preconditions for Phase 2.3

The guard will fire on any of these:

- A developer adds a new `pub fn is_*/can_*/has_*/should_*/requires_*/allows_*`
  method to `frontend-leptos/src/api/types.rs` (or any future file in
  `MIRROR_FILES`) without adding it to `ALLOWED_MIRROR_PREDICATES`.
- A developer removes a predicate from the mirror file but leaves its entry
  in the allowlist (Layer 3 — manifest drift).
- The `domain/src/models/` layout changes such that the baseline scan returns
  fewer than 10 predicates (Layer 4 — catches SSOT reorganisation).
- The mirror file (`api/types.rs`) is moved or renamed without updating
  `MIRROR_FILES` (the scope-sanity panic in `collect_mirror_predicates`).

If a legitimate new mirror predicate is needed, the right response is to add
it to `ALLOWED_MIRROR_PREDICATES` with a non-empty reason explaining why the
mirror exists instead of delegating to the SSOT. If the reason is "we should
actually delegate", then delegate — that's the SSOT discipline.

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md`
- Plan 014 Phase 2.3 inline conclusion: ibid., Phase 2.3 task entry
- Plan 014 negative results log: `.plans/014_negative_results.md` (9 entries;
  Phase 2.3 is a positive result with corrected scope, so no entry added)
- Previous handover (Phase 5.3 deterministic guard): `.handovers/111_plan_014_phase5_3_deterministic_guard.md`
- Phase 5.4 zero-alloc audit handover: `.handovers/110_plan_014_phase5_4_zero_alloc_audit.md`
- Phase 4.3 I/O wins handover: `.handovers/109_plan_014_phase4_3_io_wins.md`
- Phase 1 wire format handover (where mirror types were documented as
  deliberate): `.handovers/108_plan_014_phase1_wire_format.md`

## 8. How to Dev / Test

### Run the guard

```sh
cd frontend-leptos
cargo test --test ssot_mirror_audit
```

No feature flags required. The 9 tests break down as:

- 4 guard tests (manifest well-formedness, mirror-file scan, manifest drift,
  SSOT baseline)
- 5 self-tests under the `self_tests` module (3 prefix-classification positive
  cases, 2 simulation/baseline checks)

### Add a new intentional mirror predicate

1. Add the predicate method to the mirror type in
   `frontend-leptos/src/api/types.rs`.
2. Open `frontend-leptos/tests/ssot_mirror_audit.rs`.
3. Append an entry to `ALLOWED_MIRROR_PREDICATES`:

   ```rust
   AllowedMirrorPredicate {
       method_name: "is_new_predicate",
       domain_source: "domain::models::your_type::YourType::is_new_predicate",
       reason: "why this mirror exists instead of delegating — be specific",
   },
   ```

4. Run the guard. All tests must pass.
5. Update the `allowlist_covers_current_audit_baseline` self-test if the
   baseline count changed (it currently asserts exactly 1 mirror predicate).

### Add a new mirror file to the scan

If a second mirror-types file appears in the frontend:

1. Open `frontend-leptos/tests/ssot_mirror_audit.rs`.
2. Add the file's relative path (from `frontend-leptos/`) to `MIRROR_FILES`.
3. Run the guard. If the new file contains predicates not in the allowlist,
   they will be flagged — add them or delegate.

### Verify the guard fires on a real violation

```sh
cd frontend-leptos

# Inject a new predicate into the mirror file
python3 -c "
content = open('src/api/types.rs').read()
injected = content.replace(
    '    pub fn is_approved(&self) -> bool {',
    '    pub fn is_early_bird_eligible(&self) -> bool { true }\n\n    pub fn is_approved(&self) -> bool {',
    1
)
open('src/api/types.rs', 'w').write(injected)
"

# Run the guard — it must fail
cargo test --test ssot_mirror_audit frontend_mirror_predicates_are_all_in_manifest

# Restore
git checkout src/api/types.rs

# Re-run — all 9 must pass
cargo test --test ssot_mirror_audit
```

### Relationship to Phase 2.1 (SSOT audit)

Phase 2.1 is the substantive follow-up: the cross-crate audit that would
decide whether the mirror types stay, get merged into domain, or get replaced
by delegation. Until 2.1 lands:

- This guard's allowlist is the **documentation of intent** — every mirror
  predicate has a recorded reason.
- When 2.1 lands and a mirror predicate is replaced by delegation, the
  allowlist entry for that predicate must be removed (Layer 3 — manifest
  drift — will catch it if forgotten).
- If 2.1 eliminates the mirror types entirely (full SSOT migration), the
  `MIRROR_FILES` list will become empty and the `frontend_mirror_predicates_are_all_in_manifest`
  test's non-empty assertion will fire — a conscious decision point to either
  remove the guard or re-scope it.