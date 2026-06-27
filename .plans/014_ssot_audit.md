# Plan 014 Phase 2.1 — Cross-Crate SSOT Audit Findings

**Status:** ✅ COMPLETE (audit only — no code changes)
**Date:** 2026-06-27
**Scope:** Logic duplicated across `worker/src/` and `frontend-leptos/src/`
**Discipline:** Audit-first. Measure reality before claiming duplication exists. Classify each candidate as genuine duplication vs. deliberate pattern.

---

## Executive Summary

This is the **7th consecutive Plan 014 audit**. As with the prior six, the
plan's stated premises did not match the codebase — but this audit surfaced a
**real, actionable finding** that the prior Phase 2.3 guard missed.

**Headline finding:** The Phase 2.3 mirror-types guard
(`frontend-leptos/tests/ssot_mirror_audit.rs`) has a **scope gap** that hides
three load-bearing mirrored business predicates. The guard's
`MIRROR_FILES = &["src/api/types.rs"]` constant excludes `api/event.rs`, which
contains the same kind of mirror types (with the same kind of doc comments,
just worded slightly differently). The result: three business predicates are
silently mirrored in the frontend with **no allowlist entry, no regression
guard, and active call sites** — exactly the silent-divergence risk the guard
was built to prevent.

**Secondary finding (genuine, removable):** Two worker sites hand-map
`DepositMethod ↔ string` instead of using the domain `Display` impl / serde
`rename_all = "snake_case"`. Removable in Phase 2.2 with zero behavior change.

**Non-findings (deliberate patterns, not duplication):**
- Participation-type normalization is NOT duplicated (domain is the SSOT).
- Frontend `label()` / `icon_name()` / `css_class()` helpers are UI
  presentation, legitimately frontend-only.

| # | Plan's stated candidate | Reality |
|---|---|---|
| 1 | Participation-type normalization | Domain is SSOT; worker delegates; frontend doesn't parse client-side. **No duplication.** |
| 2 | DepositMethod enum → string in "3+ places" | 2 genuine removable sites in worker; 9 equality-comparison sites (smell, not dup); frontend `as_str()` is dup-of-Display. **Partial.** |
| 3 | Escrow-state predicates | 3 mirrored business predicates in `api/event.rs` uncovered by Phase 2.3 guard; **actively used in 6+ call sites.** |

---

## Candidate 1 — Participation-Type Normalization

### Plan's premise
> "worker has `normalize_override`, leptos likely re-implements display logic"

### Reality: NOT DUPLICATED

**Domain provides the full SSOT** in `domain/src/models/attendee.rs`:
- `ParticipationType::parse(s: &str) -> Self` (L94+) — case-insensitive
  substring matching; handles "In-Person", "physical", "virtual", legacy empty.
- `ParticipationType::as_str(&self) -> &'static str` (L66-76) — canonical
  snake_case for D1 storage.
- `ParticipationType::display(&self) -> &'static str` (L77-81) — display-case
  for Sheet/UI.
- `impl FromStr` and `impl Display` delegating to the above.

**Worker's `normalize_override`** (`worker/src/handlers/attendee.rs:1052-1062`)
is a **thin HTTP-layer validation wrapper**, not a re-implementation:
```rust
fn normalize_override(raw: &str) -> Result<ParticipationType, AppError> {
    if raw.trim().is_empty() {
        return Err(AppError::Validation("participation_type must not be empty".to_string()));
    }
    let parsed = ParticipationType::parse(raw);   // ← delegates to domain
    if parsed == ParticipationType::Other {
        return Err(AppError::Validation(format!(...)));
    }
    Ok(parsed)
}
```
The empty-string rejection and the "Other"/"walkin" sentinel rejection are
worker-specific HTTP-input concerns. This is a **legitimate separation of
concerns**, not duplication — identical to the two-stage guard pattern from
Phase 3.1 / negative-results entry #8.

**Frontend does NOT parse `participation_type` client-side.** It carries the
field as a raw `String` (`AttendeeResponse.participation_type`, `ClaimLookupData`)
and writes display-case back via `update_participation_type(new_value: &str)`
(`frontend-leptos/src/api/attendee.rs:323`), which forwards the raw string to
the backend. The backend is the parser; the frontend is a pass-through.

**Verdict: No duplication. No action.**

---

## Candidate 2 — DepositMethod Enum Mapping

### Plan's premise
> "deposit-status enum mapping (worker matches on `DepositMethod` → string in 3+ places)"

### Reality: 2 genuine removable sites + UI presentation (legit)

**Domain `DepositMethod`** (`domain/src/models/deposit.rs:12-32`):
```rust
#[derive(..., Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositMethod { Usdc, Thb, CreditThb, CreditUsdc }

impl std::fmt::Display for DepositMethod {  // → "usdc"/"thb"/"credit_thb"/"credit_usdc"
    ...
}
```
Note: domain has `Display` but NOT `as_str()` (unlike `ParticipationType`,
`CheckInStatus`, `EscrowStatus`, which all expose `as_str()`).

#### 2a. Genuine removable duplication (worker)

**Site 1 — enum → string for JSON output**
(`worker/src/handlers/attendee.rs:367-372`, inside `get_public_ticket`):
```rust
"method": match d.method {
    DepositMethod::Usdc => "usdc",
    DepositMethod::Thb => "thb",
    DepositMethod::CreditThb => "credit_thb",
    DepositMethod::CreditUsdc => "credit_usdc",
},
```
**Duplicates `Display::to_string()`.** Removable: replace with
`"method": d.method.to_string()` (or `d.method.as_str()` if domain adds one).

**Site 2 — string → enum for DB row parsing**
(`worker/src/db/deposit_statuses.rs:352-360`, inside `row_to_deposit_status`):
```rust
let method = match method_str.as_str() {
    "usdc" => DepositMethod::Usdc,
    "thb" => DepositMethod::Thb,
    "credit_thb" => DepositMethod::CreditThb,
    "credit_usdc" => DepositMethod::CreditUsdc,
    other => return Err(format!("unknown DepositMethod: '{other}'")),
};
```
**Duplicates serde deserialize.** Removable: derive `FromStr` on domain (or
expose the existing parse via `serde_json::from_value`/`serde_json::from_str`
with the snake_case rename). The explicit `Err` on unknown variants is the
only nuance — serde would produce a different error message; preserve the
context string when migrating.

The plan said "3+ places". **Only 2 serialization sites exist.** The plan
appears to have miscounted equality comparisons (below) as serialization.

#### 2b. Equality comparisons — NOT duplication (smell only)

9 worker sites use `d.method == DepositMethod::Usdc` or
`method != DepositMethod::Usdc`:
- `worker/src/handlers/attendee.rs:313, 455`
- `worker/src/handlers/deposit/escrow/handlers.rs:210, 687, 814`
- `worker/src/handlers/deposit/escrow/status.rs:46, 95, 99, 103`
- `worker/src/handlers/deposit/escrow/handlers.rs:562` (rollover)
- `worker/src/handlers/register.rs:1017`

These are **not serialization duplication** — they're equality checks on a
typed enum, which is type-safe. The mild smell is that 9 sites special-case
exactly one variant (`Usdc`) of a 4-variant enum. An optional ergonomic
`DepositMethod::is_usdc(&self) -> bool` on domain would self-document intent
and centralize the "only USDC supports X" rule, but this is a readability
concern, not a correctness or duplication concern.

**Verdict for 2b: Not duplication. Optional ergonomic improvement, defer.**

#### 2c. Frontend `DepositMethod` mirror — UI helpers are legit

`frontend-leptos/src/api/types.rs:65-95` mirrors the enum and adds:
- `as_str()` — duplicates domain `Display` (mild dup; same value).
- `label()` — returns `"USDC (Solana)"`, `"THB (PromptPay)"`, etc. **UI
  presentation.** Domain intentionally lacks this; it's a frontend concern.
- `icon_name()` — returns `"coin"` / `"baht"`. **UI presentation.** Domain
  has no concept of icons; this cannot live in domain.

**Verdict for 2c:** `label()` and `icon_name()` are legitimately frontend.
`as_str()` is mild dup (could delegate if domain grew `as_str()`), but it's
explicitly excluded from the Phase 2.3 predicate guard as a "UI helper".

**Overall Candidate 2 verdict: 2 genuine removable sites (Site 1, Site 2).
Carry forward to Phase 2.2.**

---

## Candidate 3 — Escrow-State & Event-Format Predicates

### Plan's premise
> "escrow-state predicates" duplicated across crates.

### Reality: 3 mirrored business predicates UNCOVERED by the Phase 2.3 guard

This is the headline finding. Domain defines two enums with business
predicates in `domain/src/models/event.rs`:

```rust
// L89-106
impl EventFormat {
    pub fn has_in_person(&self) -> bool { matches!(self, Self::InPerson | Self::Hybrid) }
    pub fn has_online(&self) -> bool { matches!(self, Self::Online | Self::Hybrid) }
}

// L143-147
impl EscrowStatus {
    pub fn is_active(&self) -> bool { matches!(self, Self::Initialized | Self::Deactivated) }
}
```

Frontend re-implements **all three** in `frontend-leptos/src/api/event.rs`
(with identical match arms):

```rust
// event.rs L75-83
impl EventFormat {
    pub fn has_in_person(&self) -> bool { matches!(self, Self::InPerson | Self::Hybrid) }
    pub fn has_online(&self) -> bool { matches!(self, Self::Online | Self::Hybrid) }
}

// event.rs L43-45
impl EscrowStatus {
    pub fn is_active(&self) -> bool { matches!(self, Self::Initialized | Self::Deactivated) }
}
```

These are **business predicates** by the Phase 2.3 guard's own definition
(methods starting with `is_`/`can_`/`has_`/`should_`/`requires_`/`allows_`).
They are **actively used** in the frontend:

| Predicate | Frontend call sites |
|---|---|
| `EventFormat::has_in_person()` | `pages/admin.rs:974, 1044, 1158, 1195`; `pages/event_form.rs:1543`; `pages/scanner.rs:1476` (×2) — **7 call sites** |
| `EventFormat::has_online()` | `pages/admin.rs:995` |
| `EscrowStatus::is_active()` | (defined; not directly called as of audit — UI uses string compare for cancel flow, see `pages/admin_cancel.rs:129`) |

If domain tightens `has_in_person()` (e.g. adds a 4th format variant), the
frontend mirror silently diverges — the UI would show/hide the wrong nav
groups, scanner buttons, and deposit gates. This is exactly the divergence
risk the Phase 2.3 guard exists to prevent.

#### Why the Phase 2.3 guard missed them

The guard's `MIRROR_FILES` constant is too narrow:

```rust
// frontend-leptos/tests/ssot_mirror_audit.rs:88
const MIRROR_FILES: &[&str] = &["src/api/types.rs"];
```

The guard's own doc comment (lines 40-44) admits the scope assumption:
> "Mirror types outside `api/types.rs`. The audit found that `api/types.rs`
> is the only file with the `/// Mirrors domain::...` doc comment pattern. If
> a second mirror-types file appears, this guard's scope constant
> (`MIRROR_FILES`) must be updated."

**The assumption was wrong at ship time.** `api/event.rs` already contained
four mirror types, just worded slightly differently — lowercase "mirrors
backend X" instead of "Mirrors domain::X":

| `api/event.rs` line | Doc comment |
|---|---|
| L10-11 | `/// Event status (mirrors backend EventStatus).` |
| L21-22 | `/// On-chain escrow lifecycle status (mirrors backend EscrowStatus).` |
| L50-51 | `/// Event format (mirrors backend EventFormat).` |
| L116-117 | `/// Event visibility (mirrors backend EventVisibility).` |

A text-pattern-based audit ("find `Mirrors domain::`") missed these because
of capitalization and path wording. The mirror types were always there; the
audit pattern was too narrow.

`api/admin.rs` mirrors `FormFieldConfig` (L422) and
`RegistrationFormConfig` (L466) — both pure data structs, **no business
predicates**. Confirmed.

#### Worker side — NOT duplicated

The worker uses domain's `EscrowStatus` type directly via
`event_checkin_domain::models::event::EscrowStatus`. A grep for
`escrow_status.*==.*"(none|initialized|deactivated|closed|cancelled)"`
returned **zero** worker matches. Worker makes decisions on the typed enum,
not on status strings. No duplication on the worker side.

**Verdict: 3 uncovered mirrored business predicates in `api/event.rs`.
Phase 2.3 guard scope must be widened. Carry forward to Phase 2.2 / 2.3 fix.**

---

## Domain Predicate Baseline (re-verified)

The Phase 2.3 baseline claimed 18 domain predicates. This audit re-counted
18 `pub fn (is_|can_|has_|should_|requires_|allows_)*(&self)` methods in
`domain/src/models/`. **Baseline confirmed.**

| Module | Predicates |
|---|---|
| `attendee.rs` (`Attendee`) | `is_approved`, `is_checked_in`, `is_in_person`, `can_check_in`, `has_verified_deposit`, `is_refund_eligible` |
| `attendee.rs` (`ColumnMapping`) | `is_valid` |
| `deposit.rs` (`DepositStatus`) | `is_refundable_tier`, `is_past_deadline` |
| `error.rs` (`AppError`) | `is_not_found`, `is_auth_error` |
| `event.rs` (`EventFormat`) | `has_in_person`, `has_online` |
| `event.rs` (`EscrowStatus`) | `is_active` |
| `event.rs` (`EventConfig`) | `is_registration_open`, `is_refund_eligible`, `has_in_person_capacity`, `has_online_capacity` |
| **Total** | **18** |

Note: `ColumnMapping::is_valid`, `AppError::is_not_found`,
`AppError::is_auth_error` are infrastructure classification, not monetary
business rules. They're in the count because the Phase 2.3 guard scans by
naming convention, not semantics. Consistent with baseline.

---

## Cross-Reference: Frontend Mirror-Predicate Inventory

**Complete inventory** of business predicates mirrored in the frontend
(audited across all of `frontend-leptos/src/api/`):

| # | Predicate | Mirror location | In Phase 2.3 allowlist? |
|---|---|---|---|
| 1 | `CheckInStatus::is_approved()` | `api/types.rs` | ✅ Yes |
| 2 | `EscrowStatus::is_active()` | `api/event.rs` | ❌ No — **uncovered** |
| 3 | `EventFormat::has_in_person()` | `api/event.rs` | ❌ No — **uncovered, 6 call sites** |
| 4 | `EventFormat::has_online()` | `api/event.rs` | ❌ No — **uncovered, 1 call site** |

`as_str()` methods on mirror types (`CheckInStatus`, `DepositMethod`,
`EscrowStatus`, `EventFormat`, `OnlineOpenMode`) are serialization helpers,
explicitly excluded by the Phase 2.3 guard design. They are mild dups of
domain `Display`/`as_str()`, tracked here but not actionable as predicates.

---

## Recommendations for Phase 2.2 (R1 implemented; R2/R3 open)

Per Plan 014 Phase 2.2's own gate ("the remaining candidates ... still need
their own audit (Task 2.1) before any moves"), this audit's output feeds 2.2.
**No code changes were made in Phase 2.1 itself** — but R1 below was
subsequently implemented as the Phase 2.2 guard scope fix (see handover 114).
Recommended Phase 2.2 scope, in priority order:

### R1. Fix the Phase 2.3 guard scope (highest priority — closes the silent gap) — ✅ IMPLEMENTED

**Status: Implemented in Phase 2.2 (guard scope fix). See handover 114.**

Widened `MIRROR_FILES` from `["src/api/types.rs"]` to:
```rust
const MIRROR_FILES: &[&str] = &[
    "src/api/types.rs",
    "src/api/event.rs",
    "src/api/admin.rs",
];
```
Added the three uncovered predicates to `ALLOWED_MIRROR_PREDICATES` with
documented reasons (same `is_approved` pattern: mirror type with
`#[serde(default)]`, delegation deferred until types merge). The silent gap
is now converted into a documented decision. Live-injection re-verification
extended across all 3 files in scope — guard fires correctly on each. Audit
baseline self-test updated to assert `mirror_predicates.len() == 4`.
Frontend tests: 159 passing (unchanged). Workspace tests: 308 passing
(unchanged). Zero new clippy warnings on the test file.

### R2. Eliminate the two `DepositMethod` serialization sites (worker) — ✅ IMPLEMENTED

**Status: Implemented in Phase 2.2 (worker DepositMethod SSOT). See handover 115.**

Domain `DepositMethod` gained a `FromStr` impl (inverse of the existing
`Display`, with `type Err = String` and error format
`unknown DepositMethod: '{other}'` preserved verbatim from the prior
worker-side message so logs, e2e scripts, and any error-display code see
no behavior change). Worker `db/deposit_statuses.rs` replaced its 4-arm
string→enum match with `DepositMethod::from_str(&method_str)?`; worker
`handlers/attendee.rs` replaced its 4-arm enum→string match with
`d.method.to_string()` (delegating to domain's `Display`).

Three new domain tests pin the wire strings and the error format:
`test_deposit_method_from_str_round_trip` (Display→FromStr identity for
every variant), `test_deposit_method_from_str_wire_strings` (exact
snake_case strings, both directions), and
`test_deposit_method_from_str_rejects_unknown_with_canonical_message`
(pins the `unknown DepositMethod: '{other}'` format that the worker now
propagates directly via `?`).

Verification: workspace tests 311 (was 308, +3 from the new pinning
tests); frontend tests 159 (unchanged — R2 does not touch frontend);
zero new clippy warnings on domain or worker.

### R3. Decide on the EventFormat/EscrowStatus mirror types (larger scope)

The substantive question Phase 2.2 must answer: should `EventFormat`,
`EscrowStatus`, `EventStatus`, `OnlineOpenMode` mirror types in
`api/event.rs` be (a) kept as documented mirrors, (b) replaced by direct
imports from domain, or (c) replaced by delegation wrappers? This is the
same decision Phase 2.3 deferred for `CheckInStatus`. Options (b)/(c)
eliminate the divergence risk entirely but require domain to be
WASM-compatible (it already is — Plan 014 Phase 1 confirmed this) and may
require domain to grow `#[serde(default)]` annotations for partial-JSON
safety.

**Recommendation:** Defer the substantive type-merge decision to a dedicated
session. R1 (widening the guard + documenting) is the immediate, low-risk
action that preserves the audit-first discipline.

### R4. NOT recommended

- Do NOT add `DepositMethod::is_usdc()` to domain to "consolidate" the 9
  equality checks. They are type-safe already; the helper would add API
  surface for marginal readability gain. Smell, not bug.
- Do NOT migrate frontend `label()` / `icon_name()` to domain. UI
  presentation is legitimately frontend.

---

## What This Audit Deliberately Refuses to Claim

1. **Inline re-implementations.** Patterns like
   `let is_checked_in = x.checked_in_at.is_some();` (frontend) inline a
   domain predicate without naming it. Catching these requires semantic
   analysis, not text scanning. Out of scope (documented in Phase 2.3 guard).
2. **Serde shape compatibility.** This audit did not verify that frontend
   mirror types deserialize the same JSON the worker emits. That is a
   contract-test concern, partially covered by
   `frontend-leptos/tests/serde_contract.rs`. Out of scope here.
3. **Worker-internal duplication.** Plan 014's SSOT objective is
   cross-crate (`worker` ↔ `frontend-leptos`). Worker-internal repetition
   (e.g. the 9 `method == DepositMethod::Usdc` sites) is noted but not the
   focus of this objective.

---

## Audit Method (reproducibility)

Every claim above is grounded in a `rg` search with file:line evidence:

- **Participation-type normalization**: `rg "normalize_override"` (15 matches,
  all in worker + plan doc); `rg "participation_type|ParticipationType"` in
  `frontend-leptos/src/` (frontend carries raw string, no parse).
- **DepositMethod mapping**: `rg "DepositMethod::(Usdc|Thb|CreditThb|CreditUsdc)"`
  in worker (18 matches, classified into serialization vs. equality).
- **Escrow/EventFormat predicates**: `rg "impl EscrowStatus|pub fn (is_|can_|has_|should_|requires_|allows_)"`
  in `domain/src/models/event.rs`; cross-referenced with
  `frontend-leptos/src/api/event.rs` lines 33-83.
- **Mirror-file inventory**: `rg "mirrors backend|Mirrors domain"` across
  `frontend-leptos/src/api/` (6 matches across `api/event.rs` and `api/admin.rs`).
- **Phase 2.3 guard scope**: read
  `frontend-leptos/tests/ssot_mirror_audit.rs:88` (`MIRROR_FILES` constant)
  and lines 40-44 (doc comment admitting the scope assumption).
- **Domain predicate count**: `rg "pub fn (is_|can_|has_|should_|requires_|allows_)[a-z_]+\s*\(\s*&self"`
  in `domain/src/` → 18 distinct methods.

---

## Conclusion

The 7th Plan 014 audit again found the plan's premises did not match the
codebase — but this time the audit produced a **real, actionable finding**:
the Phase 2.3 guard's scope was too narrow, leaving three load-bearing
mirrored business predicates uncovered in `api/event.rs`. The remediation
(R1: widen `MIRROR_FILES` + document the three predicates in the allowlist)
is small, low-risk, and preserves the forward-looking regression-guard
discipline. The two `DepositMethod` serialization sites (R2) are genuine,
removable, zero-behavior-change refactors for Phase 2.2.

**Phase 2.1 is complete.** No code changes were made during Phase 2.1 itself;
the output was this document. **R1 has since been implemented** as the
Phase 2.2 guard scope fix (see handover 114) — `MIRROR_FILES` widened,
3 predicates added to the allowlist, live-injection re-verification extended
across all 3 files in scope, audit baseline self-test updated to expect 4
mirror predicates. **R2 has since been implemented** as the Phase 2.2 worker
`DepositMethod` SSOT refactor (see handover 115) — domain `DepositMethod`
gained a `FromStr` impl (inverse of `Display`, error format preserved
verbatim); the two worker serialization sites in `handlers/attendee.rs` and
`db/deposit_statuses.rs` were replaced by `to_string()` and `from_str()?`
respectively; 3 new domain tests pin the wire strings and error format;
workspace tests 311 (was 308, +3), frontend 159 (unchanged). Only R3
(substantive EventFormat/EscrowStatus type-merge decision) remains open and
is not required to close the guard gap.