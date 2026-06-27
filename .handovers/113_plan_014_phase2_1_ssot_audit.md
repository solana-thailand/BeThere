# Handover 113 — Plan 014 Phase 2.1 Cross-Crate SSOT Audit

> Branch: `feature/014_ssot_audit` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.1

## 1. What Happened

Plan 014 Phase 2.1 — the **cross-crate Single Source of Truth audit** scanning
for duplicated business logic across `worker/src/` and `frontend-leptos/src/`
— is complete. The deliverable is a findings document,
`.plans/014_ssot_audit.md` (434 lines, file:line evidence throughout). **No
code was changed**; this was a pure read-only audit.

The audit-first discipline was applied to all three candidates the plan named:
participation-type normalization, `DepositMethod` enum mapping, and
escrow-state predicates. As with six prior Plan 014 audits, the plan's stated
premises did not match the codebase — but this time the audit surfaced a
**real, actionable finding** that the shipped Phase 2.3 guard had missed.

### The 7th consecutive audit miss — and this one found something real

The pattern continued: the plan's premises did not survive contact with the
codebase.

| # | Plan's premise | Reality |
|---|---|---|
| 1 | Phase 1.4 EventMetaWire fixed shape | Variable strings; Pod 26% larger than JSON |
| 2 | Phase 1.5 DepositStatusWire 1-day work | Needs base58 + ID length policy (~2 days) |
| 3 | Phase 4.3.1 event-series endpoint uncached | Already cached at 120s (Plan 013) |
| 4 | Phase 4.3.3 quiz does one PUT per answer | Batches in-memory, writes once |
| 5 | Phase 4.3.4 blockhash valid ~120s | Confuses ring-buffer with `MAX_PROCESSING_AGE` (~60–90s) |
| 6 | Phase 2.3 grep against `domain::policy` | `domain::policy` never created; 18 predicates are methods on `domain::models::*` |
| **7** | **Phase 2.1: "DepositMethod → string in 3+ places", escrow predicates duplicated** | **Only 2 serialization sites exist; participation-type is NOT duplicated — BUT the audit uncovered that the Phase 2.3 guard's scope hides 3 load-bearing mirrored predicates** |

The difference this time: audit #7 produced a **real defect** in a previously
shipped guard. The Phase 2.3 mirror-types guard
(`frontend-leptos/tests/ssot_mirror_audit.rs`) declared
`MIRROR_FILES = &["src/api/types.rs"]`, on the stated assumption (its own doc
comment, L40-44) that `api/types.rs` was the only file with the
`/// Mirrors domain::` doc-comment pattern. That text-pattern assumption was
**wrong at ship time**.

`frontend-leptos/src/api/event.rs` contains four mirror types using lowercase
"mirrors backend X" wording (`EventStatus`, `EscrowStatus`, `EventFormat`,
`EventVisibility`). Three of them carry **business predicates** that are
silently uncovered:

| Predicate | Mirror location | Frontend call sites | In Phase 2.3 allowlist? |
|---|---|---|---|
| `CheckInStatus::is_approved()` | `api/types.rs` | — | ✅ Yes |
| `EventFormat::has_in_person()` | `api/event.rs` | **7** (admin ×4, event_form ×1, scanner ×2) | ❌ No — uncovered |
| `EventFormat::has_online()` | `api/event.rs` | 1 (admin) | ❌ No — uncovered |
| `EscrowStatus::is_active()` | `api/event.rs` | 0 (defined; UI uses string compare in cancel flow) | ❌ No — uncovered |

`EventFormat::has_in_person()` is load-bearing: if domain tightens it (e.g.
adds a 4th format variant), the frontend mirror silently diverges and the UI
shows the wrong nav groups, scanner buttons, and deposit gates. This is
exactly the silent-divergence risk the Phase 2.3 guard was built to prevent.

### Candidate-by-candidate reality

- **Participation-type normalization — NOT duplicated.** Domain provides the
  full SSOT (`ParticipationType::parse`/`as_str`/`display` in
  `domain/src/models/attendee.rs`). Worker's `normalize_override`
  (`worker/src/handlers/attendee.rs:1052`) is a thin HTTP-input validation
  wrapper that delegates to domain and adds worker-specific empty/walkin
  rejection. Frontend carries the raw `String` and never parses client-side.
  Legitimate separation of concerns (same pattern as Phase 3.1's two-stage
  guard / negative-results #8).
- **DepositMethod enum mapping — 2 genuine removable sites (not "3+").**
  Worker hand-maps `enum → string` at `handlers/attendee.rs:367-372`
  (duplicates `Display::to_string()`) and `string → enum` at
  `db/deposit_statuses.rs:352-360` (duplicates serde with
  `rename_all = "snake_case"`). The plan's "3+ places" appears to have
  miscounted the 9 type-safe equality checks (`method == DepositMethod::Usdc`)
  as serialization — they are not duplication, just a mild ergonomic smell.
  Frontend `label()`/`icon_name()` are legitimately UI presentation.
- **Escrow/EventFormat predicates — the headline finding.** Three mirrored
  business predicates in `api/event.rs` are uncovered by the Phase 2.3 guard
  (details above). Worker side is clean: it imports domain's typed
  `EscrowStatus` directly; zero status-string comparisons found.

### Domain predicate baseline re-verified

Re-counted **18** `pub fn (is_|can_|has_|should_|requires_|allows_)*(&self)`
methods in `domain/src/models/`. **Matches the Phase 2.3 baseline exactly.**
No drift.

## 2. Changes (1 new doc, 1 doc edit)

### New: `.plans/014_ssot_audit.md` (434 lines)

The audit findings document. Structured as:

1. **Executive summary** — the headline finding up front, with a 3-row
   candidate reality table.
2. **Candidate 1: Participation-type normalization** — verdict: NOT duplicated.
3. **Candidate 2: DepositMethod mapping** — three sub-sections (2a genuine
   removable, 2b equality-comparison smell, 2c frontend UI helpers legit).
4. **Candidate 3: Escrow/EventFormat predicates** — the headline finding,
   with the Phase 2.3 guard scope-gap root cause and worker-side check.
5. **Domain predicate baseline** — 18-method table, re-verified.
6. **Cross-reference inventory** — all 4 mirrored business predicates
   (1 covered + 3 uncovered).
7. **Recommendations R1-R4 for Phase 2.2** (documented, not implemented).
8. **What this audit refuses to claim** — inline re-implementations, serde
   contract compatibility, worker-internal duplication.
9. **Audit method** — every `rg` search documented for reproducibility.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md`

Phase 2.1 task entry flipped from `[ ]` to `[x]` with an inline summary
matching the format of the other six concluded Plan 014 phase entries. The
summary covers the three candidate verdicts, the Phase 2.3 guard scope-gap
finding, the domain baseline re-verification, and pointers to R1-R3.

### NOT changed (deliberately)

- **No code.** No `.rs` files touched. Test counts unchanged (308 workspace +
  159 frontend). No regression risk because nothing ran.
- **No Phase 2.2.** The plan explicitly gates 2.2 on 2.1's output ("the
  remaining candidates ... still need their own audit (Task 2.1) before any
  moves"). 2.2 remains open pending review of these findings.
- **No R1/R2/R3 implementation.** The recommendations are documented in the
  audit; none are executed.
- **No deploy.** Operator action, deferred pending explicit confirmation.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Branch isolation | `git status -sb` on `feature/014_ssot_audit` | Only `.plans/` files in diff |
| Diffstat before commit | `git diff --cached --stat` | 2 files, +472/-1, zero `.rs` |
| Domain predicate count | `rg "pub fn (is_\|can_\|has_\|should_\|requires_\|allows_)[a-z_]+\s*\(\s*&self"` in `domain/src/` | 18 (matches Phase 2.3 baseline) |
| Frontend mirror inventory | `rg "mirrors backend\|Mirrors domain"` in `frontend-leptos/src/api/` | 6 matches across `api/event.rs` (4) + `api/admin.rs` (2) |
| `is_active()` call sites | `rg "is_active"` in `frontend-leptos/src/` | 1 match (the definition itself) — confirms "defined, not called" |
| Worker escrow string comparisons | `rg 'escrow_status.*==.*"(none\|initialized\|deactivated\|closed\|cancelled)"'` in `worker/src/` | 0 matches — worker uses typed enum |
| Test count | (unchanged — no code modified) | 308 workspace + 159 frontend (not re-run) |

No test run was warranted: the diff is markdown-only.

## 4. Plan / Code / Test Locations

- **Audit findings (the deliverable):** `.plans/014_ssot_audit.md`
- **Plan file (Phase 2.1 marked `[x]`):** `.plans/014_katgpt_rs_paradigm_migration.md`,
  Phase 2.1 task entry
- **The guard with the scope gap:** `frontend-leptos/tests/ssot_mirror_audit.rs`
  — `MIRROR_FILES` constant at L88, doc-comment assumption at L40-44
- **The uncovered mirror file:** `frontend-leptos/src/api/event.rs` —
  `EscrowStatus` at L21-47, `EventFormat` at L50-83
- **The genuine removable worker sites:**
  - `worker/src/handlers/attendee.rs:367-372` (enum→string, dup of `Display`)
  - `worker/src/db/deposit_statuses.rs:352-360` (string→enum, dup of serde)
- **Previous handover (Phase 2.3, whose guard this audit scoped):**
  `.handovers/112_plan_014_phase2_3_ssot_mirror_audit.md`

## 5. Reflections

### What went well

- **The audit-first discipline paid off again** — but differently than the
  prior six. This time the audit didn't just correct the plan's premises; it
  found a real gap in a previously shipped guard. That's the more valuable
  outcome: a pure planning-miss is a no-op, but a guard-scope miss is a
  latent defect.
- **Verifying the `is_active()` "not called" claim separately.** The initial
  grep grouped `is_active`/`has_in_person`/`has_online` together. Before
  writing the claim into the audit doc, a dedicated `rg "is_active"` was run
  in `frontend-leptos/src/` to confirm zero call sites (only the definition
  matched). This avoided overclaiming that `is_active()` is load-bearing when
  it is in fact defined-but-unused.
- **Correcting the call-site count.** First draft of the audit doc said
  `has_in_person()` had "6 call sites"; a recount (admin ×4 + event_form ×1
  + scanner ×2) gave 7. The doc was corrected before commit. Honesty over
  convenience.
- **"What this refuses to claim" section.** Explicitly documenting out-of-
  scope items (inline re-implementations, serde contract compatibility,
  worker-internal duplication) prevents the audit from overclaiming and
  scopes future work honestly.

### What was harder than expected

- **Reconciling "16 matches" vs 18 predicates.** The grep header reported 16
  matches but a manual enumeration of the matched methods gave 18. The
  discrepancy was grouping (impl blocks counted as one match). Resolved by
  listing every method explicitly in a table and counting rows.
- **Distinguishing "serialization duplication" from "equality-comparison
  smell."** The plan said "3+ places" of `DepositMethod` mapping. Sorting
  the 18 grep hits into genuine serialization (2 sites) vs. type-safe
  equality (9 sites) vs. construction (rest) required reading each site's
  context, not just pattern-counting. The plan's count was wrong because it
  conflated these categories.

### Where the result differs from the plan

The plan expected Phase 2.1 to produce a list of duplicated logic to move in
2.2. The audit produced a more valuable artifact: **a scope defect in the
Phase 2.3 guard**, plus a much smaller genuine-duplication list than
anticipated (2 sites, not "3+"). The substantive 2.2 work is now refocused:
the highest-priority item (R1) is fixing the guard's `MIRROR_FILES` scope,
not moving business predicates.

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): ✅ **CONCLUDED (this handover)** — findings in
  `.plans/014_ssot_audit.md`; Phase 2.3 guard scope gap identified
- **Phase 2.2** (move duplicated predicates): 🟡 **Open, gated on review of
  2.1 findings** — recommended scope in R1-R3 (R1 = widen guard + allowlist
  the 3 uncovered predicates; R2 = eliminate 2 `DepositMethod` sites; R3 =
  defer EventFormat/EscrowStatus type-merge decision)
- **Phase 2.3** (forward-looking CI dup-check): ✅ CONCLUDED — **but has a
  known scope gap** (R1 above); the guard works correctly *within its
  declared scope*, the scope is just too narrow
- **Phase 2.4** (type-state escrow lifecycle): 🟡 Open — independent of 2.1/2.2
- **Phase 3** (policy traits): ✅ CONCLUDED (trait demoted, entry #7)
- **Phase 4.1** (profile): 🟡 Blocked on infra
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic lint): ✅ CONCLUDED
- **Phase 5.4** (zero-alloc audit): ✅ CONCLUDED
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied)

### What's next (priority order)

1. **Deploy** the commits now sitting on `develop` (14 from prior handovers +
   this handover's 1 commit = **15 commits behind main**) via
   `develop → main → deploy.sh`. Operator action, requires explicit
   confirmation. No schema changes; rollback is `wrangler rollback`.
2. **Review Phase 2.1 findings** (`.plans/014_ssot_audit.md`) and decide
   Phase 2.2 scope. R1 (widen the guard) is the highest-value, lowest-risk
   follow-up; R2 is a zero-behavior-change refactor; R3 is a larger
   type-merge decision best deferred.
3. **Phase 2.4** (type-state escrow lifecycle FSM) — independent of 2.1/2.2.

### Phase 2.2 re-open preconditions (carry-forward from R1-R3)

Phase 2.2 should be opened once one of these is true:

- **R1 is approved** — widen `MIRROR_FILES` to
  `&["src/api/types.rs", "src/api/event.rs", "src/api/admin.rs"]` and add
  the 3 uncovered predicates (`EscrowStatus::is_active`,
  `EventFormat::has_in_person`, `EventFormat::has_online`) to
  `ALLOWED_MIRROR_PREDICATES` with documented reasons. Requires re-running
  the live-injection verification (handover 112 §2) to confirm the widened
  guard still fires on undeclared predicates in the newly-scanned files.
- **R2 is approved** — eliminate the two `DepositMethod` serialization sites
  in worker. Zero behavior change; the 308-test workspace suite is the
  safety net.
- **R3 is approved** — decide whether `EventFormat`/`EscrowStatus`/
  `EventStatus`/`OnlineOpenMode` mirror types in `api/event.rs` should be
  kept as documented mirrors, replaced by direct domain imports, or replaced
  by delegation wrappers. Domain is already WASM-compatible (Plan 014
  Phase 1 confirmed this).

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md`
- Plan 014 Phase 2.1 inline conclusion: ibid., Phase 2.1 task entry
- Phase 2.1 audit findings (the deliverable): `.plans/014_ssot_audit.md`
- Plan 014 negative results log: `.plans/014_negative_results.md`
  (Phase 2.1 is a mixed result — a real finding plus the usual premise-miss —
  so no clean negative-results entry; the nuance is captured in the audit doc)
- Previous handover (Phase 2.3, whose guard scope this audit corrected):
  `.handovers/112_plan_014_phase2_3_ssot_mirror_audit.md`
- Phase 5.3 deterministic guard: `.handovers/111_plan_014_phase5_3_deterministic_guard.md`
- Phase 5.4 zero-alloc audit: `.handovers/110_plan_014_phase5_4_zero_alloc_audit.md`
- Phase 4.3 I/O wins: `.handovers/109_plan_014_phase4_3_io_wins.md`
- Phase 1 wire format (where mirror types were first documented as
  deliberate): `.handovers/108_plan_014_phase1_wire_format.md`

## 8. How to Dev / Test

This was a docs-only handover. There is no new code to run. The artifacts are
two markdown files; verification is reading them.

### Read the audit findings

```sh
bat .plans/014_ssot_audit.md            # the deliverable
bat .plans/014_katgpt_rs_paradigm_migration.md  # Phase 2.1 entry marked [x]
```

### Reproduce the headline finding (Phase 2.3 guard scope gap)

```sh
# 1. The guard's scope constant — only one file:
rg "MIRROR_FILES" frontend-leptos/tests/ssot_mirror_audit.rs
# → const MIRROR_FILES: &[&str] = &["src/api/types.rs"];

# 2. The actual mirror types in the uncovered file:
rg "mirrors backend" frontend-leptos/src/api/event.rs
# → 4 matches: EventStatus, EscrowStatus, EventFormat, EventVisibility

# 3. The uncovered business predicates (identical match arms to domain):
rg "pub fn (is_active|has_in_person|has_online)" frontend-leptos/src/api/event.rs
# → 3 matches

# 4. Compare against domain:
rg "pub fn (is_active|has_in_person|has_online)" domain/src/models/event.rs
# → 3 matches, same signatures, same bodies
```

### Reproduce the secondary finding (DepositMethod serialization sites)

```sh
rg "DepositMethod::(Usdc|Thb|CreditThb|CreditUsdc)" worker/src/ \
  | rg '=>"|=> DepositMethod'
# → 2 serialization sites (attendee.rs:367, deposit_statuses.rs:352)
# The other ~9 hits are type-safe equality comparisons, not duplication.
```

### Verify the domain predicate baseline (should be 18)

```sh
rg "pub fn (is_|can_|has_|should_|requires_|allows_)[a-z_]+\s*\(\s*&self" \
  domain/src/models/
# Count distinct method definitions (not match groups): 18
```

### Confirm no code changed in this handover

```sh
git --no-pager log --oneline -1 develop   # 79084b4 docs(plan-014): ...
git --no-pager show --stat 79084b4
# → 2 files changed, .plans/014_ssot_audit.md (new) +
#   .plans/014_katgpt_rs_paradigm_migration.md (edit), all markdown
```

### Relationship to Phase 2.2

Phase 2.2 is **gated on review of this audit's findings**. Do not start 2.2
work until the findings in `.plans/014_ssot_audit.md` are reviewed and a
scope decision is made (R1 only? R1+R2? all three?). The plan's own
sequencing requires this: Phase 2.2's task text says "the remaining
candidates ... still need their own audit (Task 2.1) before any moves" —
that audit is now done; the moves await approval.