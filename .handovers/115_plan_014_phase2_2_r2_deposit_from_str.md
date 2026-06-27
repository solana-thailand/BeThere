# Handover 115 — Plan 014 Phase 2.2 R2 Worker DepositMethod SSOT

> Branch: `feature/014_phase2_2_r2_deposit_from_str` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.2 (R2)
> Audit ref: `.plans/014_ssot_audit.md` §"Recommendations for Phase 2.2" R2

## 1. What Happened

Plan 014 Phase 2.2's **second-priority recommendation (R2)** — eliminating
the two `DepositMethod` serialization sites in the worker that hand-mapped
strings already produced by domain's `Display` impl — is implemented and
verified. **R3 (the substantive EventFormat/EscrowStatus type-merge
decision) remains open** and was deliberately not touched.

The handover-113 audit had identified two genuine removable duplication
sites in the worker: (1) `handlers/attendee.rs` re-implemented
`enum → string` for JSON output, and (2) `db/deposit_statuses.rs`
re-implemented `string → enum` for D1 row parsing. Both were
zero-behavior-change refactors with the test suite as the safety net. This
session executes that refactor cleanly: domain becomes the SSOT for both
directions of the `DepositMethod ↔ snake_case_string` mapping, and the
worker delegates via the standard `Display`/`FromStr` traits.

### The change in one paragraph

Domain `DepositMethod` gained a `FromStr` impl — the inverse of the
existing `Display` impl — with `type Err = String` and an error message
format (`unknown DepositMethod: '{other}'`) that exactly matches the
prior worker-side message so logs, e2e scripts, and any error-display
code see no behavior change. Worker `db/deposit_statuses.rs` replaced
its 4-arm `match method_str.as_str() { ... other => Err(...) }` block
with `DepositMethod::from_str(&method_str)?`; worker
`handlers/attendee.rs` replaced its 4-arm `match d.method { ... }`
block with `d.method.to_string()` (delegating to domain's `Display`).
Three new domain tests pin the exact wire strings and the error format
so a future drift is caught at test time. No frontend, no guard test,
and no production behavior changed.

### Audit-first verification before any change

Before editing anything, I re-verified the handover-113 audit's claims
against the actual code. The audit said domain `DepositMethod` already
had a `Display` impl producing the right strings; my first grep
(`impl.*Display.*for DepositMethod` in `domain/src/models/**/*.rs`)
returned **no matches**, which would have been a critical audit miss.
A broader grep (`DepositMethod` across the whole project) located the
enum in `domain/src/models/deposit.rs:12-21` and confirmed the `Display`
impl exists at L23-31, producing exactly `"usdc"`, `"thb"`,
`"credit_thb"`, `"credit_usdc"` — matching the worker's hand-coded
strings exactly. The audit's R2 claims are 100% accurate.

| Audit claim | Verified |
|---|---|
| Domain has `impl Display for DepositMethod` producing the right snake_case strings | ✅ `domain/src/models/deposit.rs:23-31` |
| Worker site 1 hand-maps `enum → string` for JSON output | ✅ `worker/src/handlers/attendee.rs` (~L364-370) |
| Worker site 2 hand-maps `string → enum` for D1 row parsing, with error format `unknown DepositMethod: '{other}'` | ✅ `worker/src/db/deposit_statuses.rs:352-360` |
| Refactor is zero-behavior-change | ✅ 308 workspace tests still pass (plus 3 new pinning tests = 311) |

### The FromStr error format decision

The audit offered two options for the string → enum direction: "derive
`FromStr` on domain `DepositMethod` (or use serde), preserving the
'unknown variant' error context". Two considerations drove the choice:

1. **Serde's error format is different.** Calling
   `serde_json::from_value::<DepositMethod>(Value::String(s))` would
   produce a message like `unknown variant 'foo', expected one of ...`.
   That loses the worker's existing `"unknown DepositMethod: '{other}'"`
   message shape, which e2e scripts and any log-scraping tooling may
   depend on.
2. **Domain already has a `FromStr` pattern.** `CheckInStatus` and
   `ParticipationType` both have `impl FromStr` in
   `domain/src/models/attendee.rs`. Adding `FromStr` to `DepositMethod`
   follows an established domain idiom and gives the worker a typed
   `parse()`/`from_str()` entry point — the SSOT discipline the plan
   calls for.

So `FromStr::Err = String` with the exact prior error format was chosen.
The third new domain test (`test_deposit_method_from_str_rejects_unknown_with_canonical_message`)
pins the message format so a future change cannot silently break the
worker's error contract.

### What R2 deliberately did NOT do

- **No mirror-type merge.** The mirror types in `api/event.rs`
  (`EventStatus`, `EscrowStatus`, `EventFormat`, `EventVisibility`,
  `OnlineOpenMode`) and `api/types.rs` (`CheckInStatus`, `DepositMethod`,
  `QrGenerationStatus`) are untouched. That is R3 and is explicitly
  deferred.
- **No frontend changes.** Frontend has its own `DepositMethod` mirror
  type in `api/types.rs` with its own `as_str()` — this is UI
  presentation (R4 in the audit explicitly recommends against migrating
  frontend `label()`/`as_str()`). Untouched.
- **No new domain predicate.** R4 in the audit explicitly recommends
  against adding `DepositMethod::is_usdc()` to "consolidate" the 9
  worker equality checks. Not added.
- **No guard test change.** The Phase 2.3 guard's `MIRROR_FILES` and
  `ALLOWED_MIRROR_PREDICATES` are unchanged from handover 114. R2 is
  worker-only; the guard only watches frontend mirror files.
- **No worker behavior change.** Both refactored sites produce the same
  outputs for the same inputs, including the same error message text.

## 2. Changes (3 files: 1 domain, 2 worker)

### Edited: `domain/src/models/deposit.rs` (+91)

Two structural additions:

1. **`impl FromStr for DepositMethod`** (after the existing `Display`
   impl) — inverse of `Display`, with `type Err = String` and error
   message `unknown DepositMethod: '{other}'` preserved verbatim from
   the prior worker-side format. A doc comment explains the SSOT role
   and the rationale for the error format choice.
2. **3 new tests in `mod tests`** (under a new
   `FromStr / Display round-trip (Plan 014 Phase 2.2 R2)` section):
   - `test_deposit_method_from_str_round_trip` — `Display → FromStr →
     identity` for every variant.
   - `test_deposit_method_from_str_wire_strings` — pins the exact
     snake_case wire strings (both `parse()` and `to_string()`), so any
     drift in `Display`, `FromStr`, or the `#[serde(rename_all =
     "snake_case")]` attribute is caught.
   - `test_deposit_method_from_str_rejects_unknown_with_canonical_message`
     — pins the error format that the worker now propagates directly
     via `?`. Includes empty string and PascalCase rejection cases.

Plus `use std::str::FromStr;` at the top of the file (matches the
existing pattern in `domain/src/models/attendee.rs`).

### Edited: `worker/src/handlers/attendee.rs` (+3/-6)

One targeted edit in `get_public_ticket`'s `deposit_info` block: the
4-arm `match d.method { ... }` that produced `"usdc"`/`"thb"`/etc.
strings for the JSON output is replaced with `d.method.to_string()`.
The match was using the fully-qualified path
`event_checkin_domain::models::deposit::DepositMethod::*` — `to_string()`
via `Display` is both shorter and SSOT. A two-line comment explains the
SSOT delegation and cites Plan 014 Phase 2.2 R2.

### Edited: `worker/src/db/deposit_statuses.rs` (+8/-8)

Two targeted edits:

1. **`use std::str::FromStr;`** added at the top of the file (after the
   doc comment, before the existing `use worker::D1Database;`).
2. In `row_to_deposit_status`: the 4-arm
   `match method_str.as_str() { ... other => Err(...) }` block (with its
   inline early-return-on-error) is replaced with the one-liner
   `let method = DepositMethod::from_str(&method_str)?;`. A four-line
   comment explains the SSOT delegation, the preserved error format,
   and cites R2.

The error format `unknown DepositMethod: '{other}'` is preserved
**exactly** because the domain `FromStr` impl emits that identical
string and the worker now propagates it directly via `?` (no
`map_err`).

### NOT changed (deliberately)

- `frontend-leptos/src/api/types.rs` — frontend's `DepositMethod`
  mirror with its own `as_str()` is UI presentation; R4 explicitly
  recommends against migrating it.
- `frontend-leptos/tests/ssot_mirror_audit.rs` — guard unchanged from
  handover 114.
- `worker/src/db/deposit_statuses.rs` write path
  (`upsert_deposit_status`) — uses serde serialize via
  `serde_json::to_value(&status)`, which already delegates to domain's
  `#[serde(rename_all = "snake_case")]`. No duplication to remove there.
- The 9 worker equality sites (`method == DepositMethod::Usdc` etc.) —
  type-safe comparisons, not serialization duplication. R4 explicitly
  recommends against adding `is_usdc()` for them.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Domain deposit tests | `cargo test -p event-checkin-domain deposit` | ✅ 20/20 (was 17, +3 new FromStr tests) |
| Workspace tests | `cargo test --workspace` | ✅ **311** (was 308, +3 new), 0 failed |
| Frontend tests | `cargo test` in `frontend-leptos/` | ✅ **159** (unchanged — R2 does not touch frontend) |
| Clippy on domain | `cargo clippy -p event-checkin-domain` | ✅ Zero new warnings |
| Clippy on worker | `cargo clippy -p event-checkin-worker` | ✅ Zero new warnings |
| Diagnostics on edited files | IDE diagnostics on all 3 edited `.rs` files | ✅ Zero errors, zero warnings |
| Worker site 1 round-trip | Inspected diff: `match d.method { ... }` → `d.method.to_string()` | ✅ Same JSON output via domain `Display` |
| Worker site 2 error format | Inspected diff + new test pins the exact message | ✅ `unknown DepositMethod: '{other}'` preserved verbatim |

### Test count growth

- Domain `deposit.rs`: **17 → 20** (+3 from R2's new pinning tests)
- Workspace total: **308 → 311** (matches the +3 above)
- Frontend: **159** unchanged (R2 does not touch frontend)
- Guard tests: **9/9** unchanged (R2 does not touch the guard)

### Why no live-injection test this time

Handover 114's R1 work added a regression guard (the Phase 2.3
mirror-types audit) — that needed a live-injection test to prove the
guard actually fires. R2 here is **not** a guard; it's a refactor of
production code that delegates to existing trait impls. The safety net
is the **308 existing tests** (now 311 with the pinning tests), not a
negative-test harness. The relevant proofs are: (a) all 308 prior tests
still pass, confirming zero behavior change; (b) the 3 new pinning
tests confirm the wire strings and error format are stable.

## 4. Plan / Code / Test Locations

- **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.2
  (still `[~]` partial — R1 and R2 done; only R3 open).
- **Audit doc**: `.plans/014_ssot_audit.md` — R2 marked ✅ IMPLEMENTED
  with status line and verification summary; conclusion updated.
- **Domain SSOT**:
  - `domain/src/models/deposit.rs` `enum DepositMethod` (L12-21)
  - `impl std::fmt::Display for DepositMethod` (L23-31, unchanged)
  - `impl FromStr for DepositMethod` (L36-51, **new**)
  - 3 new tests at end of `mod tests` (L379-449)
- **Worker site 1** (serialization → JSON):
  `worker/src/handlers/attendee.rs` in `get_public_ticket`'s
  `deposit_info` block (~L364-370).
- **Worker site 2** (D1 row → enum):
  `worker/src/db/deposit_statuses.rs` in `row_to_deposit_status`
  (~L351-359).
- **Frontend mirror (NOT touched, R4)**:
  `frontend-leptos/src/api/types.rs` `DepositMethod::as_str()`.

## 5. Reflections

### What went well

- **The audit-first discipline caught a near-miss.** My initial grep
  for `impl.*Display.*for DepositMethod` in
  `domain/src/models/**/*.rs` returned zero matches and would have
  invalidated R2's premise. A broader grep across the whole project
  found the enum and confirmed the `Display` impl exists exactly as
  the audit described. Lesson reinforced: when a targeted grep returns
  nothing, broaden the scope before concluding the audit was wrong.
  This time the audit was right; the grep was too narrow.
- **The error-format-preservation decision paid off immediately.**
  Choosing `FromStr::Err = String` with the exact prior message format
  meant the worker change was a clean one-liner
  (`DepositMethod::from_str(&method_str)?`) with no `map_err` glue,
  AND the third pinning test could assert byte-equal error text.
  Had I used serde, the worker would need `map_err` to reformat and
  the test would have been weaker.
- **The pinning tests are the real deliverable.** The refactor itself
  is mechanically small — two match blocks replaced by trait calls.
  The lasting value is the 3 tests that pin the wire strings and the
  error format. A future contributor who tries to change `"usdc"` to
  `"USDC"` (or vice versa) will get a clear test failure pointing at
  every downstream consumer (D1 column, JSON API, e2e scripts).
- **The domain `FromStr` pattern was already established.** Adding
  `impl FromStr for DepositMethod` next to `CheckInStatus::from_str`
  and `ParticipationType::from_str` feels native — no new idiom
  introduced, no new dependency, no surprise for future readers.

### What was harder than expected

- **Deciding whether to add `FromStr` to domain vs. use serde at the
  worker call site.** The audit offered both options. The deciding
  factors were (a) preserving the worker's existing error format
  (serde's `unknown variant 'foo', expected ...` is different) and
  (b) following the existing domain `FromStr` pattern. I considered a
  third option — deriving `FromStr` from serde at runtime via
  `serde_json::from_value` — and rejected it as too clever and
  dependent on the `rename_all` attribute being stable. The explicit
  `FromStr` impl is the most readable and the most testable.
- **Resisting scope creep into the 9 equality sites.** The audit
  flagged `method == DepositMethod::Usdc` as "smell only" (R4
  recommends against `is_usdc()`). It would have been easy to
  "consolidate" them while in the file. I left them alone — the audit
  explicitly recommends against it, the user's handover rule says
  "make only essential changes", and the 9 sites are type-safe already.
- **Writing the doc comments.** Each edit needed a comment explaining
  the SSOT delegation and citing Plan 014 Phase 2.2 R2, so a future
  reader can find the audit and the rationale. The comments are short
  (2-4 lines each) but took a few iterations to keep concise without
  losing the audit trail.

### Where the result differs from the plan

R2 is the smallest possible refactor that eliminates the cross-crate
`DepositMethod` serialization duplication. The plan's Phase 2.2 task
description ("Move all duplicated business predicates into
`domain/src/policy/`") imagines a much larger substantive refactor that
would also resolve R3. R2 deliberately does **not** do that — it
eliminates the two flagged serialization sites without touching the
mirror types themselves. The substantive merge-vs-delegate-vs-keep
decision for mirror types is R3 and remains open. The plan's Phase 2.2
checkbox is therefore still `[~]` (partial), not `[x]`.

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): ✅ CONCLUDED (handover 113)
- **Phase 2.2** (move predicates): 🟡 **PARTIAL — R1 done (handover 114),
  R2 done (this handover), only R3 open**
- **Phase 2.3** (CI dup-check): ✅ CONCLUDED (handover 112); scope gap
  closed by R1 in handover 114
- **Phase 2.4** (type-state escrow lifecycle): 🟡 Open — independent of 2.2
- **Phase 3** (policy traits): ✅ CONCLUDED
- **Phase 4.1** (profile): 🟡 Blocked on infra (200-attendee staged event)
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic guard): ✅ CONCLUDED
- **Phase 5.4** (zero-alloc audit): ✅ CONCLUDED
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied)

### Phase 2.2 still-open items

1. **R3** — substantive type-merge decision for the mirror types
   (`EventFormat`, `EscrowStatus`, `EventStatus`, `OnlineOpenMode` in
   `api/event.rs`; `CheckInStatus`, `DepositMethod`, `QrGenerationStatus`
   in `api/types.rs`). Three options documented in
   `.plans/014_ssot_audit.md`:
   - (a) **keep as documented mirrors** — current state after R1; the
     guard's `ALLOWED_MIRROR_PREDICATES` is the documentation;
   - (b) **replace with direct imports from domain** — eliminates
     divergence risk entirely but requires domain to gain
     `#[serde(default)]` annotations for partial-JSON safety;
   - (c) **replace with delegation wrappers** — middle ground; the
     frontend types stay but their methods call into domain.

   This is a design decision, not a refactor. It needs its own session.
   It is **not** required to close the Phase 2.3 guard gap — R1 already
   did that, and R2 just eliminated the only genuine cross-crate
   serialization duplication the audit found.

### Immediate operator actions

- [ ] **Push** `develop` to `origin` — `git push origin develop`. After
  this handover's commits land, `develop` will be N commits ahead of
  `origin/develop` (N = handover-113's 2 + handover-114's 2 + this
  handover's commits).
- [ ] **Deploy** the commits now sitting on `develop` to dev/prod via
  `develop → main → deploy.sh`. **Requires explicit operator
  confirmation.** No schema changes; rollback is `wrangler rollback`.

### Housekeeping

- [ ] **Delete feature branch** —
  `git branch -d feature/014_phase2_2_r2_deposit_from_str` (after merge
  to develop).
- [ ] **Optional cleanup**: `feature/014_ssot_audit` (handover 113) and
  `feature/014_phase2_2_guard_widen` (handover 114) are also safe to
  delete — both merged to `develop` via fast-forward.

### Explicitly deferred (do NOT do without confirmation)

- ❌ **R3** — substantive mirror-type merge decision. Do not start
  without explicit scope approval and a dedicated session.
- ❌ **Deploy** — do not deploy without explicit operator confirmation.
- ❌ **Phase 2.4** (type-state escrow FSM) — independent track, larger
  scope; do not start without explicit go-ahead.
- ❌ **R4 items** — explicitly recommended against by the audit; do not
  add `DepositMethod::is_usdc()` or migrate frontend UI helpers.

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 2.2 still
  `[~]`, but only R3 open)
- Phase 2.1 audit findings: `.plans/014_ssot_audit.md` (R2 marked ✅
  IMPLEMENTED)
- Phase 2.1 audit handover: `.handovers/113_plan_014_phase2_1_ssot_audit.md`
- Phase 2.2 R1 handover (guard scope fix):
  `.handovers/114_plan_014_phase2_2_r1_guard_widen.md`
- Phase 2.3 guard handover (with live-injection protocol §2):
  `.handovers/112_plan_014_phase2_3_ssot_mirror_audit.md`
- Domain SSOT: `domain/src/models/deposit.rs` (`Display` + `FromStr`)
- Existing `FromStr` pattern in domain: `domain/src/models/attendee.rs`
  (`CheckInStatus::from_str`, `ParticipationType::from_str`)
- Frontend mirror (NOT touched, R4): `frontend-leptos/src/api/types.rs`
  `DepositMethod::as_str()`

## 8. How to Dev / Test

### Read the change

```sh
git --no-pager log --oneline develop..feature/014_phase2_2_r2_deposit_from_str
git --no-pager diff develop..feature/014_phase2_2_r2_deposit_from_str
```

### Re-run the new domain tests

```sh
cd /Users/ozone/event-checkin
cargo test -p event-checkin-domain deposit
# Expect: 20 passed (was 17 before R2; +3 from the new pinning tests)
```

The three new tests live at the end of `mod tests` in
`domain/src/models/deposit.rs`:

- `test_deposit_method_from_str_round_trip`
- `test_deposit_method_from_str_wire_strings`
- `test_deposit_method_from_str_rejects_unknown_with_canonical_message`

### Confirm zero behavior change in worker

The two refactored worker sites are covered indirectly by the
workspace test suite — any deposit-status read or ticket-page render
exercise them. Run the full workspace test suite:

```sh
cd /Users/ozone/event-checkin
cargo test --workspace
# Expect: 311 passed (was 308 before R2; +3 new domain pinning tests)
```

### Confirm the error format is preserved

The third new test pins the exact error message text. To verify
manually:

```sh
cd /Users/ozone/event-checkin
cargo test -p event-checkin-domain \
  test_deposit_method_from_str_rejects_unknown_with_canonical_message -- --nocapture
```

The test asserts that `"bitcoin".parse::<DepositMethod>()` produces
exactly `"unknown DepositMethod: 'bitcoin'"` — the same string the
worker's prior 4-arm match produced, and the same string the worker
now propagates via `?`.

### Confirm frontend is untouched

```sh
cd /Users/ozone/event-checkin/frontend-leptos
cargo test                                # Expect 159 passing (unchanged)

cd /Users/ozone/event-checkin
git --no-pager diff develop..feature/014_phase2_2_r2_deposit_from_str -- \
  frontend-leptos/
# Expect: empty (no frontend files changed)
```

### Clippy

```sh
cd /Users/ozone/event-checkin
cargo clippy -p event-checkin-domain      # Expect zero new warnings
cargo clippy -p event-checkin-worker      # Expect zero new warnings
```

### Relationship to R1 / R3

R1 (handover 114) closed the Phase 2.3 guard's silent scope gap by
widening `MIRROR_FILES` and documenting the 3 previously-uncovered
mirrored predicates. R2 (this handover) eliminated the only genuine
cross-crate serialization duplication the audit found — the two worker
sites that hand-mapped strings domain already provided via `Display`.
R3 (the substantive mirror-type merge decision) is independent of both
and remains open. None of R1/R2/R3 is required for the others to ship;
each closes its own piece of the SSOT discipline.

### Reproducing the audit-first verification

Before any edit, the audit's claims were re-verified against the
actual code. To reproduce:

```sh
cd /Users/ozone/event-checkin

# 1. Confirm domain DepositMethod has a Display impl producing the right strings:
rg "impl.*Display.*for DepositMethod" domain/
rg 'Self::Usdc => write!\(f, "usdc"\)' domain/src/models/deposit.rs

# 2. Confirm worker site 1 (handlers/attendee.rs) hand-maps enum → string:
rg "DepositMethod::Usdc => \"usdc\"" worker/

# 3. Confirm worker site 2 (db/deposit_statuses.rs) hand-maps string → enum:
rg '"usdc" => DepositMethod::Usdc' worker/

# 4. Confirm the error format the audit quoted:
rg "unknown DepositMethod" worker/
```

All four searches returned the matches the audit claimed (before this
handover's edits). After this handover, searches 2 and 3 return zero
matches in `worker/` — the duplication is gone, replaced by trait
delegation to domain.