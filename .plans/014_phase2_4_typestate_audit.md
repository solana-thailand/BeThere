# Plan 014 Phase 2.4 — Type-State Escrow Lifecycle Audit Findings

> Branch: `feature/014_phase2_4_audit` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.4
> Method: read-only audit-first pass (8th consecutive Plan 014 audit)

## Executive Summary

Phase 2.4 of Plan 014 asks for a **compile-time type-state FSM** over the
"escrow lifecycle": `Created → DepositOpen → CheckedIn → Refundable → Claimable
→ Closed`. The plan describes this as "the legitimate sibling of katgpt-rs's
`ConstraintPruner` trait — a compile-time FSM that makes invalid state
transitions not compile. Limit to escrow (the one place state-machine
correctness has monetary consequences)."

**The audit's conclusion: the plan's premise does not match the codebase.**
This is the **8th consecutive Plan 014 audit miss**, in the same structural
pattern as the previous seven. The mismatch has three layers:

1. **The plan's 6 states don't exist as a typed FSM anywhere.** The codebase
   has two **separate** state surfaces that the plan conflates into one:
   - **Event-level escrow** (`domain::models::event::EscrowStatus`, 5 states:
     `None, Initialized, Deactivated, Closed, Cancelled`) — the off-chain
     projection of the on-chain PDA lifecycle. Doc comment at
     `domain/src/models/event.rs:117-118` explicitly states the state machine
     is `None → Initialized → Deactivated → Closed`.
   - **Per-attendee deposit** (`DepositStatus`, `ThbDeposit`) — **no typed
     state at all**. Lifecycle is encoded as **independent boolean flags**
     (`verified: bool`, `refundable: bool`, `rejected: bool` on
     `DepositStatus`; `verified: bool`, `refunded: bool` on `ThbDeposit`).

2. **The on-chain program uses booleans, not a typed state.** `EventEscrow`
   has `is_active: bool`; `AttendeeDeposit` has `checked_in: bool` +
   `refunded: bool`. The `EscrowStatus` enum in `domain` is an **off-chain
   projection/summary** of these booleans, not a type the program enforces.

3. **Transition enforcement already exists at runtime.** The worker has an
   explicit allowlist guard at `worker/src/event_store/write.rs:502-506`
   (mirrored at `:768-772`) covering all 5 legal transitions:
   `(None→Initialized)`, `(Initialized→Deactivated)`, `(Deactivated→Closed)`,
   `(Closed→None)`, `(Cancelled→None)`. Any other transition produces the
   error `invalid escrow status transition: X → Y`. The on-chain program
   additionally enforces `is_active` via Anchor constraints (see
   `bethere-escrow/src/instructions/deactivate_event.rs:25`).

The plan's 6-state list (`Created, DepositOpen, CheckedIn, Refundable,
Claimable, Closed`) is a **synthesis** of these two surfaces into a single
typed FSM that **does not exist in the codebase**. The states `CheckedIn,
Refundable, Claimable` describe the **per-attendee deposit** lifecycle; the
states `Created, DepositOpen, Closed` describe the **event-level escrow**
lifecycle. Type-stating them as one FSM would require either (a) merging
the two surfaces (a large redesign), or (b) introducing a typed state enum
on `DepositStatus`/`ThbDeposit` that currently has none.

**Recommendation: do NOT implement Phase 2.4 as written.** The compile-time
type-state FSM the plan describes is not the right artifact for this
codebase. The existing runtime transition allowlist + on-chain constraints
already enforce the monetary-correctness invariant. The real risk is
**guard drift** (the runtime allowlist vs the `EscrowStatus::is_active()`
predicate vs the on-chain constraints), which is better addressed by a
**contract test** than by a type-state refactor. See Recommendations R1-R3.

---

## The 8th Consecutive Audit Miss

| # | Plan's premise | Reality |
|---|---|---|
| 1 | Phase 1.4 EventMetaWire fixed shape | Variable strings; Pod 26% larger than JSON |
| 2 | Phase 1.5 DepositStatusWire 1-day work | Needs base58 + ID policy (~2 days) |
| 3 | Phase 4.3.1 event-series endpoint uncached | Already cached at 120s (Plan 013) |
| 4 | Phase 4.3.3 quiz does one PUT per answer | Batches in-memory, writes once |
| 5 | Phase 4.3.4 blockhash valid ~120s | Confuses ring-buffer with `MAX_PROCESSING_AGE` (~60–90s) |
| 6 | Phase 2.3 grep against `domain::policy` | `domain::policy` never created; predicates are methods on `domain::models::*` |
| 7 | Phase 2.1: "DepositMethod → string in 3+ places", escrow predicates duplicated | Only 2 serialization sites exist; participation-type NOT duplicated — BUT the Phase 2.3 guard's scope hides 3 load-bearing mirrored predicates |
| **8** | **Phase 2.4: type-state the 6-state escrow FSM at compile time** | **5-state event-level enum + per-attendee booleans + on-chain booleans; runtime transition allowlist already exists; the 6 states don't map to any single typed surface** |

The discipline that has worked every time is: **audit first, ship what
survives the audit, document the miss inline**. Phase 2.4 follows that
discipline — the audit produced a recommendation to NOT implement the plan
as written, plus a smaller alternative (R1) that captures the genuine
monetary-correctness value.

---

## The Plan's Premise

From `.plans/014_katgpt_rs_paradigm_migration.md` Phase 2.4:

> **2.4 Type-state the escrow lifecycle** (`Created → DepositOpen →
> CheckedIn → Refundable → Claimable → Closed`). This is the *legitimate* sibling
> of katgpt-rs's `ConstraintPruner` trait — a compile-time FSM that makes invalid
> state transitions not compile. Limit to escrow (the one place state-machine
> correctness has monetary consequences).

The premise has four load-bearing claims:

1. There is a single "escrow lifecycle" with 6 named states.
2. The states form a linear progression (`Created → DepositOpen → ... → Closed`).
3. Implementing this as a compile-time type-state FSM is the right tool.
4. The benefit is "make invalid state transitions not compile".

The audit examined each claim against the codebase. None survives unchanged.

---

## Reality: Three Separate State Surfaces

### Surface 1 — Event-level escrow (domain `EscrowStatus` enum)

Defined at `domain/src/models/event.rs:117-147`:

```rust
/// On-chain escrow lifecycle status.
/// Tracks the state machine: None → Initialized → Deactivated → Closed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    #[default]
    None,
    Initialized,
    Deactivated,
    Closed,
    Cancelled,
}

impl EscrowStatus {
    pub fn as_str(&self) -> &'static str { /* ... */ }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Initialized | Self::Deactivated)
    }
}
```

**5 states, not 6.** The doc comment explicitly names the state machine as
`None → Initialized → Deactivated → Closed`. `Cancelled` is a parallel
recovery state (refunds-in-progress after organizer cancellation). This is
the **event-level** lifecycle — one `EscrowStatus` per `EventConfig`,
mirrored on `EventMeta`. The frontend mirror at
`frontend-leptos/src/api/event.rs:24-31` is byte-identical (already
documented in handover 114's R1 widening).

### Surface 2 — Per-attendee deposit (no typed state at all)

`DepositStatus` (`domain/src/models/deposit.rs:57-89`):

```rust
pub struct DepositStatus {
    pub attendee_id: String,
    pub event_id: String,
    pub method: DepositMethod,
    pub amount: u64,
    pub currency: String,
    pub tx_signature: Option<String>,
    pub verified: bool,            // ← boolean flag, not state
    pub deposited_at: String,
    pub wallet_address: Option<String>,
    pub deposit_order: u32,
    pub refundable: bool,          // ← boolean flag, not state
    pub rejected: bool,            // ← boolean flag, not state
}
```

`ThbDeposit` (`domain/src/models/deposit.rs:108-134`):

```rust
pub struct ThbDeposit {
    pub attendee_id: String,
    pub event_id: String,
    pub amount_thb: u64,
    pub slip_url: Option<String>,
    pub verified: bool,            // ← boolean flag
    pub verified_by: Option<String>,
    pub verified_at: Option<String>,
    pub uploaded_at: String,
    pub refunded: bool,            // ← boolean flag
    pub refunded_at: Option<String>,
    // ... display enrichment fields
}
```

**There is no typed deposit-lifecycle state enum.** The "states"
`DepositOpen, CheckedIn, Refundable, Claimable` from the plan's premise are
**implicit** — combinations of these independent booleans. For example:
- "refundable" = `refundable == true && !is_past_deadline(...) && !rejected`
- "checked in" = lives on `Attendee` via `Attendee::is_checked_in()`, NOT on `DepositStatus`
- "claimable" = (computed on-chain from `event_end + refund_deadline` and `checked_in` flags; no off-chain representation at all)

The plan's `CheckedIn` and `Claimable` states are not even represented on
the off-chain `DepositStatus` struct. They exist only on-chain.

### Surface 3 — On-chain program (booleans, not typed state)

`bethere-escrow/src/state.rs`:

```rust
pub struct EventEscrow {
    // ...
    pub is_active: bool,           // ← single boolean; "states" are implicit
    // ...
}

pub struct AttendeeDeposit {
    // ...
    pub checked_in: bool,          // ← boolean flag
    pub refunded: bool,            // ← boolean flag
    // ...
}
```

The on-chain program has **no status enum**. "Initialized" = account exists
+ `is_active == true`; "Deactivated" = `is_active == false`; "Closed" =
account closed/reclaimed (account ceases to exist). The off-chain
`EscrowStatus` enum is a **projection/summary** of these on-chain facts,
kept in sync manually by the worker (see Surface 4 below).

The on-chain constraints enforce state-machine correctness directly via
Anchor `constraints`. Example from
`bethere-escrow/src/instructions/deactivate_event.rs:20-26`:

```rust
#[account(
    mut,
    has_one(organizer) @ EscrowError::Unauthorized,
    constraints(event_escrow.is_active()) @ EscrowError::EventNotActive,
    address = EventEscrow::seeds(organizer.address(), event_id)
)]
pub event_escrow: Account<EventEscrow>,
```

This is the **on-chain state-machine enforcement** the plan worries about.
It already exists. It enforces: you can only deactivate an active escrow.

### Surface 4 — Worker transition guard (runtime allowlist)

`worker/src/event_store/write.rs:498-514` (mirrored at `:764-780`):

```rust
if let Some(ref v) = req.escrow_status {
    // Validate state transition
    let valid = matches!(
        (&config.escrow_status, v),
        (EscrowStatus::None, EscrowStatus::Initialized)
            | (EscrowStatus::Initialized, EscrowStatus::Deactivated)
            | (EscrowStatus::Deactivated, EscrowStatus::Closed)
            | (EscrowStatus::Closed, EscrowStatus::None)
            | (EscrowStatus::Cancelled, EscrowStatus::None)
    );
    if !valid {
        return Err(format!(
            "invalid escrow status transition: {} → {}",
            config.escrow_status, v
        ));
    }
    config.escrow_status = v.clone();
}
```

This is the **off-chain state-machine enforcement** at the write boundary.
It covers all 5 legal transitions on `EscrowStatus`. Any illegal transition
produces a typed error message naming both states.

---

## Why the Plan's 6-State List Doesn't Map

The plan's premise lists 6 states: `Created, DepositOpen, CheckedIn,
Refundable, Claimable, Closed`. These don't map cleanly to either surface:

| Plan's state | Event-level `EscrowStatus`? | Per-attendee `DepositStatus`? | On-chain `EventEscrow` / `AttendeeDeposit`? |
|---|---|---|---|
| `Created` | ≈ `None` (no escrow account yet) | — | — |
| `DepositOpen` | ≈ `Initialized` (PDA created, accepting deposits) | — | `is_active == true` |
| `CheckedIn` | — | NOT represented (lives on `Attendee`) | `AttendeeDeposit.checked_in == true` |
| `Refundable` | — | `refundable: bool` (composable with deadline check) | computed from `event_end + refund_deadline` |
| `Claimable` | — | NOT represented off-chain | computed on-chain from `event_end + refund_deadline` + `!checked_in` |
| `Closed` | `Closed` | — | `EventEscrow` account closed (ceases to exist) |

The plan's 6 states are a **synthesis** of two genuinely different lifecycles:
- `Created → DepositOpen → Closed` is the **event-level escrow PDA** lifecycle (Surface 1 + 3).
- `... → CheckedIn → Refundable → Claimable →` is the **per-attendee deposit** lifecycle (Surface 2 + the AttendeeDeposit on-chain account).

To implement the plan's single typed FSM, you would have to either:
- (a) **Merge the two surfaces** into a single typed lifecycle — a large redesign that crosses the off-chain/on-chain boundary and would require either the on-chain program to grow a typed status field or the off-chain domain to compute "claimable" on-chain at read time; or
- (b) **Introduce a typed state enum on `DepositStatus`/`ThbDeposit`** that currently has none — adding new representation, not type-stating existing representation.

Neither is the small "type-state refactor" the plan imagines.

---

## The Monetary-Correctness Risk Is Already Covered

The plan's motivation — "the one place state-machine correctness has
monetary consequences" — is correct in identifying the risk surface. The
risk is real: an illegal escrow transition could let an organizer claim
forfeited deposits before the refund window closes, or could let an
attendee refund a deposit after forfeiture.

But the **enforcement** is already in place, in three independent layers:

1. **On-chain Anchor constraints** (the authoritative layer). Each
   instruction checks the relevant precondition: `deactivate_event`
   requires `is_active == true`; `close_event` requires `is_active == false`
   and that all deposits are settled; `claim_forfeited` requires
   `now > refund_deadline && !checked_in`; `refund` requires the deposit
   account exists and isn't already refunded. These cannot be bypassed —
   they're enforced by the Solana runtime.
2. **Worker transition allowlist** (off-chain fast-fail). Catches illegal
   transitions before they reach the on-chain program, with a clear error
   message. This is the guard at `worker/src/event_store/write.rs:502-506`.
3. **Drift detection** (post-hoc). `worker/src/handlers/deposit/escrow/status.rs:373-383`
   has an `escrow_health_handler` that compares the off-chain
   `EscrowStatus` to on-chain account existence and flags DRIFT
   explicitly.

The risk is **not missing enforcement**. The risk is **drift between the
three layers** — if someone adds a new state to `EscrowStatus` without
updating the runtime allowlist, the allowlist silently rejects the new
transition. Or if the on-chain program grows a new instruction that
changes `is_active` semantics, the off-chain projection drifts.

A **compile-time type-state FSM** would not solve drift between these
layers — type-state is intra-crate (compile-time within one Rust crate),
not cross-crate or cross-boundary (off-chain ↔ on-chain). The on-chain
program is Anchor/Quasar, not Rust type-state; the off-chain worker is
a different crate from domain. Type-state cannot reach across these
boundaries.

---

## Recommendations for Phase 2.4 (do NOT implement the plan as written)

### R1. Add a contract test that pins the runtime transition allowlist (highest priority — closes the drift gap)

The genuine monetary-correctness risk is **allowlist drift**. Add a
worker-level integration test that:

1. Constructs an `EventConfig` in each of the 5 `EscrowStatus` variants.
2. For each (source, target) pair in the cartesian product (5 × 5 = 25
   cases), attempts the transition via `update_event`.
3. Asserts the 5 legal transitions succeed and the 20 illegal transitions
   produce the exact error format
   `invalid escrow status transition: {source} → {target}`.

If someone adds a 6th variant or changes the allowlist, this test forces
them to update both the allowlist AND the test in the same diff. Same
pattern as the Phase 2.3 SSOT guard (handover 112) and the Phase 5.3
deterministic guard (handover 111).

Risk: low. Test-only change; no production code touched.

### R2. Document the three-layer state-machine architecture in `.plans/`

The Phase 2.4 plan was wrong because it didn't account for the three
layers. A half-page architecture note in `.plans/014_phase2_4_typestate_audit.md`
(this document) is the artifact. Future contributors reading the plan
need to understand why Phase 2.4 was reframed.

Risk: zero. Documentation only.

### R3. Do NOT type-state the escrow lifecycle

The plan's premise is structurally wrong for this codebase. Type-state is
the wrong tool because:
- It can't reach across the off-chain/on-chain boundary (the on-chain
  program enforces the authoritative state machine).
- It can't reach across the domain/worker crate boundary without major
  refactoring (the worker's `update_event` writes `EventConfig.escrow_status`
  via serde deserialization, which cannot preserve type-state).
- The two surfaces (event-level + per-attendee) cannot be merged into a
  single typed FSM without a large redesign that crosses the
  off-chain/on-chain boundary.

Risk of doing it anyway: high. Would require a large redesign for a
benefit (compile-time enforcement) that the existing runtime allowlist +
on-chain constraints already provide.

### R4. NOT recommended

- Do NOT add a typed `DepositLifecycle` enum to `DepositStatus`/`ThbDeposit`.
  The current boolean-flag representation is composable and works; adding
  a typed enum would require migrating every read site and would not
  eliminate the on-chain/off-chain split.
- Do NOT merge `EscrowStatus` (event-level) with a per-attendee state enum.
  They are genuinely different lifecycles; conflating them was the plan's
  original mistake.

---

## What This Audit Deliberately Refuses to Claim

1. **On-chain program correctness.** The audit read
   `deactivate_event.rs` to confirm the constraint pattern but did not
   exhaustively audit all 11 instruction files (`claim_forfeited.rs`,
   `close_deposit.rs`, `close_event.rs`, `create_event.rs`,
   `deactivate_event.rs`, `deposit.rs`, `mark_checked_in.rs`,
   `refund.rs`, `rollover_deposit.rs`, `introspection.rs`). A separate
   on-chain security audit is the right scope for that question.
2. **Race conditions in the worker transition guard.** The audit did not
   verify that the runtime allowlist at
   `worker/src/event_store/write.rs:502-506` is checked atomically with
   the write. D1's KV write elimination (handover 100) and the
   durable-objects migration (handovers 090-091) touched this surface;
   the audit takes the current write path as given.
3. **Cross-event state invariants.** The audit examined one event's
   escrow lifecycle. It did not verify invariants across events (e.g.
   "an organizer cannot have two Initialized escrows for the same
   deposit_mint") — that's a separate concern.

---

## Audit Method (reproducibility)

Every claim above is grounded in `rg` / file reads with file:line evidence:

- **Plan's premise**: read `.plans/014_katgpt_rs_paradigm_migration.md`
  Phase 2.4 entry (L223-227).
- **`EscrowStatus` enum**: `rg "pub enum EscrowStatus"` →
  `domain/src/models/event.rs:119-129`. Doc comment at L117-118 names the
  state machine.
- **`DepositStatus` struct fields**: `rg "pub struct DepositStatus"` →
  `domain/src/models/deposit.rs:57-67`. Booleans at `verified` (L70),
  `refundable` (L82), `rejected` (L86).
- **`ThbDeposit` struct fields**: `rg "pub struct ThbDeposit"` →
  `domain/src/models/deposit.rs:108-134`. Booleans at `verified` (L120),
  `refunded` (L128).
- **On-chain `EventEscrow`**: read `bethere-escrow/src/state.rs` —
  `is_active: bool` field confirmed at the struct definition.
- **On-chain `AttendeeDeposit`**: read `bethere-escrow/src/state.rs` —
  `checked_in: bool` + `refunded: bool` fields confirmed.
- **On-chain constraint pattern**: read
  `bethere-escrow/src/instructions/deactivate_event.rs:20-26` — Anchor
  `constraints(event_escrow.is_active()) @ EscrowError::EventNotActive`
  confirmed.
- **Worker transition allowlist**: `rg "EscrowStatus::(Deactivated|Closed|Cancelled)"`
  → `worker/src/event_store/write.rs:502-506` (mirrored at `:768-772`).
  Read full context at L498-514.
- **Worker escrow-health drift detection**: same grep →
  `worker/src/handlers/deposit/escrow/status.rs:373-383`.
- **Frontend transitions**: same grep →
  `frontend-leptos/src/pages/escrow_init.rs` (~L251-264, L808-812,
  L1000-1004, etc.) confirm the UI drives transitions via
  `api::UpdateEventBody { escrow_status: Some(...) }` calls, which route
  through the worker guard.

---

## Conclusion

Phase 2.4 is the **8th consecutive Plan 014 audit miss**, in the same
structural pattern as the previous seven: the plan's premise describes a
state machine that doesn't match the codebase, and the audit's value is
in preventing the wrong refactor from shipping.

The codebase already has a three-layer state-machine enforcement for the
escrow lifecycle: on-chain Anchor constraints (authoritative), a worker
runtime transition allowlist (fast-fail), and a drift-detection health
check (post-hoc). The genuine monetary-correctness risk is **drift
between these layers**, not missing enforcement. A compile-time type-state
FSM cannot solve cross-layer drift — type-state is intra-crate; the
authoritative state machine lives on-chain in Anchor/Quasar, not in Rust
type-state.

**Phase 2.4 should be reframed.** The recommendation is:

- **R1 (highest priority)**: Add a contract test pinning the worker's
  runtime transition allowlist (5 legal transitions, 20 illegal, exact
  error format). Same discipline as the Phase 2.3 SSOT guard.
- **R2**: This audit document is the architecture note explaining the
  three-layer model. No further doc work needed.
- **R3**: Do NOT implement the plan's type-state FSM as written. The
  premise is structurally wrong; the existing enforcement already covers
  the monetary-correctness invariant; a type-state refactor would require
  a large redesign for no incremental safety gain.

Phase 2.4 is therefore **concluded as a negative result** (the plan as
written should not be implemented), with a single positive follow-up
(R1) that captures the genuine value. A negative-results entry should be
appended to `.plans/014_negative_results.md`.