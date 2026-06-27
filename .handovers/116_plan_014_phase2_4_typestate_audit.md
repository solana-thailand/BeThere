# Handover 116 — Plan 014 Phase 2.4 Type-State Escrow Audit

> Branch: `feature/014_phase2_4_audit` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.4
> Audit ref: `.plans/014_phase2_4_typestate_audit.md` (new — full findings)

## 1. What Happened

Plan 014 Phase 2.4 — the proposal to **type-state the escrow lifecycle as a
compile-time FSM** (`Created → DepositOpen → CheckedIn → Refundable → Claimable
→ Closed`) — is **concluded as a negative result** after a read-only
audit-first pass. **No code was changed**; this was a pure audit. The deliverable
is a findings document (`.plans/014_phase2_4_typestate_audit.md`, 480 lines,
file:line evidence throughout) plus a negative-results log entry (#10).

This is the **8th consecutive Plan 014 audit miss**, in the same structural
pattern as the previous seven: the plan's premise describes a state machine
that doesn't match the codebase, and the audit's value is in preventing the
wrong refactor from shipping.

### The 8th consecutive audit miss — full table

| # | Plan's premise | Reality |
|---|---|---|
| 1 | Phase 1.4 EventMetaWire fixed shape | Variable strings; Pod 26% larger than JSON |
| 2 | Phase 1.5 DepositStatusWire 1-day work | Needs base58 + ID policy (~2 days) |
| 3 | Phase 4.3.1 event-series endpoint uncached | Already cached at 120s (Plan 013) |
| 4 | Phase 4.3.3 quiz does one PUT per answer | Batches in-memory, writes once |
| 5 | Phase 4.3.4 blockhash valid ~120s | Confuses ring-buffer with `MAX_PROCESSING_AGE` (~60–90s) |
| 6 | Phase 2.3 grep against `domain::policy` | `domain::policy` never created; 18 predicates are methods on `domain::models::*` |
| 7 | Phase 2.1: "DepositMethod → string in 3+ places", escrow predicates duplicated | Only 2 serialization sites exist; participation-type NOT duplicated — BUT the Phase 2.3 guard's scope hides 3 load-bearing mirrored predicates |
| **8** | **Phase 2.4: type-state the 6-state escrow FSM at compile time** | **5-state event-level enum + per-attendee booleans + on-chain booleans; runtime transition allowlist already exists; the 6 states don't map to any single typed surface** |

### The headline finding — three separate state surfaces, not one FSM

The plan's premise conflates **three independent state surfaces** into a single
6-state typed FSM. None of the three matches the plan's list:

**Surface 1 — Event-level escrow** (`domain::models::event::EscrowStatus`):
```rust
// domain/src/models/event.rs:117-129
/// On-chain escrow lifecycle status.
/// Tracks the state machine: None → Initialized → Deactivated → Closed.
pub enum EscrowStatus { None, Initialized, Deactivated, Closed, Cancelled }
```
**5 states, not 6.** Doc comment explicitly names the state machine. The
`Cancelled` variant is a parallel recovery state (refunds-in-progress after
organizer cancellation). One `EscrowStatus` per `EventConfig`, mirrored on
`EventMeta` and on the frontend's `api/event.rs`.

**Surface 2 — Per-attendee deposit** (`DepositStatus`, `ThbDeposit`):
```rust
// domain/src/models/deposit.rs:57-89
pub struct DepositStatus {
    pub verified: bool,        // ← boolean flag, not state
    pub refundable: bool,      // ← boolean flag, not state
    pub rejected: bool,        // ← boolean flag, not state
    // ...
}
pub struct ThbDeposit {
    pub verified: bool,        // ← boolean flag
    pub refunded: bool,        // ← boolean flag
    // ...
}
```
**No typed state at all.** The plan's `CheckedIn` and `Claimable` states are
not even represented off-chain — they live only on the on-chain
`AttendeeDeposit` account. `CheckedIn` is on `Attendee` (via
`Attendee::is_checked_in()`), not on `DepositStatus`.

**Surface 3 — On-chain program** (`bethere-escrow/src/state.rs`):
```rust
pub struct EventEscrow {
    pub is_active: bool,       // ← single boolean; "states" are implicit
    // ...
}
pub struct AttendeeDeposit {
    pub checked_in: bool,      // ← boolean flag
    pub refunded: bool,        // ← boolean flag
    // ...
}
```
**No status enum on-chain.** "Initialized" = account exists + `is_active == true`;
"Deactivated" = `is_active == false`; "Closed" = account closed/reclaimed. The
off-chain `EscrowStatus` is a **projection/summary** of these on-chain facts,
kept in sync manually by the worker.

### Why the plan's 6 states don't map

The plan's states are a synthesis of two genuinely different lifecycles:

| Plan's state | Event-level `EscrowStatus`? | Per-attendee `DepositStatus`? | On-chain? |
|---|---|---|---|
| `Created` | ≈ `None` | — | — |
| `DepositOpen` | ≈ `Initialized` | — | `is_active == true` |
| `CheckedIn` | — | NOT represented (lives on `Attendee`) | `AttendeeDeposit.checked_in` |
| `Refundable` | — | `refundable: bool` | computed from `event_end + refund_deadline` |
| `Claimable` | — | NOT represented off-chain | computed on-chain |
| `Closed` | `Closed` | — | account closed (ceases to exist) |

`Created → DepositOpen → Closed` is the **event-level escrow PDA** lifecycle.
`... → CheckedIn → Refundable → Claimable →` is the **per-attendee deposit**
lifecycle. Implementing the plan's single typed FSM would require either (a)
merging the two surfaces (a large redesign crossing the off-chain/on-chain
boundary), or (b) introducing a typed state enum on
`DepositStatus`/`ThbDeposit` that currently has none.

### The monetary-correctness risk is already covered (in three layers)

The plan's motivation — "the one place state-machine correctness has monetary
consequences" — correctly identifies the risk surface. But the **enforcement
already exists**:

1. **On-chain Anchor constraints** (authoritative). Each instruction checks
   preconditions: `deactivate_event` requires `is_active == true`;
   `claim_forfeited` requires `now > refund_deadline && !checked_in`;
   `refund` requires the deposit account exists and isn't already refunded.
   Cannot be bypassed — enforced by the Solana runtime.
2. **Worker runtime transition allowlist** (`worker/src/event_store/write.rs:502-506`,
   mirrored at `:768-772`):
   ```rust
   let valid = matches!(
       (&config.escrow_status, v),
       (EscrowStatus::None, EscrowStatus::Initialized)
           | (EscrowStatus::Initialized, EscrowStatus::Deactivated)
           | (EscrowStatus::Deactivated, EscrowStatus::Closed)
           | (EscrowStatus::Closed, EscrowStatus::None)
           | (EscrowStatus::Cancelled, EscrowStatus::None)
   );
   ```
   Catches illegal transitions before they reach the on-chain program, with
   the typed error `invalid escrow status transition: {source} → {target}`.
3. **Drift detection** (`worker/src/handlers/deposit/escrow/status.rs:373-383`,
   `escrow_health_handler`). Compares off-chain `EscrowStatus` to on-chain
   account existence and flags DRIFT explicitly.

The risk is **not missing enforcement**. The risk is **drift between the three
layers** — if someone adds a new `EscrowStatus` variant without updating the
runtime allowlist, the allowlist silently rejects the new transition. A
compile-time type-state FSM cannot solve this drift because type-state is
intra-crate; the authoritative state machine lives on-chain in Anchor/Quasar,
not in Rust type-state, and the worker writes `EventConfig.escrow_status` via
serde deserialization (which cannot preserve type-state across the
domain/worker crate boundary).

## 2. Changes (1 new doc, 2 doc edits)

### New: `.plans/014_phase2_4_typestate_audit.md` (480 lines)

The full audit findings. Structure mirrors `.plans/014_ssot_audit.md`
(handover 113):

- **Executive Summary** — the headline finding (three surfaces, not one FSM)
  and the recommendation (do NOT implement the plan as written).
- **The 8th Consecutive Audit Miss** — full table positioning Phase 2.4 in
  the audit-miss pattern.
- **The Plan's Premise** — the four load-bearing claims the plan makes, each
  examined against the codebase.
- **Reality: Three Separate State Surfaces** — detailed evidence for each
  surface (event-level enum, per-attendee booleans, on-chain booleans),
  with file:line citations.
- **Why the Plan's 6-State List Doesn't Map** — the mapping table showing
  the plan's states come from two different lifecycles.
- **The Monetary-Correctness Risk Is Already Covered** — the three
  enforcement layers, with the drift analysis.
- **Recommendations R1-R4** — R1 (contract test, highest priority),
  R2 (this doc as architecture note), R3 (do NOT type-state), R4 (NOT
  recommended: typed enum on DepositStatus, merging surfaces).
- **What This Audit Deliberately Refuses to Claim** — out-of-scope items
  (on-chain program security audit, race conditions, cross-event
  invariants).
- **Audit Method (reproducibility)** — every `rg` / file-read with
  file:line, so a future agent can reproduce the findings.
- **Conclusion** — Phase 2.4 concluded as negative result; R1 is the
  positive follow-up.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md` (+36/-1)

Phase 2.4 task entry checkbox changed from `[ ]` to `[x]` DONE with an inline
summary of the audit outcome. The summary covers:
- The three state surfaces (event-level enum, per-attendee booleans,
  on-chain booleans) with file:line.
- The three enforcement layers (Anchor constraints, worker allowlist,
  drift handler) with file:line.
- Why type-state is the wrong tool (cross-crate/cross-boundary reach).
- The three recommendations (R1 contract test, R2 doc, R3 do-not-implement).
- Pointers to the full audit and the negative-results entry.

### Edited: `.plans/014_negative_results.md` (+79)

Entry #10 appended: "Type-state escrow lifecycle FSM (Plan 014 Phase 2.4)".
Follows the established entry structure (Status, Original framing, Why demoted
with three layers, Proof with file:line, Preconditions that would re-open).
The reference table at the end of the file was updated with row #10:

| 10. Type-state escrow lifecycle FSM | Demote | Plan premise structurally wrong — 6 states don't map to any single typed surface (event-level enum + per-attendee booleans + on-chain booleans); runtime transition allowlist already exists; type-state can't reach cross-layer/cross-boundary drift (8th miss) |

### NOT changed (deliberately)

- **No `.rs` files touched.** This is a read-only audit. Confirmed by
  `git diff --stat` showing only markdown files.
- **No frontend changes.** The frontend mirror of `EscrowStatus` is
  documented in handover 114 (Phase 2.2 R1); out of scope here.
- **No on-chain program changes.** The audit read `deactivate_event.rs` to
  confirm the constraint pattern but did not modify any instruction.
- **No worker transition-allowlist changes.** The runtime guard at
  `worker/src/event_store/write.rs:502-506` is the **subject** of the
  audit's R1 recommendation; changing it would preempt the recommendation.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Branch isolation | `git status -sb` on `feature/014_phase2_4_audit` | Only `.plans/` files in diff |
| Diffstat before commit | `git diff --stat` + `git status --short` | 2 modified + 1 untracked, all markdown |
| No Rust touched | `git diff -- '*.rs'` | Empty |
| Workspace tests still pass | `cargo test --workspace` | ✅ 311 passing, 0 failed (unchanged from handover 115 — docs-only session) |
| Plan's premise re-verified | `rg "pub enum EscrowStatus"` → `domain/src/models/event.rs:119-129` | ✅ 5 states, doc comment names `None → Initialized → Deactivated → Closed` |
| Runtime allowlist re-verified | `rg "EscrowStatus::(Deactivated\|Closed\|Cancelled)"` in worker | ✅ `worker/src/event_store/write.rs:502-506` + `:768-772` |
| On-chain state model re-verified | Read `bethere-escrow/src/state.rs` | ✅ `is_active: bool`, `checked_in: bool`, `refunded: bool` — no status enum |
| On-chain constraint re-verified | Read `bethere-escrow/src/instructions/deactivate_event.rs:20-26` | ✅ Anchor `constraints(event_escrow.is_active())` |

### Test count unchanged

Workspace: **311** (was 311 in handover 115). Frontend: not re-run (no
frontend files touched). This session is pure documentation; the test
count is identical by construction.

## 4. Plan / Code / Test Locations

- **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.4 (now marked
  `[x]` DONE with audit outcome inline).
- **Audit findings**: `.plans/014_phase2_4_typestate_audit.md` (480 lines, new).
- **Negative-results entry**: `.plans/014_negative_results.md` entry #10 (new).
- **Three state surfaces (audit subjects)**:
  - Event-level enum: `domain/src/models/event.rs:117-147` (`EscrowStatus`,
    `is_active()`).
  - Per-attendee deposit: `domain/src/models/deposit.rs:57-89`
    (`DepositStatus`); `:108-134` (`ThbDeposit`).
  - On-chain program: `bethere-escrow/src/state.rs` (`EventEscrow.is_active`,
    `AttendeeDeposit.checked_in/refunded`).
- **Three enforcement layers (audit subjects)**:
  - On-chain constraints: `bethere-escrow/src/instructions/*.rs` (e.g.
    `deactivate_event.rs:20-26`).
  - Worker runtime allowlist: `worker/src/event_store/write.rs:502-506`
    (mirrored at `:768-772`).
  - Drift detection: `worker/src/handlers/deposit/escrow/status.rs:373-383`.
- **R1 follow-up target** (not implemented): a worker-level integration test
  pinning the 5 legal × 20 illegal transitions on the runtime allowlist.

## 5. Reflections

### What went well

- **The audit-first discipline caught another structural miss — the 8th in a row.**
  Had I shipped the plan's type-state FSM literally, I would have either
  (a) built a typed enum that doesn't match the on-chain program's booleans
  (creating new drift), or (b) introduced a typed `DepositLifecycle` enum on
  `DepositStatus`/`ThbDeposit` where none exists today (a major refactor with
  no incremental safety). The audit prevented both.
- **The three-surfaces framing is the lasting artifact.** Future contributors
  reading the Phase 2.4 plan entry will see the `[x]` DONE marker with the
  inline summary explaining *why* it was reframed. The full audit doc has
  the file:line evidence. The negative-results entry is the durable
  "don't re-propose this" record.
- **The recommendation (R1) is small and bounded.** Rather than asking for a
  large redesign, the audit points at a concrete, low-risk follow-up: a
  contract test pinning the runtime allowlist. Same pattern as the Phase 2.3
  SSOT guard (handover 112) and the Phase 5.3 deterministic guard
  (handover 111). The discipline scales.
- **Reusing the audit-doc structure from handover 113.** The Phase 2.1 SSOT
  audit and this Phase 2.4 audit have the same shape: Executive Summary,
  Audit Miss table, Premise, Reality, Recommendations, Refuses to Claim,
  Method, Conclusion. Consistency makes the audit docs readable as a series.

### What was harder than expected

- **Resisting the urge to "fix" the per-attendee boolean flags.** The audit
  found that `DepositStatus` uses `verified`/`refundable`/`rejected` booleans
  where a typed enum might read more cleanly. It would have been easy to
  scope-creep into proposing a `DepositLifecycle` enum. R4 in the audit
  explicitly recommends against this — the current representation is
  composable, the on-chain program uses booleans too, and the divergence
  risk of a new typed enum outweighs the readability gain.
- **Verifying the on-chain constraint pattern.** The audit read
  `deactivate_event.rs` to confirm `constraints(event_escrow.is_active())`,
  but did not exhaustively audit all 11 instruction files. The audit doc
  explicitly lists this as out-of-scope (§"What This Audit Deliberately
  Refuses to Claim" item 1) — an on-chain security audit is the right
  scope for that question, not a Phase 2.4 type-state audit.
- **Deciding whether to mark Phase 2.4 `[x]` or `[~]`.** The plan was
  *demoted*, not *implemented as written*. But the plan *was* concluded —
  the audit produced a definitive answer (do NOT implement; ship R1
  follow-up). Marking `[x]` DONE with the audit outcome inline is consistent
  with how Phase 2.3 was marked (also a corrected-scope conclusion, handover
  112) and Phase 3 (also demoted, handover 109). The `[x]` reflects "this
  phase produced a durable conclusion," not "the original plan shipped
  verbatim."

### Where the result differs from the plan

Phase 2.4 as written imagined a compile-time type-state FSM as "the legitimate
sibling of katgpt-rs's `ConstraintPruner` trait." The audit found that the
analogy doesn't hold: katgpt-rs's `ConstraintPruner` is intra-crate, operating
on in-memory types; the BeThere escrow lifecycle crosses the off-chain/on-chain
boundary, where the authoritative state machine lives in Anchor/Quasar
constraints that Rust type-state cannot reach. The legitimate sibling of
`ConstraintPruner` for this codebase is the **runtime transition allowlist +
contract test** pattern already in use (Phase 2.3 guard, Phase 5.3 guard).

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): ✅ CONCLUDED (handover 113)
- **Phase 2.2** (move predicates): 🟡 PARTIAL — R1 done (handover 114),
  R2 done (handover 115), only R3 open (substantive mirror-type merge
  decision, dedicated session)
- **Phase 2.3** (CI dup-check): ✅ CONCLUDED (handover 112); scope gap
  closed by R1 in handover 114
- **Phase 2.4** (type-state escrow): ✅ **CONCLUDED (this handover)** —
  plan demoted; 8th audit miss; R1 follow-up tracked separately
- **Phase 2.5** (no standalone WASM logic): 🟡 Open — documentation-only
  task ("Document this decision in `.plans/014_no_transformer_vm.md`").
  Mostly satisfied by negative-results entry #1.
- **Phase 3** (policy traits): ✅ CONCLUDED
- **Phase 4.1** (profile): 🟡 Blocked on infra (200-attendee staged event)
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic guard): ✅ CONCLUDED
- **Phase 5.4** (zero-alloc audit): ✅ CONCLUDED
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied)

### Phase 2.4 positive follow-up (R1)

- [ ] **R1** — Contract test pinning the worker's runtime transition allowlist.
  Constructs an `EventConfig` in each of the 5 `EscrowStatus` variants; for
  each (source, target) pair in the 5×5 cartesian product (25 cases),
  attempts the transition via `update_event`; asserts the 5 legal transitions
  succeed and the 20 illegal transitions produce the exact error format
  `invalid escrow status transition: {source} → {target}`. Lives in
  `worker/tests/` (or `worker/src/event_store/write.rs` `#[cfg(test)]`
  module). Same discipline as the Phase 2.3 SSOT guard and the Phase 5.3
  deterministic guard. **Not implemented in this handover** — would be a
  code change, and this was an audit-only session.

### Other open Plan 014 work

- **Phase 2.2 R3** — substantive mirror-type merge decision (EventFormat /
  EscrowStatus / etc.). Design decision; dedicated session.
- **Phase 2.5** — documentation-only ("no standalone WASM logic"). Mostly
  satisfied by negative-results entry #1; could be marked `[x]` with an
  inline pointer in a future docs-only session.
- **Phase 4.1** — profile a staged 200-attendee event end-to-end. **Blocked
  on infrastructure coordination.**
- **Phase 4.4** — document the "no SIMD" decision. **Blocked on 4.1.**
- **Phase 5.5** — feature-flag discipline documentation. Mostly satisfied.

### Immediate operator actions

- [ ] **Push** `develop` to `origin` — `git push origin develop`. After this
  handover's commits land, `develop` will be **8 commits ahead of
  `origin/develop`** (handover 113's 2 + handover 114's 2 + handover 115's 2
  + this handover's commits).
- [ ] **Deploy** the commits now sitting on `develop` to dev/prod via
  `develop → main → deploy.sh`. **Requires explicit operator confirmation.**
  No schema changes; rollback is `wrangler rollback`. The deploy is now
  **21 commits behind main** (15 from prior handovers + 6 from handovers
  114-116).

### Housekeeping

- [ ] **Delete feature branch** — `git branch -d feature/014_phase2_4_audit`
  (after merge to develop).
- [ ] **Optional cleanup** — four prior feature branches are all safe to
  delete (all merged via fast-forward): `feature/014_ssot_audit` (113),
  `feature/014_phase2_2_guard_widen` (114),
  `feature/014_phase2_2_r2_deposit_from_str` (115), and this handover's
  `feature/014_phase2_4_audit`.

### Explicitly deferred (do NOT do without confirmation)

- ❌ **Phase 2.4 R1 implementation** — the contract test is a code change;
  this was an audit-only session. Do not start without explicit go-ahead.
- ❌ **Phase 2.4 R3 (type-state FSM as written)** — explicitly recommended
  AGAINST by the audit. Do not implement.
- ❌ **Phase 2.2 R3** — substantive mirror-type merge decision; do not start
  without a dedicated session.
- ❌ **Deploy** — do not deploy without explicit operator confirmation.
- ❌ **Phase 4.1 / 4.4** — blocked on infrastructure coordination.

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 2.4 now `[x]`)
- Phase 2.4 audit findings: `.plans/014_phase2_4_typestate_audit.md` (new)
- Negative-results log: `.plans/014_negative_results.md` (entry #10 new)
- Phase 2.1 SSOT audit handover: `.handovers/113_plan_014_phase2_1_ssot_audit.md`
- Phase 2.2 R1 guard scope fix handover: `.handovers/114_plan_014_phase2_2_r1_guard_widen.md`
- Phase 2.2 R2 worker DepositMethod SSOT handover: `.handovers/115_plan_014_phase2_2_r2_deposit_from_str.md`
- Phase 2.3 SSOT mirror guard handover: `.handovers/112_plan_014_phase2_3_ssot_mirror_audit.md`
- Three state surfaces:
  - Event-level enum: `domain/src/models/event.rs:117-147`
  - Per-attendee deposit: `domain/src/models/deposit.rs:57-134`
  - On-chain program: `bethere-escrow/src/state.rs`
- Three enforcement layers:
  - On-chain constraints: `bethere-escrow/src/instructions/*.rs`
  - Worker allowlist: `worker/src/event_store/write.rs:502-506` + `:768-772`
  - Drift detection: `worker/src/handlers/deposit/escrow/status.rs:373-383`

## 8. How to Dev / Test

### Read the audit

```sh
# The full findings document (480 lines, file:line evidence throughout):
bat .plans/014_phase2_4_typestate_audit.md

# The plan entry with inline audit outcome:
bat .plans/014_katgpt_rs_paradigm_migration.md  # Phase 2.4 section

# The negative-results entry:
bat .plans/014_negative_results.md  # entry #10
```

### Reproduce the headline finding (three state surfaces)

```sh
cd /Users/ozone/event-checkin

# Surface 1: event-level EscrowStatus enum (5 states, not 6)
rg "pub enum EscrowStatus" domain/src/models/event.rs
# → L119-129; doc comment at L117-118 names "None → Initialized → Deactivated → Closed"

# Surface 2: per-attendee DepositStatus (no typed state, just booleans)
rg "pub struct DepositStatus" domain/src/models/deposit.rs
# → L57-67; verify verified/refundable/rejected are bools

# Surface 3: on-chain program (booleans, no status enum)
bat bethere-escrow/src/state.rs
# → EventEscrow.is_active: bool; AttendeeDeposit.checked_in: bool, refunded: bool
```

### Reproduce the runtime transition allowlist finding

```sh
cd /Users/ozone/event-checkin

# The 5-arm runtime allowlist (mirrored in two places):
rg "EscrowStatus::None, EscrowStatus::Initialized" worker/src/event_store/write.rs
# → L502-506 (update_event) and L768-772 (apply_update)

# The typed error format:
rg "invalid escrow status transition" worker/
# → L510-513

# The drift-detection health handler:
rg "DRIFT: server says" worker/src/handlers/deposit/escrow/status.rs
# → L373-383
```

### Reproduce the on-chain constraint finding

```sh
cd /Users/ozone/event-checkin

# deactivate_event requires is_active (Anchor constraint):
bat bethere-escrow/src/instructions/deactivate_event.rs
# → L20-26: constraints(event_escrow.is_active()) @ EscrowError::EventNotActive

# List all 11 instruction files (each enforces its own precondition):
eza bethere-escrow/src/instructions/
```

### Confirm no code changed

```sh
cd /Users/ozone/event-checkin
git --no-pager status --short
# Expect: only .plans/*.md files modified/untracked

git --no-pager diff -- '*.rs'
# Expect: empty
```

### Confirm tests still pass (docs-only sanity check)

```sh
cd /Users/ozone/event-checkin
cargo test --workspace
# Expect: 311 passing, 0 failed (unchanged from handover 115)
```

### Relationship to R1 (positive follow-up)

R1 is a worker-level integration test that pins the runtime transition
allowlist. It is **not** implemented in this handover (this was an audit-only
session). To implement R1 in a future session:

1. Create `worker/tests/escrow_transition_contract.rs` (or add to
   `worker/src/event_store/write.rs`'s `#[cfg(test)]` module).
2. For each of the 5×5 = 25 (source, target) pairs on `EscrowStatus`:
   - Construct an `EventConfig` with `escrow_status = source`.
   - Build an `UpdateEventRequest` with `escrow_status = Some(target)`.
   - Call `update_event` (or `apply_update`).
   - Assert: the 5 legal transitions succeed; the 20 illegal transitions
     produce `Err("invalid escrow status transition: {source} → {target}")`
     with the exact format.
3. If someone adds a 6th variant or changes the allowlist, this test forces
   them to update both in the same diff. Same discipline as the Phase 2.3
   SSOT guard (handover 112 §2 live-injection) and the Phase 5.3
   deterministic guard (handover 111).

### Relationship to R3 (do NOT implement)

R3 is the explicit recommendation against implementing the plan's type-state
FSM as written. The audit's reasoning (three layers) is in
`.plans/014_phase2_4_typestate_audit.md` §"Recommendations for Phase 2.4" R3.
The short version: type-state is intra-crate; the authoritative state machine
lives on-chain in Anchor/Quasar; the worker writes via serde deserialization
that cannot preserve type-state. The existing runtime allowlist + on-chain
constraints already cover the monetary-correctness invariant; the genuine
risk is cross-layer drift, which type-state cannot solve.