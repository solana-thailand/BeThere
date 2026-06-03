# TDD + DDD Architecture for BeThere

> Status: **Draft — for discussion**
> Related: `.design/DESIGN.md`, `docs/d1_migration_architecture.md`

## Table of Contents

1. [Current State Analysis](#current-state-analysis)
2. [DDD: Where We Are vs Where We Should Be](#ddd-where-we-are-vs-where-we-should-be)
3. [Proposed Domain Model](#proposed-domain-model)
4. [Proposed Module Structure](#proposed-module-structure)
5. [Repository Pattern (Ports & Adapters)](#repository-pattern-ports--adapters)
6. [TDD: Testing Strategy](#tdd-testing-strategy)
7. [Migration Plan](#migration-plan)
8. [Decisions Needed](#decisions-needed)

---

## Current State Analysis

### What We Have

```
event-checkin/
├── domain/              # Shared types only — NO behavior
│   └── src/models/      # Event, Attendee, Deposit, Org (data structs + serde)
│       └── event.rs     # 889 lines of structs, 0 business rules
│       └── attendee.rs  # 632 lines, 20 unit tests for column mapping
│       └── deposit.rs   # Pure data types for dual-track payment
│
├── worker/              # ALL logic lives here
│   └── src/handlers/    # 20 handler files — HTTP + business rules + IO mixed
│   └── src/sheets/      # Google Sheets adapter
│   └── src/solana_escrow/  # On-chain escrow adapter
│   └── src/event_store/    # KV-backed event storage
│   └── src/claim/       # NFT claim logic
│   └── src/escrow_indexer/ # On-chain event indexing
│
└── tests/integration/   # 1 shell script
```

### The Problem: Anemic Domain Model

Our `domain` crate is just data structures. All business logic lives in `worker/src/`:

| Business Rule | Where It Lives Now | Should Live In |
|---|---|---|
| "Can this attendee check in?" | `handlers/checkin.rs` | `domain::Attendee` |
| "Is this deposit refundable?" | `handlers/deposit/` | `domain::Deposit` |
| "Has the refund deadline passed?" | Scattered across handlers | `domain::Event` |
| "Is escrow active for new deposits?" | `solana_escrow.rs` + handlers | `domain::Event` |
| "Is in-person capacity available?" | `handlers/register.rs` | `domain::Event` |
| "Is this attendee approved?" | Inline in handlers | `domain::Attendee` |

**Impact**: We can't test business rules without spinning up Cloudflare Workers runtime. Tests are slow, coupled to infrastructure, and hard to write.

### Test Coverage Map

| Layer | What's Tested | How | Speed |
|---|---|---|---|
| Domain types | Column mapping, `is_in_person()`, `display_name()` | `#[test]` inline in `attendee.rs` | <1ms |
| Serde contracts | Request/response serialization | `worker/tests/serde_contract.rs` | <1ms |
| On-chain program | Escrow instructions | SVM tests (47 unit + 38 on-chain) | ~100ms |
| Handlers/business logic | E2E via Playwright + devnet scripts | `e2e/`, `tests/integration/` | Seconds |
| **Gap: Domain business rules** | **Not tested in isolation** | — | — |

---

## DDD: Where We Are vs Where We Should Be

### Key DDD Concepts Applied to BeThere

| Concept | What It Means for Us |
|---|---|
| **Ubiquitous Language** | Event, Attendee, Deposit, Check-in, Claim, Refund — we already use consistent terms |
| **Bounded Context** | Two contexts: **Event Management** (CRUD, registration) and **Escrow/Deposit** (payments, refunds) |
| **Entities** | `Event`, `Attendee`, `Deposit` — have identity (IDs) and lifecycle |
| **Value Objects** | `DepositMethod`, `EventStatus`, `CheckInStatus`, `EscrowStatus` — already enums |
| **Aggregates** | `Event` is the aggregate root — attendees and deposits belong to an event |
| **Domain Services** | Refund eligibility, capacity gating — cross-entity rules |
| **Repositories** | Abstract data access behind traits — `EventRepo`, `AttendeeRepo` |

### What We DON'T Need

BeThere is a focused CRUD + escrow app. Avoid over-engineering:

- ❌ Domain Events / Event Sourcing
- ❌ CQRS (Command Query Responsibility Segregation)
- ❌ Complex Aggregates with invariants across entities
- ❌ Anti-Corruption Layers
- ❌ Separate Bounded Context codebases

**Keep it pragmatic**: Rich domain models + repository traits + domain services. That's it.

---

## Proposed Domain Model

### Aggregate Root: Event

```rust
// domain/src/models/event.rs — ADD behavior to existing structs

impl EventConfig {
    /// Is this event accepting new registrations?
    pub fn is_registration_open(&self, now_ms: i64) -> bool {
        self.status == EventStatus::Active
            && (self.event_start_ms == 0 || now_ms < self.event_start_ms)
    }

    /// Is the refund deadline still valid?
    pub fn is_refund_eligible(&self, now_ms: i64) -> bool {
        let deadline = self.event_end_ms
            + (self.refund_deadline_hours as i64 * 3600_000);
        now_ms <= deadline
    }

    /// Is in-person capacity still available?
    pub fn has_in_person_capacity(&self, current_count: u32) -> bool {
        self.in_person_capacity == 0 // 0 = unlimited
            || current_count < self.in_person_capacity
    }

    /// Is online capacity still available?
    pub fn has_online_capacity(&self, current_count: u32) -> bool {
        self.online_capacity == 0
            || current_count < self.online_capacity
    }

    /// Are USDC deposits accepted? Only when escrow is initialized.
    pub fn accepts_usdc_deposits(&self) -> bool {
        self.deposit_enabled && self.escrow_status == EscrowStatus::Initialized
    }

    /// Has the deposit deadline passed for a given registration date?
    pub fn deposit_deadline_passed(
        &self,
        registration_date_ms: i64,
        now_ms: i64,
    ) -> bool {
        match self.deposit_deadline_hours {
            Some(hours) => {
                let deadline = registration_date_ms
                    + (hours as i64 * 3600_000);
                now_ms > deadline
            }
            None => false,
        }
    }
}
```

### Entity: Attendee

```rust
// domain/src/models/attendee.rs — ADD behavior

#[derive(Debug, thiserror::Error)]
pub enum CheckInError {
    #[error("attendee not approved: current status is {0}")]
    NotApproved(String),
    #[error("attendee already checked in at {0}")]
    AlreadyCheckedIn(String),
    #[error("online/virtual attendees cannot check in on-site")]
    OnlineAttendee,
}

impl Attendee {
    /// Can this attendee be checked in on-site?
    pub fn can_check_in(&self) -> Result<(), CheckInError> {
        if self.is_checked_in() {
            return Err(CheckInError::AlreadyCheckedIn(
                self.checked_in_at.clone().unwrap_or_default(),
            ));
        }
        if !self.is_approved() {
            return Err(CheckInError::NotApproved(
                self.approval_status.clone(),
            ));
        }
        if !self.is_in_person() {
            return Err(CheckInError::OnlineAttendee);
        }
        Ok(())
    }

    /// Is deposit verified (USDC confirmed on-chain or THB slip approved)?
    pub fn has_verified_deposit(&self) -> bool {
        self.deposit_verified
    }

    /// Is this attendee eligible for refund?
    pub fn is_refund_eligible(&self) -> bool {
        self.has_verified_deposit()
            && self.refund_status.as_deref() != Some("refunded")
    }
}
```

### Entity: Deposit

```rust
// domain/src/models/deposit.rs — ADD behavior

impl DepositStatus {
    /// Is this deposit within the refundable tier?
    pub fn is_refundable_tier(&self, max_refundable: u32) -> bool {
        self.deposit_order <= max_refundable
    }

    /// Is the deposit past the refund deadline?
    pub fn is_past_deadline(&self, event_end_ms: i64, deadline_hours: u32, now_ms: i64) -> bool {
        let deadline = event_end_ms + (deadline_hours as i64 * 3600_000);
        now_ms > deadline
    }
}
```

### Domain Service: Refund Policy

Cross-entity rules that don't belong to a single entity:

```rust
// domain/src/policies/refund.rs — NEW file

/// Refund eligibility — combines attendee + deposit + event rules.
pub fn is_refund_eligible(
    attendee: &Attendee,
    deposit: &DepositStatus,
    event: &EventConfig,
    now_ms: i64,
) -> RefundEligibility {
    // 1. Already refunded?
    if attendee.refund_status.as_deref() == Some("refunded") {
        return RefundEligibility::AlreadyRefunded;
    }

    // 2. No verified deposit?
    if !deposit.verified {
        return RefundEligibility::DepositNotVerified;
    }

    // 3. Past refund deadline?
    if deposit.is_past_deadline(event.event_end_ms, event.refund_deadline_hours, now_ms) {
        return RefundEligibility::DeadlinePassed;
    }

    // 4. Not in refundable tier?
    if !deposit.is_refundable_tier(event.max_refundable_deposits) {
        return RefundEligibility::NotInRefundableTier;
    }

    // 5. No-show? (checked_in_at is None and event is over)
    if attendee.checked_in_at.is_none() && now_ms > event.event_end_ms {
        return RefundEligibility::NoShowForfeited;
    }

    RefundEligibility::Eligible
}

#[derive(Debug, PartialEq)]
pub enum RefundEligibility {
    Eligible,
    AlreadyRefunded,
    DepositNotVerified,
    DeadlinePassed,
    NotInRefundableTier,
    NoShowForfeited,
}
```

---

## Proposed Module Structure

```
domain/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── models/
    │   ├── mod.rs
    │   ├── event.rs        # Event + EventConfig + behavior
    │   ├── attendee.rs     # Attendee + behavior
    │   ├── deposit.rs      # Deposit + ThbDeposit + behavior
    │   ├── org.rs          # Org (unchanged)
    │   ├── auth.rs         # Auth types (unchanged)
    │   ├── api.rs          # API request/response types (unchanged)
    │   ├── adventure.rs    # Adventure types (unchanged)
    │   └── error.rs        # ADD domain-specific error types
    ├── policies/            # NEW — cross-entity business rules
    │   ├── mod.rs
    │   ├── refund.rs        # Refund eligibility policy
    │   └── capacity.rs      # Capacity gating policy
    ├── repositories.rs      # NEW — trait definitions (no IO)
    └── config/              # Existing
        ├── mod.rs
        └── types.rs

worker/
├── src/
│   ├── lib.rs               # Router + fetch handler (unchanged)
│   ├── state.rs             # AppState (unchanged)
│   ├── error.rs             # AppError (unchanged)
│   ├── http.rs              # HTTP helpers (unchanged)
│   ├── handlers/            # THIN — HTTP → domain → HTTP
│   │   ├── mod.rs
│   │   ├── checkin.rs       # Calls attendee.can_check_in()
│   │   ├── deposit/         # Calls policies::refund::is_refund_eligible()
│   │   ├── events.rs        # Calls event.is_registration_open()
│   │   └── ...
│   ├── adapters/            # RENAME from current modules
│   │   ├── sheets.rs        # Implements AttendeeRepo, EventRepo
│   │   ├── kv_store.rs      # Implements EventRepo, DepositRepo
│   │   ├── d1_store.rs      # Implements AuditRepo (future)
│   │   ├── solana.rs        # Solana RPC adapter
│   │   └── r2_storage.rs    # R2 blob adapter
│   └── middleware/           # Unchanged
└── tests/
    └── serde_contract.rs
```

---

## Repository Pattern (Ports & Adapters)

### Domain defines traits (no IO, no external deps)

```rust
// domain/src/repositories.rs

use crate::models::{Attendee, EventConfig, DepositStatus, ThbDeposit};
use crate::models::error::AppError;

/// Read access to event configuration.
#[async_trait::async_trait]
pub trait EventRepository: Send + Sync {
    async fn get_config(&self, event_id: &str) -> Result<EventConfig, AppError>;
    async fn save_config(&self, config: &EventConfig) -> Result<(), AppError>;
    async fn list_events(&self, org_id: &str) -> Result<Vec<EventConfig>, AppError>;
}

/// Read/write access to attendees.
#[async_trait::async_trait]
pub trait AttendeeRepository: Send + Sync {
    async fn get_attendee(
        &self,
        event_id: &str,
        attendee_id: &str,
    ) -> Result<Attendee, AppError>;
    async fn list_attendees(&self, event_id: &str) -> Result<Vec<Attendee>, AppError>;
    async fn update_attendee(&self, attendee: &Attendee) -> Result<(), AppError>;
    async fn count_in_person(&self, event_id: &str) -> Result<u32, AppError>;
}

/// Read/write access to deposits.
#[async_trait::async_trait]
pub trait DepositRepository: Send + Sync {
    async fn get_deposit(
        &self,
        event_id: &str,
        attendee_id: &str,
    ) -> Result<Option<DepositStatus>, AppError>;
    async fn save_deposit(&self, deposit: &DepositStatus) -> Result<(), AppError>;
    async fn get_thb_deposit(
        &self,
        event_id: &str,
        attendee_id: &str,
    ) -> Result<Option<ThbDeposit>, AppError>;
    async fn list_pending_slips(&self, event_id: &str) -> Result<Vec<ThbDeposit>, AppError>;
}
```

### Worker implements traits (actual IO)

```rust
// worker/src/adapters/kv_store.rs

pub struct KvEventStore {
    kv: KvStore,
}

#[async_trait::async_trait]
impl EventRepository for KvEventStore {
    async fn get_config(&self, event_id: &str) -> Result<EventConfig, AppError> {
        let key = format!("event:{event_id}");
        let value = self.kv.get(&key).text().await?;
        serde_json::from_str(&value).map_err(AppError::Internal)
    }
    // ...
}
```

### Handler becomes thin

```rust
// worker/src/handlers/checkin.rs — BEFORE (logic mixed with IO)
pub async fn check_in(
    State(state): State<AppState>,
    Path(attendee_id): Path<String>,
) -> Result<Json<CheckInResponse>> {
    // 50+ lines of: fetch attendee, check status, check in-person,
    // check deposit, update sheet, update KV, return response
}

// worker/src/handlers/checkin.rs — AFTER (thin delegation)
pub async fn check_in(
    State(state): State<AppState>,
    Path(attendee_id): Path<String>,
) -> Result<Json<CheckInResponse>> {
    let attendee = state.attendee_repo.get_attendee(&event_id, &attendee_id).await?;

    // Domain decides — handler just executes
    attendee.can_check_in().map_err(|e| match e {
        CheckInError::AlreadyCheckedIn(_) => AppError::Forbidden(e.to_string()),
        CheckInError::NotApproved(_) => AppError::Forbidden(e.to_string()),
        CheckInError::OnlineAttendee => AppError::Validation(e.to_string()),
    })?;

    let checked_in = attendee.perform_check_in(staff_email, now_ms());
    state.attendee_repo.update_attendee(&checked_in).await?;

    Ok(Json(CheckInResponse { success: true, .. }))
}
```

---

## TDD: Testing Strategy

### Testing Pyramid

```
        ╱╲
       ╱  ╲         E2E (Playwright) — existing
      ╱ 5% ╲        Full flow on devnet
     ╱──────╲
    ╱        ╲       Integration (axum::test + mock repos)
   ╱   15%    ╲      Handler-level, real router, fake data
  ╱────────────╲
 ╱              ╲    Unit (domain logic) — THE GAP
╱     80%        ╲   Pure Rust, no IO, <1ms each
╱────────────────╲
```

### Test Organization

```
domain/
└── src/
    └── models/
        ├── event.rs          # inline #[cfg(test)] mod tests {}
        ├── attendee.rs       # inline #[cfg(test)] mod tests {} (existing, expand)
        └── deposit.rs        # inline #[cfg(test)] mod tests {}
    └── policies/
        ├── refund.rs         # inline #[cfg(test)] mod tests {}
        └── capacity.rs       # inline #[cfg(test)] mod tests {}

worker/
└── tests/
    ├── domain_rules.rs       # NEW — integration tests for domain rules
    ├── serde_contract.rs     # existing
    └── handler_mocks.rs      # NEW — mock repo implementations

tests/
└── integration/
    └── run.sh                # existing devnet E2E
```

### TDD Workflow Example

#### Feature: Refund eligibility check

**Step 1 — RED (write test first)**

```rust
// domain/src/policies/refund.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refund_eligible_when_checked_in_before_deadline() {
        let attendee = Attendee {
            checked_in_at: Some("2025-06-01T10:00:00Z".into()),
            deposit_verified: true,
            refund_status: None,
            ..make_attendee()
        };
        let deposit = DepositStatus {
            verified: true,
            deposit_order: 1,
            ..make_deposit()
        };
        let event = EventConfig {
            event_end_ms: 1748736000000, // 2025-06-01
            refund_deadline_hours: 24,
            max_refundable_deposits: 10,
            ..make_event()
        };

        let result = is_refund_eligible(&attendee, &deposit, &event, now_before_deadline());
        assert_eq!(result, RefundEligibility::Eligible);
    }

    #[test]
    fn refund_denied_past_deadline() {
        // ... test with now_ms past the deadline
    }

    #[test]
    fn refund_denied_no_show() {
        // ... test with checked_in_at = None and event over
    }

    #[test]
    fn refund_denied_already_refunded() {
        // ... test with refund_status = "refunded"
    }

    #[test]
    fn refund_denied_deposit_not_verified() {
        // ... test with verified = false
    }

    #[test]
    fn refund_denied_not_in_refundable_tier() {
        // ... test with deposit_order > max_refundable_deposits
    }
}
```

**Step 2 — GREEN (minimum code to pass)**

```rust
// domain/src/policies/refund.rs
pub fn is_refund_eligible(...) -> RefundEligibility {
    // implement the rules
}
```

**Step 3 — REFACTOR (handler uses domain)**

```rust
// worker/src/handlers/deposit/refund.rs
// Thin handler calls is_refund_eligible() instead of duplicating logic
```

### Mock Repository for Handler Tests

```rust
// worker/tests/handler_mocks.rs

use event_checkin_domain::repositories::*;

pub struct MockAttendeeRepo {
    pub attendees: std::collections::HashMap<String, Attendee>,
}

#[async_trait::async_trait]
impl AttendeeRepository for MockAttendeeRepo {
    async fn get_attendee(&self, _event_id: &str, attendee_id: &str) -> Result<Attendee, AppError> {
        self.attendees.get(attendee_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("attendee {attendee_id}")))
    }
    async fn update_attendee(&self, _attendee: &Attendee) -> Result<(), AppError> {
        Ok(()) // no-op in mock
    }
    // ...
}
```

---

## Migration Plan

### Phase 1: Domain Behavior (Low Risk)

Move business logic from handlers into domain models. No structural changes.

**Steps:**
1. Add `can_check_in()` to `domain::Attendee`
2. Add `is_registration_open()`, `has_in_person_capacity()`, `accepts_usdc_deposits()` to `domain::EventConfig`
3. Add domain error types to `domain::models::error`
4. Add unit tests for each new method
5. Update handlers to call domain methods instead of inline logic

**Effort:** ~1-2 days
**Risk:** Low — just moving code, handlers still work the same
**Files changed:** `domain/src/models/*.rs`, `worker/src/handlers/*.rs`

### Phase 2: Domain Policies (Medium Risk)

Extract cross-entity rules into `domain/src/policies/`.

**Steps:**
1. Create `domain/src/policies/mod.rs`
2. Extract `policies/refund.rs` from `handlers/deposit/` and `handlers/claim/`
3. Extract `policies/capacity.rs` from `handlers/register.rs` and `handlers/waitlist.rs`
4. Add comprehensive unit tests for each policy
5. Update handlers to use policies

**Effort:** ~1-2 days
**Risk:** Medium — refund logic is money-sensitive, needs careful testing
**Files changed:** New `domain/src/policies/`, update `worker/src/handlers/`

### Phase 3: Repository Traits (Medium Risk)

Define repository interfaces in `domain`, implement in `worker`.

**Steps:**
1. Add `async-trait` dependency to `domain/Cargo.toml`
2. Create `domain/src/repositories.rs` with trait definitions
3. Create `worker/src/adapters/` implementing traits
4. Wire up in `worker/src/state.rs`
5. Add mock implementations for handler tests

**Effort:** ~2-3 days
**Risk:** Medium — significant structural change, but behavior unchanged
**Files changed:** `domain/Cargo.toml`, new `domain/src/repositories.rs`, new `worker/src/adapters/`

### Phase 4: TDD for New Features (Ongoing)

Apply TDD workflow for all new features going forward.

**Rules:**
1. **RED first** — write the failing test in `domain/`
2. **GREEN** — implement minimum code to pass
3. **REFACTOR** — handler becomes thin delegation
4. **No domain test = no feature** — if it's a business rule, it has a domain test

---

## Decisions Needed

### 1. `async-trait` in domain crate?

Repository traits need `async_trait`. This adds a dependency to the domain crate.

- **Option A**: Add `async-trait` to `domain` — traits live where they belong
- **Option B**: Keep traits in `worker` — domain stays dependency-free but loses port/adapter purity

**Recommendation**: Option A. The `async-trait` crate is lightweight, widely used, and compiles to both x86_64 + wasm32.

### 2. How far to take the refactoring?

- **Option A**: Only extract business rules into domain models (Phase 1+2) — quick win, no structural change
- **Option B**: Full ports & adapters with repository traits (Phase 1-3) — maximum testability
- **Option C**: Minimal — just add tests to existing handlers without restructuring

**Recommendation**: Option A first, then Option B when we have time. Don't block feature work.

### 3. Where do on-chain validation rules live?

Rules like "escrow must be Initialized to accept deposits" could live in:
- `domain::Event` (pure Rust, unaware of on-chain mechanics)
- `worker::solana_escrow` (closer to the source of truth)

**Recommendation**: Status check in `domain::Event` (`escrow_status == Initialized`), actual on-chain verification in `worker::solana_escrow`.

### 4. Test scope for Phase 1?

- **Option A**: Only test new domain methods, don't touch existing handler tests
- **Option B**: Add domain tests + verify handlers still pass via existing E2E

**Recommendation**: Option B. Domain tests prove correctness, E2E proves integration.

### 5. Naming: `adapters/` vs current module names?

Current: `sheets/`, `event_store/`, `solana_escrow/`, `storage/`
Proposed: `adapters/sheets.rs`, `adapters/kv_store.rs`, `adapters/solana.rs`, `adapters/r2_storage.rs`

**Recommendation**: Keep current names during Phase 1-2, rename to `adapters/` in Phase 3 if we do repository traits.

---

## Appendix: Existing Business Rules Inventory

Rules extracted from current codebase that should move to domain:

| Rule | Current Location | Proposed Location | Priority |
|---|---|---|---|
| Attendee must be approved to check in | `handlers/checkin.rs` | `domain::Attendee::can_check_in()` | P0 |
| Online attendees can't check in on-site | `handlers/checkin.rs` | `domain::Attendee::can_check_in()` | P0 |
| Already checked-in = reject | `handlers/checkin.rs` | `domain::Attendee::can_check_in()` | P0 |
| Deposit must be verified for refund | `handlers/deposit/` | `domain::policies::refund` | P0 |
| Refund deadline from event config | Multiple handlers | `domain::policies::refund` | P0 |
| Max refundable deposits tier | `handlers/deposit/` | `domain::policies::refund` | P1 |
| In-person capacity gating | `handlers/register.rs` | `domain::Event::has_in_person_capacity()` | P1 |
| Online capacity gating | `handlers/register.rs` | `domain::Event::has_online_capacity()` | P1 |
| Deposit deadline from registration date | `handlers/deposit/` | `domain::Event::deposit_deadline_passed()` | P1 |
| Escrow status gates USDC deposits | `handlers/deposit/` | `domain::Event::accepts_usdc_deposits()` | P1 |
| Event must be Active for registration | `handlers/register.rs` | `domain::Event::is_registration_open()` | P2 |
| THB slip verification flow | `handlers/deposit/` | `domain::policies::deposit` | P2 |
| Walk-in attendee creation rules | `handlers/walkin.rs` | `domain::Attendee::new_walkin()` | P2 |
| NFT claim eligibility | `handlers/claim.rs` | `domain::Attendee::can_claim()` | P2 |

---

## Future Consideration: Solana Subscriptions & Allowances

> **Status**: Research — see `.issues/048_solana_subscriptions_allowances.md` for full analysis.

Solana launched a native **Subscriptions & Allowances** program on mainnet (2026-06-02). The **Fixed Delegation** model could replace our custom Quasar escrow for USDC deposits.

### Why This Matters for DDD

If we adopt Fixed Delegations, the domain model changes:

| Current Domain Concept | With Fixed Delegations |
|---|---|
| `EscrowStatus` enum | Replaced by delegation `expiryTs` |
| `solana_escrow` module | Replaced by `@solana/subscriptions` calls |
| `escrow_indexer` module | Replaced by on-chain events from subscriptions program |
| `DepositStatus.verified` | Implicit — delegation exists = authorized |
| Refund = explicit TX | Refund = revoke delegation or let expire |
| `max_refundable_deposits` tier | Still needed at worker level (not on-chain) |

### What Stays the Same

- `domain::Attendee::can_check_in()` — check-in logic is independent of payment mechanism
- `domain::policies::refund` — refund eligibility rules still apply, just the execution differs
- `domain::Event::has_in_person_capacity()` — capacity logic unchanged
- THB deposit flow — completely independent of USDC mechanism

### What Changes

- `domain::Event::accepts_usdc_deposits()` — instead of checking `escrow_status == Initialized`, check if organizer has a valid subscription authority
- `domain::DepositStatus` — `tx_signature` replaced by `delegation_pda`
- Worker adapters — `solana_escrow/` replaced by `subscriptions/` adapter implementing same `DepositRepository` trait

### DDD Implication

**This is exactly why we want the repository pattern (Phase 3).** If we define:

```rust
pub trait DepositRepository: Send + Sync {
    async fn authorize_deposit(&self, attendee: &str, event: &str, amount: u64, expiry_ts: i64) -> Result<DepositAuth, AppError>;
    async fn draw_deposit(&self, auth: &DepositAuth, receiver: &str) -> Result<(), AppError>;
    async fn revoke_deposit(&self, auth: &DepositAuth) -> Result<(), AppError>;
}
```

Then swapping Quasar → Fixed Delegations is just swapping the **adapter implementation**, not the domain logic. Handlers and policies remain unchanged.

### Recommendation

1. **Don't design around Fixed Delegations yet** — program is 1 day old
2. **Design domain with repository traits** — this makes future migration a one-module swap
3. **Track in Issue #048** — re-evaluate in 2-3 months when battle-tested

---

*Document created for review. No code changes until decisions are made.*
