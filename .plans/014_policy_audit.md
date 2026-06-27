# Plan 014 — Phase 3.1: Policy Audit (Conditional Rules in `worker/src/`)

> Task 3.1 deliverable. Categorize every conditional rule as `universal`
> (same for all events) vs `parameterized` (varies). This audit is the
> precondition for Task 3.2 (`EventPolicy` trait) — it determines whether a
> trait is justified or cargo-cult.

---

## Headline finding (honest)

**Task 3.2 (`EventPolicy` trait) is NOT justified. Recommend demotion.**

The audit confirms the negative-results log's framing that our policies are
"deterministic (`if role != SuperAdmin { deny }`)" — but goes further: the
per-event variation that Phase 3.2 was designed to abstract is **already
centralized** as inherent methods on `EventConfig` in
`domain/src/models/event.rs`. There is no behavioral variation to trait-ify.

Concretely:
- Every per-event rule is a **universal formula parameterized by data fields**
  on `EventConfig`, not a different code path per event class.
- A `Policy` trait with `DefaultPolicy` + `PaidEventPolicy` impls would wrap a
  single formula in polymorphism that has exactly one implementation. That is
  the textbook definition of premature abstraction.
- The genuine problem surfaced by the audit is **duplication / SSOT violation**
  (Phase 2 territory): the *same* predicate is reimplemented inline at 14+
  worker call sites, ignoring the domain method that already captures it.

The win Phase 3.2 was reaching for is real, but it is an **SSOT consolidation**
(Phase 2.3), not a **trait introduction** (Phase 3.2).

---

## Method

`rg` over `worker/src/`, `domain/src/`, `frontend-leptos/src/` for:
- policy-relevant predicates: `deposit_enabled`, `refund_deadline_hours`,
  `accepts_usdc_deposits`, `is_refund_eligible`, `is_registration_open`,
  `deposit_deadline_passed`, `walkin`, `UserRole`, `check_event_access`.
- per-org policy fields: `rg "policy|rule|tier|plan" domain/src/models/org.rs`
  → none.

> Note: the `grep` tool's index returned zero matches for `deposit` /
> `refund` / `walkin` in `worker/**/*.rs` despite a `handlers/deposit/`
> folder existing. Cross-checked with `rg` (matches in 20+ files). This is
> the stale-index failure mode flagged in the global agent rules — `rg` is
> authoritative.

---

## Inventory

### Category A — UNIVERSAL rules (same formula for every event)

These are already correctly centralized and need **no** trait.

#### A1. Role-based access control

**Location:** `worker/src/auth.rs:485-560` (`UserRole`, `resolve_user_role`,
`check_event_access`).

**Shape:** `enum UserRole { Staff, Organizer, SuperAdmin }` with `derive(Ord)`
giving total privilege ordering `Staff < Organizer < SuperAdmin`. Resolved once
per request via `resolve_user_role`, then a single `check_event_access` call.

**Verdict:** This is textbook type-state done right — the enum's `Ord` derive
*is* the policy. A `Policy` trait would add a dispatch layer over an already-
total ordering. **No change.**

#### A2. Per-event data-parameterized rules (the "parameterized" candidates)

All live as **inherent methods on `EventConfig`** in
`domain/src/models/event.rs:551-595`:

| Rule | Method | Formula | Varies per-event? |
|---|---|---|---|
| Registration open | `is_registration_open(now_ms)` | `status == Active && (start_ms == 0 \|\| now < start_ms)` | inputs vary |
| Refund eligible | `is_refund_eligible(now_ms)` | `now <= event_end_ms + refund_deadline_hours * 3_600_000` | inputs vary |
| USDC deposits accepted | `accepts_usdc_deposits()` | `deposit_enabled && escrow_status == Initialized` | inputs vary |
| Deposit deadline passed | `deposit_deadline_passed(reg_ms, now_ms)` | `now > reg_ms + deposit_deadline_hours * 3_600_000` (if `Some`) | inputs vary |
| In-person capacity | `has_in_person_capacity(count)` | `in_person_capacity.is_none_or(\|cap\| count < cap)` | inputs vary |
| Online capacity | `has_online_capacity(count)` | `online_capacity.is_none_or(\|cap\| count < cap)` | inputs vary |

**Verdict:** The *formula* is universal. The *inputs* (`refund_deadline_hours`,
`deposit_enabled`, `escrow_status`, capacities) are per-event **data fields**.
An inherent method on the data type is the correct Rust idiom for this — it is
already parameterized by construction. A `Policy` trait whose only impl reads
the same fields adds indirection with zero new behavior.

This is the crux: Phase 3.2 assumed "varies per-event" = "needs a trait." The
audit shows "varies per-event" = "reads a struct field." Those are different.

### Category B — Per-ORG behavioral rules

**Finding:** None. `OrganizationConfig` (`domain/src/models/org.rs:12`) is pure
metadata: `id`, `name`, `slug`, `contacts_sheet_id`, display fields. `rg` for
`policy|rule|tier|plan` over `domain/src/models/org.rs` returns no matches.

**Verdict:** There is no per-org policy layer to abstract. Multi-tenancy is
data scoping (`WHERE organization_id = ?`), not behavioral dispatch.

---

## The real problem: duplication (Phase 2.3, not Phase 3.2)

The audit surfaced that the *domain methods exist* but the *call sites don't
use them consistently*. This is an SSOT violation, and it is the actual bug-
shaped win hiding inside Phase 3's framing.

### D1. `accepts_usdc_deposits()` ignored at 14 call sites

The domain method captures the correct predicate:
```rust
// domain/src/models/event.rs:581
pub fn accepts_usdc_deposits(&self) -> bool {
    self.deposit_enabled && self.escrow_status == EscrowStatus::Initialized
}
```

But the worker reinvents a **strictly weaker** inline check 14 times:

```
worker/src/handlers/attendee.rs:594            if !meta.deposit_enabled {
worker/src/handlers/deposit/escrow/status.rs:149   if !event.deposit_enabled {
worker/src/handlers/deposit/escrow/status.rs:452   if !source_event.deposit_enabled || !target_event.deposit_enabled {
worker/src/handlers/deposit/escrow/handlers.rs:39,183,288,439,537,639,793  (7 sites)
worker/src/handlers/deposit/thb/handlers/slip_upload.rs:123
worker/src/handlers/deposit/thb/handlers/hold_credit.rs:55
worker/src/handlers/deposit/usdc/handlers.rs:189,469
```

**Bug shape:** `!event.deposit_enabled` only checks the flag, not the escrow
state. `accepts_usdc_deposits()` is stricter (also requires
`escrow_status == Initialized`). The single site that uses the method correctly
(`deposit/usdc/handlers.rs:93`) is the exception, not the rule.

This is a real correctness smell: some deposit endpoints gate only on the
flag, others (USDC) also gate on escrow state. Whether that asymmetry is
intentional (e.g. THB slip upload allowed during `Pending` escrow init) or a
latent bug is **worth a focused review** — but that review belongs in Phase 2
(SSOT), not Phase 3 (traits).

### D2. Refund-deadline formula recomputed in 5 places

```
domain/src/models/event.rs:563                 event_end_ms + (refund_deadline_hours as i64 * 3_600_000)
worker/src/handlers/deposit/escrow/handlers.rs:97   event_end + (refund_deadline_hours as i64 * 3600)
worker/src/handlers/deposit/usdc/handlers.rs:128    event_end_ms + (i64::from(refund_deadline_hours) * 3_600_000)
worker/src/cleanup.rs:84                             i64::from(refund_deadline_hours) * 3600
frontend-leptos/src/pages/deposit/types.rs:322       event_end_ms + (i64::from(refund_deadline_hours) * 3_600_000)
```

Note: two sites use `3600` (seconds), three use `3_600_000` (ms). The
`cleanup.rs:84` and `escrow/handlers.rs:97` sites are seconds-flavored — they
feed a different unit into their callers. A single
`EventConfig::refund_deadline_ms() -> i64` method would kill this entire class
of unit-confusion drift.

### D3. Frontend mirrors the same predicates

`frontend-leptos` recomputes the refund deadline inline
(`pages/deposit/types.rs:321`, `pages/public_event/deposit_section.rs`) and
re-derives "has deposit" inline (`public_event/deposit_section.rs:9`).
This is the cross-boundary SSOT concern Phase 2 / Blocker A flagged: the
frontend cannot depend on `domain` (wasm build constraint), so it re-implements
predicates from JSON fields. Phase 2.3's CI check is the right lever here, not
Phase 3 traits.

---

## Verdict per task

| Task | Plan framing | Audit verdict | Action |
|---|---|---|---|
| 3.1 | Inventory conditional rules | ✅ Done (this doc) | — |
| 3.2 | Define `EventPolicy` trait + `DefaultPolicy` + `PaidEventPolicy` | ❌ **Not justified** — no behavioral variation; rules already parameterized via `EventConfig` fields | **Demote** → negative-results entry #7 |
| 3.3 | Document "no neuro-symbolic" decision | Already done (negative-results #2) | Close as duplicate |
| 3.4 | Verify audit coverage of policy decisions | Partially covered by D1/D2 above | Fold into Phase 2.3 scope |

### What survives from Phase 3

- **3.3's conclusion** (deterministic, never stochastic) is already durable in
  negative-results entry #2 and the global "deterministic-not-stochastic"
  discipline (Phase 5.3). No new doc needed.
- **The duplication findings (D1, D2, D3)** are real, actionable, and belong
  in Phase 2.3 (SSOT). They are the honest kernel of value from this audit.

---

## Preconditions that would re-open Phase 3.2 (the trait)

A trait becomes justified only when there is **genuine behavioral polymorphism**
— i.e. two events with the same inputs produce different decisions because they
follow different *formulas*. None of these hold today:

1. A new event class with a structurally different rule (e.g. "free events
   allow walk-in without deposit; paid events require deposit *and* ID
   verification" — two formulas, not two field values). Not on roadmap.
2. Per-org policy overrides (e.g. an org caps all refund windows at 48h
   regardless of event config). `OrganizationConfig` has no such field today.
3. A plugin/extension model where third-party code supplies policy impls.
   Not on roadmap; would also need a sandbox story for monetary code.

Until one of these is real, `EventConfig` inherent methods + Phase 2.3 SSOT
consolidation is the correct, lower-abstraction-cost solution.

---

## Recommended next step (revised)

**Phase 2.3 SSOT consolidation is now the highest-value *code* win**, replacing
the Phase 3.2 slot. Concrete first move:

1. Add `EventConfig::refund_deadline_ms(&self) -> i64` and
   `EventConfig::deposit_window_passed(...)` (consolidate D2).
2. Replace the 14 `if !event.deposit_enabled` sites (D1) with either
   `accepts_usdc_deposits()` (where escrow state matters) or a new
   `deposit_feature_enabled()` method (where only the flag matters) — and
   **document which is which** per site, since the current asymmetry may be
   intentional.
3. The frontend predicates (D3) stay as-is until Phase 2.3's CI check lands;
   they are a known cost of the wasm build boundary.

Phase 4.1 (live `wrangler tail` profile) remains the highest-leverage *next*
step overall, but it requires a staged 200-attendee event and is blocked on
infrastructure coordination — not autonomously executable.

---

## ⚠️ CORRECTION (added after the deeper code read executed for Phase 2.3)

> **The D1 and D2 recommendations above are RETRACTED.** Executing the proposed
> consolidation would have introduced real bugs. This section documents why,
> per-site, and is the authoritative reading — the D1/D2 sections above are
> preserved only as the audit trail of the error.

### D1 retraction: the "asymmetry" is a deliberate two-stage guard, not a bug

On reading every one of the 14 `if !event.deposit_enabled` sites in context,
the pattern is uniform and intentional. Each is the **first stage** of a
multi-stage guard: a cheap fast-fail that emits "deposit not enabled" before
any escrow work; the **second stage** then checks a *specific* escrow state
that varies per endpoint. `accepts_usdc_deposits()` collapses to
`Initialized` only, which is **wrong** for most of these:

| Site | 2nd-stage check (what it actually needs) | `accepts_usdc_deposits()` would... |
|---|---|---|
| `escrow/handlers.rs:39` (init_escrow) | builds escrow (needs status `None`) | **break the entire init flow** |
| `escrow/handlers.rs:183` (refund+close) | then `escrow_address.is_empty()` | tighten gate, wrong error path |
| `escrow/handlers.rs:288` (mark_checked_in) | then `escrow_address.is_empty()` | tighten gate |
| `escrow/handlers.rs:439` (deactivate) | then `!= Initialized` | change required state |
| `escrow/handlers.rs:537` (close_event) | then `!= Deactivated` | **wrong — needs Deactivated** |
| `escrow/handlers.rs:639` (claim_forfeited) | then `!= Deactivated` | **wrong — needs Deactivated** |
| `escrow/handlers.rs:793` (close_deposit) | then per-deposit `status.verified` | redundant; verified-check is the real gate |
| `escrow/status.rs:149` (confirm_init) | transitioning `None→Initialized` | **break the confirm flow** |
| `escrow/status.rs:452` (rollover) | `source \|\| target` cross-event | **cannot be expressed by a single-event method** |
| `thb/slip_upload.rs:123` | then `deposit_amount_thb` | **wrong — THB has no on-chain escrow** |
| `thb/hold_credit.rs:55` | THB hold-credit flow | **wrong — THB has no on-chain escrow** |
| `usdc/handlers.rs:189` (deposit) | then `deposit_amount_usdc` | escrow verified downstream |
| `usdc/handlers.rs:469` (tx callback) | then `deposit_amount_usdc` | escrow verified downstream |
| `attendee.rs:594` (rollover filter) | then separate `escrow_status` check on :599 | already a two-stage guard |

**Verdict:** the flag check is the correct, cheap, uniform first stage. The
specific escrow-state requirement is deliberately per-endpoint and CANNOT be
collapsed into one method. `accepts_usdc_deposits()` is correctly used in
exactly the one place where "Initialized" is the full gate (the deposit-status
*display* response at `usdc/handlers.rs:93`). There is no consolidation to do.

### D2 retraction: the seconds/ms split is a domain boundary, not a unit bug

The audit's "unit-confusion smell" framing was wrong. There are two distinct
unit domains and each recomputation is internally consistent:

- **Seconds domain** (Solana on-chain program + KV TTL): `escrow/handlers.rs:97`
  consumes `event.event_end_ms / 1000` (line 90) and multiplies by `3600`;
  `cleanup.rs:84` computes a KV-TTL duration in seconds. Solana's
  `Clock::unix_timestamp` is in **seconds**, and Cloudflare KV TTL is in
  **seconds**. Both are correct.
- **Milliseconds domain** (HTTP API + frontend): `domain/event.rs:563`
  (`is_refund_eligible`) and `usdc/handlers.rs:128` (status response) operate in
  epoch ms; the frontend mirrors this. All correct.

Additionally, the two ms-domain recomputations have **deliberately different
missing-data semantics** that make them non-trivially unifiable:
- `domain/event.rs:562 is_refund_eligible`: no guard — computes
  `event_end_ms + hours*3_600_000`; when `event_end_ms == 0` this yields an
  early-1970 deadline → `now <= deadline` is false → fail-safe "not eligible".
- `usdc/handlers.rs:127`: guards with `event_end_ms > 0 && hours > 0` and
  returns `0` on miss, which the frontend treats as "hide the CTA".

A single `refund_deadline_ms()` method would have to pick one missing-data
semantic, silently changing the other call site's behavior. That is a
behavioral decision requiring tests, not a mechanical refactor.

### What this means for Plan 014

This is the **fourth consecutive audit miss** in Plan 014 (after Phase 1's
EventMetaWire/AttendeeListItem/quiz-batch misses). The pattern is consistent:
**the audits reliably over-estimate how much of the codebase is wrong or
consolidable.** The GOAT-gate discipline caught it again — this time *before*
any code shipped, because the consolidation plan was itself GOAT-gated against
the actual call-site semantics.

**Net actionable change from this audit: NONE.** The Phase 2.3 SSOT
consolidation, as proposed, is demoted. See negative-results entry #8. The only
residual value is the *documentation* that the two-stage guard pattern is
deliberate (so future agents don't re-propose collapsing it).

### Revised next-step recommendation

- **Phase 4.1 (profile)** remains highest-leverage overall but is blocked on a
  staged 200-attendee event.
- **Phase 2.3 (SSOT CI check)** is still worth building as a *detection* tool
  (a lint that flags genuinely duplicated predicates across worker/frontend),
  but the *remediation* list is now known to be near-empty for the deposit/
  refund predicates specifically. The CI check's value is forward-looking
  (catching *future* duplication), not retroactive cleanup.
