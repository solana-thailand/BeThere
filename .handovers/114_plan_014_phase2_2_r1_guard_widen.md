# Handover 114 — Plan 014 Phase 2.2 R1 Guard Scope Fix

> Branch: `feature/014_phase2_2_guard_widen` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.2 (R1)
> Audit ref: `.plans/014_ssot_audit.md` §"Recommendations for Phase 2.2" R1

## 1. What Happened

Plan 014 Phase 2.2's **highest-priority recommendation (R1)** — widening the
Phase 2.3 mirror-types guard's `MIRROR_FILES` to cover `api/event.rs` and
`api/admin.rs` and allowlisting the three previously-uncovered mirrored
predicates — is implemented and verified. **R2 and R3 remain open** and were
deliberately not touched.

The handover-113 audit had surfaced a real defect in the shipped Phase 2.3
guard: its `MIRROR_FILES` constant was `["src/api/types.rs"]`, but
`frontend-leptos/src/api/event.rs` carried three load-bearing mirrored
business predicates that the guard silently did not cover
(`EscrowStatus::is_active`, `EventFormat::has_in_person`,
`EventFormat::has_online`). This session closes that gap by widening the
scope and explicitly documenting each previously-silent mirror in the
allowlist, rather than by moving or merging any types.

### The change in one paragraph

`frontend-leptos/tests/ssot_mirror_audit.rs` was edited in five targeted
places: (1) the stale "Mirror types outside `api/types.rs`" doc-comment
block was rewritten to reflect the widened scope and to point readers at the
Phase 2.1 audit for context; (2) the audit-baseline doc-comment block was
updated to record both the 2026-06-27 Phase 2.3 baseline (1 predicate) and
the Phase 2.1 re-audit (4 predicates); (3) `MIRROR_FILES` grew from one
entry to three (`src/api/types.rs`, `src/api/event.rs`, `src/api/admin.rs`);
(4) `ALLOWED_MIRROR_PREDICATES` grew from one entry to four — the existing
`is_approved` entry's stale "Phase 2.1" deferral reference was corrected to
point at Phase 2.2's substantive type-merge decision (R3), and three new
entries were added with documented reasons following the same pattern
(mirror type with `#[serde(default)]`; delegation deferred); (5) the
`allowlist_covers_current_audit_baseline` self-test was updated to assert
`mirror_predicates.len() == 4` and to verify the four known predicates by
name (using `contains` rather than index-based assertions so a failure
points at the missing predicate by name).

No `.rs` files outside the test file were touched. No production code
changed. The change is purely a scope correction + documentation update
inside a regression guard.

### Audit-first verification before any change

Before editing anything, I re-verified the handover-113 audit's claims
against the actual code:

| File | `rg "pub fn (is_|can_|has_|should_|requires_|allows_)"` result | Allowlist? |
|---|---|---|
| `frontend-leptos/src/api/types.rs` | 1 — `CheckInStatus::is_approved` | ✅ (existing) |
| `frontend-leptos/src/api/event.rs` | 3 — `EscrowStatus::is_active`, `EventFormat::has_in_person`, `EventFormat::has_online` | ❌ uncovered (now fixed) |
| `frontend-leptos/src/api/admin.rs` | 0 — `FormFieldConfigAdmin`/`RegistrationFormConfigAdmin` have no business predicates | — (defensive scope) |

The audit's headline finding is exactly correct. Total mirror predicates
after R1: **4** (was 1).

### The three new allowlist entries

Each entry follows the same structural pattern as the existing `is_approved`
entry: a non-empty `reason` documenting that the mirror type carries
`#[serde(default)]` for partial-JSON safety, the predicate's role in the
frontend, and a deferral pointer to Phase 2.2's substantive type-merge
decision (R3 in `.plans/014_ssot_audit.md`).

- `is_active` (EscrowStatus) — 0 direct call sites in the frontend as of
  the Phase 2.1 audit (UI uses string compare at
  `pages/admin_cancel.rs:129`); retained in the mirror for parity so the
  predicate is surfaced if cancel flow later moves to typed comparisons.
- `has_in_person` (EventFormat) — 7 call sites (admin nav/scanner/deposit
  gates); load-bearing. If domain tightens the membership (e.g. adds a 4th
  format variant), the mirror must update in lockstep or the UI silently
  shows wrong nav groups.
- `has_online` (EventFormat) — 1 call site (admin form); gates online
  registration UI.

### Live-injection re-verification (extended per handover 112 §2)

Handover 112's §2 protocol was extended from one file to all three files in
`MIRROR_FILES`. For each file, a synthetic
`pub fn is_early_bird_eligible(&self) -> bool { true }` predicate was
injected into an existing `impl` block, the guard was run (must fail and
name `is_early_bird_eligible`), then the file was restored from a backup
copy and the guard was re-run (must pass).

| File | Injection site | Guard fired? | Restored cleanly? |
|---|---|---|---|
| `src/api/types.rs` | `impl CheckInStatus` | ✅ | ✅ (empty diff vs HEAD) |
| `src/api/event.rs` | `impl EscrowStatus` | ✅ | ✅ (empty diff vs HEAD) |
| `src/api/admin.rs` | `impl EscrowInstruction` | ✅ | ✅ (empty diff vs HEAD) |

After restore, `git status` showed only the intended test file as modified,
and the guard passed 9/9 clean. The widened scope works.

### What R1 deliberately did NOT do

- **No type-merge.** The mirror types in `api/event.rs` (`EventStatus`,
  `EscrowStatus`, `EventFormat`, `EventVisibility`, `OnlineOpenMode`) are
  untouched. The substantive merge-vs-delegate-vs-keep decision is R3 and
  is explicitly deferred.
- **No worker-side changes.** R2 (the two `DepositMethod` serialization
  sites in `worker/src/handlers/attendee.rs:367-372` and
  `worker/src/db/deposit_statuses.rs:352-360`) is not implemented. They
  remain tracked in `.plans/014_ssot_audit.md` as zero-behavior-change
  refactors for a follow-up.
- **No domain-side changes.** No new `is_*`/`has_*` predicate was added to
  domain (R4 explicitly recommends against the
  `DepositMethod::is_usdc()` API-surface expansion).
- **No production code changed.** Only one `.rs` file (the test itself) was
  modified, plus two markdown plan/audit documents.

## 2. Changes (3 files: 1 test, 2 docs)

### Edited: `frontend-leptos/tests/ssot_mirror_audit.rs` (+82/-29)

Five targeted edits, all inside the test file:

1. **Doc-comment "out of scope" block (L37-44 area)** — replaced the stale
   "Mirror types outside `api/types.rs`" claim with a corrected
   "Mirror types outside `MIRROR_FILES`" note that explicitly cites the
   Phase 2.1 audit's discovery as the reason the scope was widened.
2. **Audit-baseline doc-comment block (L46-58 area)** — replaced the
   single dated "exactly ONE" baseline with a two-entry history: the
   2026-06-27 Phase 2.3 baseline (1 predicate) and the Phase 2.1 re-audit
   (4 predicates, surfaced by widening — not by new code).
3. **`MIRROR_FILES` constant (L90)** — widened from
   `&["src/api/types.rs"]` to a three-entry list including
   `src/api/event.rs` and `src/api/admin.rs`.
4. **`ALLOWED_MIRROR_PREDICATES` const (L129-137 area)** — grew from 1 to
   4 entries. The existing `is_approved` entry's deferral pointer was
   corrected from "Phase 2.1 SSOT migration" (Phase 2.1 is done; it did
   not merge the types) to "Phase 2.2 substantive type-merge decision (R3
   in .plans/014_ssot_audit.md)" for consistency with the three new
   entries that cite the same deferral target.
5. **`allowlist_covers_current_audit_baseline` self-test (end of file)** —
   updated from asserting `len() == 1` with an index-based `==`
   comparison to asserting `len() == 4` with a `contains` loop over the
   four expected predicate names. A failure now points at the missing
   predicate by name rather than as an opaque index mismatch.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md` (+18/-2)

Two inline updates to the Phase 2 section:

- The Phase 2.1 audit-summary block ("Recommendations for Phase 2.2") was
  updated from "not implemented" to "R1 IMPLEMENTED (Phase 2.2 — guard
  scope fix). See handover 114. R2 and R3 remain open."
- The Phase 2.2 task entry checkbox was changed from `[ ]` to `[~]`
  (partial), with an inline summary of what R1 did and an explicit note
  that R2/R3 remain open and neither is required to close the guard gap.

### Edited: `.plans/014_ssot_audit.md` (+29/-7)

Three inline updates to keep the audit doc honest as a living artifact:

- Section header "Recommendations for Phase 2.2 (NOT implemented in this
  audit)" → "(R1 implemented; R2/R3 open)".
- R1 subsection marked **✅ IMPLEMENTED** with a status line, the actual
  `MIRROR_FILES` value shipped, and the verification summary
  (live-injection extended across 3 files; frontend tests 159 unchanged;
  workspace tests 308 unchanged; zero new clippy warnings).
- Conclusion updated: "Phase 2.1 is complete" stays, but now records that
  R1 has since been implemented as the Phase 2.2 guard scope fix, with
  R2/R3 explicitly noted as open and not required to close the guard gap.

### NOT changed (deliberately)

- `frontend-leptos/src/api/types.rs`, `frontend-leptos/src/api/event.rs`,
  `frontend-leptos/src/api/admin.rs` — untouched. Confirmed by empty
  `git diff` after the live-injection round.
- `worker/src/handlers/attendee.rs`, `worker/src/db/deposit_statuses.rs`
  — R2 sites, untouched.
- `domain/src/models/event.rs` — SSOT untouched; no new predicates added.
- `.plans/014_negative_results.md` — no new entry. R1 is a positive
  result (a real fix shipped against a real defect), not a negative result.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Guard tests | `cargo test --test ssot_mirror_audit` in `frontend-leptos/` | ✅ 9/9 |
| Frontend tests | `cargo test` in `frontend-leptos/` | ✅ 159 (92 + 55 + 9 + 3), unchanged from handover 113 |
| Workspace tests | `cargo test --workspace --quiet` | ✅ 308+, 0 failed (unchanged) |
| Clippy on test file | `cargo clippy --test ssot_mirror_audit` | ✅ Zero new warnings in `ssot_mirror_audit.rs` (pre-existing 185 lib warnings unchanged) |
| Frontend native check | `cargo check --all-targets` | ✅ EXIT 0 |
| Frontend wasm32 check | `cargo check --target wasm32-unknown-unknown` | ✅ EXIT 0 |
| Live injection — types.rs | Inject `is_early_bird_eligible` into `impl CheckInStatus`, run, restore | ✅ Guard fired and named the predicate |
| Live injection — event.rs | Inject into `impl EscrowStatus`, run, restore | ✅ Guard fired and named the predicate |
| Live injection — admin.rs | Inject into `impl EscrowInstruction`, run, restore | ✅ Guard fired and named the predicate |
| Restore cleanliness | `git diff` on the three source files | ✅ Empty — clean restore |
| Diagnostics on test file | IDE diagnostics on `ssot_mirror_audit.rs` | ✅ Zero errors, zero warnings |

### Test count unchanged

Frontend: **159** (was 159 in handover 113). Workspace: **308+** (was 308
in handover 113). R1 is a documentation and scope change to an existing
test, not a new test, so the count is identical.

## 4. Plan / Code / Test Locations

- **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.2 (now
  marked `[~]` partial — R1 done, R2/R3 open).
- **Audit doc**: `.plans/014_ssot_audit.md` — R1 marked ✅ IMPLEMENTED;
  R2/R3 still describe open work.
- **Guard test**: `frontend-leptos/tests/ssot_mirror_audit.rs` (~520
  lines after edit, +82/-29 from handover 113 baseline). 9 tests,
  unchanged in count; the `allowlist_covers_current_audit_baseline`
  self-test now asserts `len() == 4`.
- **Allowlist artifact**: `ALLOWED_MIRROR_PREDICATES` const struct array —
  4 entries (`is_approved`, `is_active`, `has_in_person`, `has_online`).
- **Mirror files under guard**: `src/api/types.rs`, `src/api/event.rs`,
  `src/api/admin.rs` (configured in `MIRROR_FILES`).
- **SSOT baseline paths**: `DOMAIN_PREDICATE_PATHS` const (4 files:
  attendee, event, deposit, error) — unchanged.

## 5. Reflections

### What went well

- **The audit-first discipline paid off again.** Before any edit, I
  re-ran the same `rg` searches that produced the handover-113 findings
  and confirmed the three predicates still exist exactly where the audit
  said they do. No drift between audit and reality. The fix shipped
  against verified facts, not against a stale claim.
- **The live-injection re-verification was the highest-value check — again.**
  Handover 112 §2 only verified one file. R1 widens scope to three files,
  so I extended the protocol to all three. A scope-widening edit that
  silently misconfigures the new files would pass the static test (the
  allowlist would simply not see the new files) but fail the live
  injection. The injection round confirmed the scan actually walks each
  new file in `MIRROR_FILES`.
- **The `contains`-loop refactor of the baseline self-test** turned out
  better than the original index-based `==` assertion. A future failure
  now points at the missing predicate by name, which is more actionable
  than "expected 4, found 3" with no name. Small win, but worth keeping.
- **The reason-text consistency pass on the existing `is_approved` entry.**
  It had a stale "Phase 2.1 SSOT migration merges the two types" pointer
  — Phase 2.1 is done and explicitly chose not to merge. Updating it to
  point at Phase 2.2 / R3 keeps the manifest internally consistent with
  the three new entries that cite the same deferral target.

### What was harder than expected

- **Reason-text writing.** Each new allowlist entry needs a non-empty,
  non-placeholder `reason` that documents *why the mirror exists instead
  of delegating to the SSOT*. The handover-113 audit gave the structural
  pattern (mirror type with `#[serde(default)]`; delegation deferred to
  the type-merge decision) but I had to write per-predicate specifics
  (call-site counts, the "UI uses string compare in cancel flow" caveat
  for `is_active`, the "load-bearing" warning for `has_in_person`). The
  `manifest_entries_are_well_formed` test enforces non-empty non-placeholder
  reasons, which kept me honest.
- **Deciding whether to update the existing `is_approved` reason at all.**
  The user's handover rule says "Only update comments if related code
  changes." Adding three new entries to the same const array as
  `is_approved` is arguably a related change — the four entries now live
  side-by-side and would have looked inconsistent if one cited Phase 2.1
  and three cited Phase 2.2. I judged the consistency pass as in-scope;
  it's documented above so the choice is auditable.

### Where the result differs from the plan

R1 is the smallest possible fix that closes the Phase 2.3 silent gap. The
plan's Phase 2.2 task description ("Move all duplicated business predicates
into `domain/src/policy/`") imagines a much larger substantive refactor.
R1 deliberately does **not** do that — it documents the existing mirrors
rather than eliminating them. The substantive move/merge decision is R3
and remains open. The plan's Phase 2.2 checkbox is therefore `[~]`
(partial), not `[x]`.

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): ✅ CONCLUDED (handover 113)
- **Phase 2.2** (move predicates): 🟡 **PARTIAL — R1 done (this handover),
  R2/R3 open**
- **Phase 2.3** (CI dup-check): ✅ CONCLUDED (handover 112); scope gap
  closed by R1 this handover
- **Phase 2.4** (type-state escrow lifecycle): 🟡 Open — independent of 2.2
- **Phase 3** (policy traits): ✅ CONCLUDED
- **Phase 4.1** (profile): 🟡 Blocked on infra (200-attendee staged event)
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic guard): ✅ CONCLUDED
- **Phase 5.4** (zero-alloc audit): ✅ CONCLUDED
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied)

### Phase 2.2 still-open items (priority order)

1. **R2** — eliminate the two `DepositMethod` serialization sites in worker
   (`worker/src/handlers/attendee.rs:367-372` → `d.method.to_string()`;
   `worker/src/db/deposit_statuses.rs:352-360` → derive `FromStr` on
   domain `DepositMethod` or use serde). Zero behavior change; 308
   workspace tests are the safety net.
2. **R3** — substantive type-merge decision for `EventFormat`,
   `EscrowStatus`, `EventStatus`, `OnlineOpenMode` mirror types in
   `api/event.rs`. Three options documented in
   `.plans/014_ssot_audit.md`: (a) keep as documented mirrors
   (current state after R1); (b) replace with direct imports from domain;
   (c) replace with delegation wrappers. Options (b)/(c) eliminate
   divergence risk but require a `#[serde(default)]` audit on domain.
   Recommendation: dedicated session, not a drive-by.

### Immediate operator actions

- [ ] **Push** `develop` to `origin` — `git push origin develop`. After
  this handover's commits land, `develop` will be N commits ahead of
  `origin/develop` (N = handover-113's 2 + this handover's commits).
- [ ] **Deploy** the commits now sitting on `develop` to dev/prod via
  `develop → main → deploy.sh`. **Requires explicit operator
  confirmation.** No schema changes; rollback is `wrangler rollback`.

### Housekeeping

- [ ] **Delete feature branch** —
  `git branch -d feature/014_phase2_2_guard_widen` (after merge to develop).

### Explicitly deferred (do NOT do without confirmation)

- ❌ **R2 / R3** — do not implement without explicit scope approval. R1
  closes the Phase 2.3 guard gap; neither R2 nor R3 is urgent.
- ❌ **Deploy** — do not deploy without explicit operator confirmation.
- ❌ **Phase 2.4** (type-state escrow FSM) — independent track, larger
  scope; do not start without explicit go-ahead.

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 2.2 now `[~]`)
- Phase 2.1 audit findings: `.plans/014_ssot_audit.md` (R1 marked ✅ IMPLEMENTED)
- Phase 2.1 audit handover: `.handovers/113_plan_014_phase2_1_ssot_audit.md`
- Phase 2.3 guard handover (with live-injection protocol §2):
  `.handovers/112_plan_014_phase2_3_ssot_mirror_audit.md`
- Guard test file: `frontend-leptos/tests/ssot_mirror_audit.rs`

## 8. How to Dev / Test

### Read the change

```sh
git --no-pager log --oneline develop..feature/014_phase2_2_guard_widen
git --no-pager diff develop..feature/014_phase2_2_guard_widen -- \
  frontend-leptos/tests/ssot_mirror_audit.rs
```

### Re-run the guard

```sh
cd frontend-leptos
cargo test --test ssot_mirror_audit
# Expect: 9 passed; 0 failed
```

### Re-verify the widened scope (live-injection reproduction)

The protocol from handover 112 §2, extended to all three files in
`MIRROR_FILES`. Back up the source files first:

```sh
cd /Users/ozone/event-checkin
cp frontend-leptos/src/api/types.rs /tmp/types.rs.bak
cp frontend-leptos/src/api/event.rs /tmp/event.rs.bak
cp frontend-leptos/src/api/admin.rs /tmp/admin.rs.bak
```

For each file, inject a synthetic predicate into an existing `impl`
block, run the guard (must fail and name `is_early_bird_eligible`), then
restore from backup:

```sh
cd frontend-leptos
# Inject into src/api/event.rs (the newly-scoped file):
awk '{ print }
     /impl EscrowStatus \{/ {
       print "    pub fn is_early_bird_eligible(&self) -> bool { true }"
     }' src/api/event.rs > src/api/event.rs.tmp \
  && mv src/api/event.rs.tmp src/api/event.rs

cargo test --test ssot_mirror_audit
# Expect: FAILURE, message names is_early_bird_eligible

cp /tmp/event.rs.bak src/api/event.rs
cargo test --test ssot_mirror_audit
# Expect: 9 passed
```

Repeat for `src/api/types.rs` (use marker `impl CheckInStatus \{`) and
`src/api/admin.rs` (use marker `impl EscrowInstruction \{`).

### Confirm only the test file changed

```sh
cd /Users/ozone/event-checkin
git --no-pager status --short
# Expect: only frontend-leptos/tests/ssot_mirror_audit.rs (and the two
# .plans/ docs) modified; src/api/*.rs untouched.

git --no-pager diff frontend-leptos/src/api/types.rs \
                  frontend-leptos/src/api/event.rs \
                  frontend-leptos/src/api/admin.rs
# Expect: empty
```

### Full test suites

```sh
cd /Users/ozone/event-checkin/frontend-leptos
cargo test                              # Expect 159 passing (92+55+9+3)

cd /Users/ozone/event-checkin
cargo test --workspace --quiet          # Expect 308+ passing, 0 failed

cd /Users/ozone/event-checkin/frontend-leptos
cargo clippy --test ssot_mirror_audit   # Expect zero NEW warnings in test file
cargo check --all-targets               # Expect EXIT 0
cargo check --target wasm32-unknown-unknown  # Expect EXIT 0
```

### Relationship to R2 / R3

R1 closes the Phase 2.3 guard's silent scope gap. R2 (worker
`DepositMethod` serialization sites) and R3 (substantive mirror-type
merge decision) are independent follow-ups tracked in
`.plans/014_ssot_audit.md`. Neither is required to close the guard gap;
both are deferred until explicit scope approval.