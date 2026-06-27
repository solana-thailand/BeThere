# Plan 014 — Negative Results Log (Phase 5.2)

> Modeled on katgpt-rs's `.docs/20_negative_results.md`. Every demoted idea is
> recorded here with the *reason*, so future agents/readers save time by not
> re-proposing them. This is the katgpt-rs "GOAT FAILED → demote" discipline
> made durable.
>
> **Rule for entries:** a demotion is only final until the *preconditions that
> killed it* change. Each entry lists those preconditions explicitly. If they
> change, re-open the idea — don't cargo-cult the no.

---

## 1. Transformer VM (Plan 014 Objective 2, original framing)

**Status:** Demoted at plan-creation time. Not re-evaluated.

**Original framing:** Port katgpt-rs's micro-Transformer VM to host event-checkin
business logic.

**Why demoted:** `event-checkin` has no transformer. It has deterministic CRUD
+ escrow flows. Building a probabilistic VM to host deterministic logic is the
textbook definition of over-engineering. There is nothing to inference.

**Preconditions that would re-open it:** A genuine ML inference workload is
added to the codebase (recommendation engine, fraud scoring). Not on any
current roadmap.

---

## 2. Neuro-symbolic policy graphs (Plan 014 Phase 3, original framing)

**Status:** Demoted at plan-creation time. Phase 3 itself was reframed to
*trait-based* policies (deterministic analog of `ConstraintPruner`); the
graph/learned variant is what's demoted.

**Original framing:** Use katgpt-rs's `ConstraintPruner`/`SpeculativeGenerator`/
`DominoPruner`-style functor graphs for event policy decisions.

**Why demoted:** Our policies are decidable in O(1) with no uncertainty
(`if role != SuperAdmin { deny }`). Adding a learned component would:
- require a training pipeline we don't have,
- make monetary decisions non-auditable,
- violate the deterministic-validator principle that makes the escrow flow
  trustworthy.

The legitimate kernel — parameterizing per-event/per-org rules with traits —
survives as Phase 3.2's `EventPolicy` trait. The graph/learned part does not.

**Preconditions that would re-open it:** A policy decision with genuine
uncertainty (e.g. probabilistic fraud detection on THB slips). Not on any
current roadmap.

---

## 3. SIMD kernels (Plan 014 Phase 4, original framing)

**Status:** Demoted at plan-creation time. Phase 4 itself was reframed to
*I/O-bound "green software" wins*; the SIMD part is what's demoted.

**Original framing:** Port katgpt-rs's `simd_sigmoid`, `simd_dot_f32`, OCT+PQ
KV codec, PlasmaPath bit-plane ternary to hot paths.

**Why demoted:** `event-checkin`'s hot paths are network-bound (KV, Solana RPC,
Sheets API) or branchy/string-heavy (JSON). SIMD accelerates dense `f32` math
over fixed-size arrays; we have no such workload. katgpt-rs's own
"DFlare Progressive Budget: GOAT FAILED" is the cautionary tale — vectorizing
branchy code makes it slower.

**Preconditions that would re-open it:** Phase 4.1 profile reveals a CPU-bound
hot spot that is (a) >5% of total CPU and (b) over contiguous `f32`/`u64`.
Phase 4.1 is not yet run; until it is, SIMD stays out.

---

## 4. EventMetaWire — zero-copy Pod for `GET /api/events` (Plan 014 Phase 1.4)

**Status:** Demoted after Phase 1.1 audit + Phase 1.7 GOAT-gate. **Audit was wrong.**

**Original framing:** The Phase 1.1 audit nominated `EventMetaWire` as the
"highest batch leverage" production target — ~200 B wire vs ~600 B JSON, ~1 day
effort.

**Why demoted — three layers:**

1. **Audit was incomplete.** The refactor map listed only fixed-size fields
   (slugs, IDs, timestamps, enums, counts — ~80 B total) and asserted
   "~200 B wire". It missed **8 variable-length `String` fields**: `name`,
   `tagline`, `location`, `video_url`, `nft_image_url`, `poster_url`,
   `created_at`, `organization_id`.

2. **Forced fixed arrays make it worse than JSON.** At realistic lengths,
   fixed-array Pod is **1,232 B/row vs 981 B JSON (+26%)**. Worst-case sizing
   (`[u8; 192]` for URLs, `[u8; 128]` for tagline) inflates every row
   regardless of actual content.

3. **The alternative (hybrid format) reinvents FlatBuffers.** A Pod header +
   variable string heap with offset tables loses pure zero-copy (offset
   chasing has pointer chasing), loses bytemuck's safety, and is 3–5 days of
   work for an I/O-bound codebase where JSON decode is not the bottleneck.

**Proof:** see `.plans/014_wire_audit.md` → "Phase 1 — CONCLUDED" → audit miss
log entry #3, and the byte-size table immediately below it.

**Preconditions that would re-open it:**
- The variable `String` fields are removed from `EventMeta` (split into a
  separate `EventDisplay` fetch), leaving a pure fixed-size core. Not planned.
- OR a hybrid format is justified by a measured CPU hotspot on the events
  list. Not measured (Phase 4.1 pending).

---

## 5. AttendeeListItemWire — zero-copy Pod for `GET /api/attendees` (Plan 014 Phase 1, original framing)

**Status:** Demoted in Phase 1.1 audit (the audit's own call).

**Original framing:** Original plan proposed `AttendeeListItem` as a top-3
batch target.

**Why demoted:** `AttendeeListItem` mirrors `Attendee` — 32 fields, 25
`Option<String>` Google Sheets columns. Refactor cost is enormous; per-row win
is muted because most rows are short in practice. Cost/benefit doesn't clear
the GOAT-gate relative to smaller fixed-size types.

**Preconditions that would re-open it:** Sheet schema is restructured to
separate the ~7 hot fields (status, name, deposit) from the ~25 cold sheet
columns. Not planned.

---

## 6. Quiz answer batch / Adventure checkpoint batch (Plan 014 Phase 1, original framing)

**Status:** Demoted in Phase 1.1 audit.

**Original framing:** Original plan proposed these as 2 of the top 3 batch
targets.

**Why demoted:**
- **Quiz batches:** ~5–10 questions, user-once-per-claim path, never hot.
- **Adventure checkpoints:** No batch endpoint exists.
  `AdventureProgress::scores` is a `HashMap<String, LevelScore>` shipped inline
  in a small-struct response.

Both fail the GOAT-gate's "row count ≥ 50 AND frequency ≥ once/event" filter.

**Preconditions that would re-open it:** A batch quiz-result endpoint is added
(e.g. `GET /api/events/{id}/quiz/results` returning all attendees' answers).
Not planned.

---

## 7. `EventPolicy` trait (Plan 014 Phase 3.2, trait-based reframing)

**Status:** Demoted after Phase 3.1 audit. **The reframing itself was wrong**
about where the value lived.

**Original framing:** Phase 3.2 (the surviving kernel of the demoted
neuro-symbolic Phase 3) proposed a `Policy` trait in
`domain/src/policy/mod.rs` with `DefaultPolicy` + `PaidEventPolicy` impls as
the deterministic analog of katgpt-rs's `ConstraintPruner`. The
negative-results log (entry #2) flagged this as "likely to clear its (much
lower) bar."

**Why demoted — the audit found no behavioral polymorphism:**

1. **Per-event variation is data, not behavior.** Every "parameterized" rule
   (`is_refund_eligible`, `accepts_usdc_deposits`, `deposit_deadline_passed`,
   capacities) is a *universal formula* reading per-event *data fields* off
   `EventConfig` (`domain/src/models/event.rs:551-595`). Two events differ in
   their `refund_deadline_hours` value, not in the formula they apply. A trait
   whose only impl reads the same struct fields is premature abstraction — it
   wraps one implementation in polymorphism.

2. **The correct idiom is already in place.** Inherent methods on `EventConfig`
   ARE the parameterization. `event.is_refund_eligible(now_ms)` is exactly what
   a `policy.refund_eligible(event, now_ms)` call would dispatch to, with one
   fewer indirection.

3. **Role-based access is already type-state.** `UserRole { Staff < Organizer <
   SuperAdmin }` with `derive(Ord)` + a single `check_event_access` is textbook
   type-state. A `Policy` trait would layer dispatch over an already-total
   ordering.

4. **No per-org policy layer exists.** `OrganizationConfig` is pure metadata
   (name/slug/contacts). Multi-tenancy is data scoping, not behavioral
   dispatch.

**The real value (folded into Phase 2.3):** The audit surfaced that the domain
methods *exist* but are *ignored* at the call sites — `if !event.deposit_enabled`
is reimplemented inline 14 times in the worker (weaker than the domain's
`accepts_usdc_deposits()`, which also checks escrow state), and the refund-
deadline formula is recomputed in 5 places with mixed `3600`/`3_600_000` units.
That is an **SSOT violation** (Phase 2.3), not a missing trait (Phase 3.2).

**Proof:** `.plans/014_policy_audit.md` — full inventory, the 14-site + 5-site
duplication tables, and the per-task verdict.

**Preconditions that would re-open it:**
- A new event class with a structurally different *formula* (not just different
  field values) — e.g. free events allow walk-in, paid events require deposit
  *and* ID verification. Not on roadmap.
- Per-org policy overrides added to `OrganizationConfig`. Not on roadmap.
- A third-party plugin/extension model for policy (would also need a sandbox
  story for monetary code). Not on roadmap.

---

## 8. Deposit/refund SSOT consolidation (Plan 014 Phase 2.3, as scoped by the Phase 3.1 audit)

**Status:** Demoted after a per-site code read. **The Phase 3.1 audit's D1/D2
findings were over-optimistic — caught before any code shipped.**

**Original framing:** The Phase 3.1 policy audit (`.plans/014_policy_audit.md`)
concluded that, since the `EventPolicy` trait wasn't justified, the real value
was SSOT consolidation of two "duplications":
- **D1:** 14 `if !event.deposit_enabled` sites ignoring the existing
  `accepts_usdc_deposits()` method.
- **D2:** the refund-deadline formula recomputed in 5 places with mixed
  `3600`/`3_600_000` units.

**Why demoted — both findings dissolved on per-site inspection:**

1. **D1 "asymmetry" is a deliberate two-stage guard.** Every one of the 14 sites
   uses `if !event.deposit_enabled` as the *cheap fast-fail first stage*, then a
   *specific* escrow-state check as the second stage — and the required state
   varies per endpoint (`None` for init, `Initialized` for deactivate,
   `Deactivated` for close/claim-forfeited, cross-event `||` for rollover, none
   at all for the THB flow which has no on-chain escrow). `accepts_usdc_deposits()`
   collapses to `Initialized` only, which is **wrong** for 13 of the 14 sites and
   would have **broken escrow init, close_event, claim_forfeited, the rollover
   endpoint, and the entire THB deposit flow**. It is correctly used in the one
   place (deposit-status *display*) where `Initialized` is the full gate.

2. **D2 "unit confusion" is a domain boundary.** The seconds-flavor sites
   (`escrow/handlers.rs:97`, `cleanup.rs:84`) feed Solana's on-chain program and
   KV TTL — both of which are in **seconds**. The ms-flavor sites (domain
   `is_refund_eligible`, USDC status response, frontend) are the HTTP API layer
   in **milliseconds**. Each is internally consistent. The two ms-domain
   recomputations also have *deliberately different* missing-data semantics
   (fail-safe "not eligible" vs. "return 0, hide CTA"), so a single
   `refund_deadline_ms()` method would silently change behavior.

**Net code change from the consolidation proposal: NONE.** Executing it as
written would have introduced real bugs. This is the **fourth consecutive
audit miss** in Plan 014 (after the three Phase 1 misses) and reinforces the
meta-finding: Plan 014's audits reliably over-estimate how much of the codebase
is wrong/consolidable. The GOAT-gate discipline caught it pre-merge because the
consolidation plan was itself GOAT-gated against actual call-site semantics.

**Proof:** `.plans/014_policy_audit.md` → "⚠️ CORRECTION" section — full
per-site table for D1 and the seconds/ms domain analysis for D2.

**Residual value:** the documentation that the two-stage guard pattern is
deliberate (prevents future re-proposals), and Phase 2.3's CI *check* is still
worth building as a forward-looking duplication detector — just not as a
retroactive cleanup tool for the deposit/refund predicates.

**Preconditions that would re-open it:**
- The escrow lifecycle is simplified so all mutation endpoints share one required
  state (today they span `None`/`Initialized`/`Deactivated` — by design). Not
  planned.
- THB (PromptPay) deposits gain an on-chain representation and stop being a
  separate flow. Not planned.

---

## 9. Solana blockhash cache TTL promotion 30s → 90s (Plan 014 Phase 4.3.4)

**Status:** Demoted after audit against Solana protocol constants. **The plan's
"blockhash valid ~120s" premise is factually wrong.**

**Original framing:** Promote the blockhash cache TTL in
`worker/src/solana_escrow/wire.rs` from 30s to ~90s, on the claim that
"blockhash valid ~120s" so a 90s cache "halves RPC calls."

**Why demoted — three layers:**

1. **The "~120s" premise confuses two unrelated Solana constants.** The plan
   cites `MAX_HASH_AGE_IN_SECONDS = 120`, but that constant is used to compute
   the size of the recent-blockhash ring buffer (`MAX_RECENT_BLOCKHASHES = 300`),
   *not* transaction validity. The actual transaction validity limit is
   `MAX_PROCESSING_AGE = 150` **blocks** (from `solana-sdk/clock/src/lib.rs`:
   "The maximum age of a blockhash that will be accepted by the leader"). This
   is the hard limit: transactions referencing a blockhash older than 150 blocks
   are rejected by the network.

2. **150 blocks ≠ 120 seconds.** Block time is ~400ms/slot, but blocks ≠ slots
   (~5% of slots are skipped). 150 blocks therefore takes **~60–90s wall-clock**
   in practice. Empirical observations across the ecosystem confirm this:
   Flash Trade docs say "approximately 60 seconds (~150 slots)"; Chainstack
   docs say "about 80-90 seconds"; Helius docs give "~2 minutes" as the upper
   bound. **None** support a 120s validity window.

3. **BeThere uses `"commitment": "finalized"`, which makes the problem worse.**
   Solana's transaction-confirmation guide warns that finalized commitment
   *"effectively reduces the expiration of your transactions by about 13
   seconds"* because finalized is ~32 slots behind confirmed. So the blockhash
   is **already ~13s stale at fetch time** — before the cache TTL even starts
   counting.

**The concrete failure mode if implemented as written:** A 90s cache TTL would
let the worker hand the frontend a blockhash up to ~90s (cache age) + ~13s
(finalized staleness) ≈ **103s old** — past the 150-block (~60–90s) validity
window. The frontend would then submit a transaction that the network rejects
with "Blockhash not found" / "block height exceeded." These failures are
intermittent (depend on cache hit timing) and hard to debug — exactly the
symptom class handover #086 already documented during rollover testing.

**The current 30s is the correct, defensible value.** The existing code comment
("Solana blockhashes expire after ~60s on mainnet; 30s gives a good trade-off
between RPC call reduction and freshness") is accurate and well-reasoned. A
30s cache means worst-case blockhash age at frontend submission ≈ 30s (cache) +
~13s (finalized) + ~15s (wallet signing) + ~2s (network) ≈ ~60s — comfortably
within the 150-block window.

**Proof:** `solana-sdk/clock/src/lib.rs` lines defining `MAX_PROCESSING_AGE =
150` and `MAX_HASH_AGE_IN_SECONDS = 120`; Solana transaction-confirmation guide
on finalized commitment's ~13s expiration reduction; BeThere's
`fetch_blockhash_from_rpc` uses `"commitment": "finalized"`.

**Preconditions that would re-open it:**
- BeThere switches to `"commitment": "confirmed"` (gains ~13s of validity
  headroom at the cost of ~5% blockhash-from-dropped-fork risk). Not planned.
- Solana protocol changes `MAX_PROCESSING_AGE` upward (protocol-level decision,
  not under our control).
- A durable-nonce transaction path replaces the recent-blockhash path for
  escrow flows (eliminates the staleness window entirely). Not on roadmap.

---

## Pending evaluations (not yet demoted or promoted)

These are listed for completeness — they have not been evaluated yet and may
end up in this log or in a positive-results companion.

- **Phase 2.3 SSOT CI check** — the *check* (a forward-looking lint that
  flags genuinely duplicated predicates across worker/frontend) is still worth
  building, but the *retroactive remediation* list is now known to be near-empty:
  the Phase 3.1 audit's D1/D2 consolidation proposals were demoted (entry #8) —
  the deposit-flag "duplication" is a deliberate two-stage guard, and the
  refund-deadline seconds/ms split is a Solana-vs-HTTP domain boundary. See
  `.plans/014_policy_audit.md` → "⚠️ CORRECTION".
- **Phase 3.2 `EventPolicy` trait** — **demoted** (see entry #7). The Phase 3.1
  audit found no behavioral polymorphism to trait-ify; the real value is SSOT
  consolidation, folded into Phase 2.3.
- **Phase 4.1 profile** — not yet run. Determines whether any Phase 4.3 I/O win
  is worth pursuing and is the precondition for re-evaluating SIMD. Blocked on
  a staged 200-attendee event (infrastructure coordination).
- **Phase 4.3.1–4.3.4 I/O wins** — **CONCLUDED 2026-06-27.** Audit found that
  Plan 014's premises were wrong for 3 of 4 items, mirroring the pattern of
  earlier audit misses:
  - **4.3.1 (KV cache event-series)** — already satisfied; the endpoint is
    already server-cached at 120s via `cache_public_120_layer` in
    `worker/src/handlers/mod.rs:50-57`. Plan was written before Plan 013
    shipped this. No code change.
  - **4.3.2 (parallelize `get_public_ticket`)** — shipped (commit `c6f89d2`)
    with scope corrected: the plan's "3 sequential reads" was overstated
    (event→attendee is a dependency chain); only the two independent post-
    attendee deposit reads (USDC + THB) were `join!`'d.
  - **4.3.3 (batch quiz/adventure writes)** — already satisfied; the per-answer
    write anti-pattern does not exist. `worker/src/quiz.rs:356` grades all
    answers in-memory then writes once via `save_quiz_progress`. Adventure is
    one write per `/save` request (natural granularity). No code change.
  - **4.3.4 (blockhash TTL 30s→90s)** — **DEMOTED** (see entry #9). The
    "blockhash valid ~120s" premise confuses `MAX_HASH_AGE_IN_SECONDS` with
    `MAX_PROCESSING_AGE=150 blocks` (~60–90s). A 90s cache would cause
    intermittent stale-blockhash transaction failures. The current 30s is
    correct.

---

## Reference: GOAT-gate outcomes recorded in this log

| Entry | Outcome | Reason class |
|---|---|---|
| 1. Transformer VM | Demote | No workload (over-engineering) |
| 2. Neuro-symbolic graphs | Demote (graph part only) | Deterministic code, no uncertainty |
| 3. SIMD | Demote | I/O-bound, no dense f32 math |
| 4. EventMetaWire | Demote | Audit miss — variable strings (3rd miss in a row) |
| 5. AttendeeListItemWire | Demote | Cost/benefit (audit's own call) |
| 6. Quiz/adventure batches | Demote | Wrong scope — never batched |
| 7. `EventPolicy` trait | Demote | No behavioral polymorphism — variation is data, not behavior |
| 8. Deposit/refund SSOT consolidation | Demote | Audit miss — "duplication" is a deliberate two-stage guard; unit split is a domain boundary (4th miss) |
| 9. Blockhash cache TTL 30s→90s | Demote | Plan premise factually wrong — `MAX_HASH_AGE_IN_SECONDS=120` ≠ transaction validity (`MAX_PROCESSING_AGE=150 blocks` ≈ 60–90s); 90s TTL would cause stale-blockhash failures (5th miss) |

**Positive result for contrast:** Phase 1.7 GOAT-gate on `LevelScore` cleared
both thresholds at every row count (smallest: 4.7× decode, 71.8% size reduction
at 50 rows). The format infrastructure ships as opt-in — see
`domain/src/wire.rs`. That is a positive result at the *format* level; the
*application* of the format to production types is what's demoted above.
