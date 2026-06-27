# Plan 014 — katgpt-rs Paradigm Migration Assessment (Zero-Copy / WASM VM / Neuro-Symbolic / SIMD)

> Architectural assessment of migrating `event-checkin` toward the `katgpt-rs`
> (Transformer VM & Latent Space) paradigm. Requested as a "GOAT" plan with four
> objectives: (1) eliminate JSON bottlenecks via zero-copy, (2) WASM/Transformer-VM
> integration, (3) neuro-symbolic policies, (4) SIMD for green software.
>
> **Honesty preamble.** `katgpt-rs` is a micro-transformer *inference engine*
> (vocab=27, n_embd=16, n_layer=1, 359 feature flags) — its architecture optimizes
> *compute-bound* token generation (matmul, attention, KV cache, speculative decode).
> `event-checkin` is an *I/O-bound* CRUD + on-chain escrow system (KV / D1 / R2 /
> Google Sheets / Solana RPC). The two workloads live in different performance
> regimes. A top-tier architect's job is to say which katgpt-rs ideas transfer and
> which would be cargo-cult over-engineering. This plan does both.

## Context (verified against the codebase)

- **Workspace**: `Cargo.toml` declares `members = ["domain", "worker"]`, with
  `frontend-leptos` excluded but built from the same Rust toolchain.
- **`domain` crate** already compiles `x86_64 + wasm32` — the *single source of
  truth* foundation katgpt-rs advocates for already partially exists.
- **Serialization hotspot**: handlers return `ApiOk<serde_json::Value>` (see
  `worker/src/handlers/attendee.rs` — 14+ `json!({...})` sites, `serde_json::from_str`
  for KV-stored lock payloads). Frontend decodes via `web_sys::Response::text().await`
  → `serde_json::from_str` (`frontend-leptos/src/api/fetch.rs`, 8+ api modules).
- **Concurrency already optimized**: `futures-util::join!` for parallel KV reads,
  KV caches for Google OAuth token (3500s), attendees (30s), Solana blockhash (30s).
- **Performance regime**: all measured savings in the README's Performance Layers
  table are *latency-from-I/O-elimination* (100–800ms saved per cache hit). There is
  **zero** CPU-bound hot loop anywhere in the worker.

## Objective-by-objective verdict

| # | Objective | katgpt-rs idea | Applicability to event-checkin | Action |
|---|-----------|----------------|-------------------------------|--------|
| 1 | Zero-copy wire | `bytemuck`, flat `[f32]` layouts, "flat variants 1.8–5.1× faster" | **High** for Worker↔Leptos batch paths | **DO** (scoped) |
| 2 | WASM Transformer VM | Percepta: WASM interpreter in transformer weights | **None** — no transformer in the workload | **DON'T** (reframe to SSOT) |
| 3 | Neuro-symbolic policies | `ConstraintPruner` / `SpeculativeGenerator` functors | **Low** — rules are deterministic FSMs | **DON'T** (use type-state instead) |
| 4 | SIMD green software | `simd_sigmoid`, `simd_dot_f32`, NEON/AVX2 kernels | **None** — no SIMD-shaped workload | **DON'T** (invest in I/O instead) |

The detailed task lists below turn each verdict into concrete, scoped work.

---

## Phase 1 — Zero-Copy Wire Protocol (Objective 1, the one that actually fits)

**Why this transfers:** katgpt-rs's "MUX-Latent Wire Patch" (Plan 243) and its
general preference for flat `&[f32]` over `Vec<Vec<f32>>` (1.8–5.1× faster, per
the Sink-Aware Attention benchmark) is just *good systems Rust*. The same logic
applies to our batch attendee lists, quiz state, and escrow instruction payloads.

**Why it does NOT transfer everywhere:** Google Sheets API, Solana RPC, and D1 are
externally JSON-bound. Zero-copy is **only** a win on the internal Worker↔Leptos
path where we control both ends and the `domain` crate already compiles to both
targets.

### Tasks

> **Phase 1 status:** CONCLUDED. See `.plans/014_wire_audit.md` → "Phase 1 —
> CONCLUDED" and `.plans/014_negative_results.md` for the full record. The
> GOAT-gate (1.7) cleared at the format level; the production rollout
> (1.4–1.6) was demoted after the audit nominated types that turned out to
> have variable-length strings. Acceptance criterion below is satisfied via
> the "OR Phase 1 is documented as a negative result and demoted" clause —
> specifically the production-rollout half is the demoted part.

- [x] **1.1 Audit candidate endpoints.** Inventory Worker↔Leptos endpoints by
  payload shape. Tag as `scalar` (claim status, single attendee), `small-struct`
  (event config, ticket), or `batch` (attendee list, quiz results, adventure
  progress). Zero-copy only pays off for `batch`. Produce table in
  `.plans/014_wire_audit.md`.
- [x] **1.2 Add `bytemuck` to `domain` Cargo.toml** behind a `wire` feature.
  Keep `serde` as the default (no breaking change to existing JSON paths).
  *(Done as 1.2a + 1.2b — Pod derives + round-trip tests.)*
- [x] **1.3 Define the shared wire envelope** in `domain/src/wire.rs` with a
  BLAKE3 commitment + version tag (katgpt-rs pattern: every committed blob has
  a version tag in its BLAKE3 input). Add a `Content-Type:
  application/x-bethere-bin` smoke route (`GET /api/wire-sample/level-score?fmt=bin`)
  to prove the path end-to-end. JSON stays the default — the smoke endpoint is
  an opt-in seam, not a production rollout.
- [x] **1.4 — DEMOTED.** Originally "Add a parallel `?fmt=bin` route for the
  audited batch endpoints (EventMeta list)." Demoted: every nominated
  production type has variable-length strings that defeat pure Pod. See
  `.plans/014_negative_results.md` entry 4.
- [x] **1.5 — DEFERRED.** Originally "client-side decoder in
  `frontend-leptos/src/api/fetch.rs`." The decoder *helper* shipped as part of
  1.3 (`response_array_buffer` + `get_wire_sample_level_score`); the production
  *rollout* (wiring it to a real endpoint) is deferred with 1.4. Revisit when a
  per-check-in CPU hotspot is profiled (likely `DepositStatusWire` on the KV
  hot path — see audit's Blocker B).
- [x] **1.6 — DEMOTED with 1.4.** Originally "BLAKE3 commitment for KV cache
  invalidation." The commitment primitive is already in the envelope
  (`domain::wire::pack` hashes header + payload); the KV-cache *application*
  (`cached_get_bin`) was demoted because no production type needs it.
- [x] **1.7 Benchmark: `criterion` microbench** comparing JSON vs bin for a
  500-row payload. Target: ≥3× decode-speed improvement AND ≥40% payload-size
  reduction. **PASSED** — 6.2× decode, 73.5% size reduction at 500 rows.
  See `.plans/014_wire_audit.md` → "Task 1.7 — GOAT-gate result (CLEAR)".
- [x] **1.8 Rollout gate: RESOLVED — stay opt-in.** The `wire` feature stays
  default-off. 1.7 passed but 1.4–1.6 demoted; no production type justifies
  default-on. Future types get added per-PR behind `--features wire` when
  their specific cost/benefit clears the bar.

**Non-goals (explicitly):**
- Do NOT replace JSON for external integrations (Sheets, Solana, D1) — they are
  JSON-only by protocol.
- Do NOT convert scalar endpoints — the per-request overhead of a versioned bin
  header exceeds the savings.
- Do NOT pursue this for endpoints called <10×/event — micro-optimization.

---

## Phase 2 — Single Source of Truth (Objective 2, honestly reframed)

**The honest reframe:** katgpt-rs's "Transformer VM" / Percepta (WASM interpreter
embedded in transformer weights) has **no application** here — we have no
transformer. But the underlying *principle* — pure logic lives in one crate that
compiles to every target — is exactly what `domain/` already half-does. The
honest version of Objective 2 is: **finish the SSOT migration, don't build a VM.**

### Tasks

- [ ] **2.1 Audit logic duplicated across `worker/src/` and `frontend-leptos/src/`.**
  Candidates: participation-type normalization (worker has `normalize_override`,
  leptos likely re-implements display logic), deposit-status enum mapping (worker
  matches on `DepositMethod` → string in 3+ places), escrow-state predicates.
  Produce list in `.plans/014_ssot_audit.md`.
- [ ] **2.2 Move all duplicated business predicates into `domain/src/policy/`.**
  These are deterministic functions over typed models — the *real* equivalent of
  katgpt-rs's pure-algorithm crates (`katgpt-core`). No traits-for-the-sake-of-traits;
  just `pub fn is_refundable(d: &Deposit, now_ms: i64, event_end_ms: i64) -> bool`.
  **Note (Phase 3.1 audit):** the deposit/refund predicate subset turned out to
  be a deliberate two-stage guard, not duplication — see negative-results entry #8.
  The scope of *what to move* is therefore smaller than this task assumed; the
  remaining candidates (participation-type normalization, deposit-status enum
  mapping) still need their own audit (Task 2.1) before any moves.
- [ ] **2.3 Compile-gate both crates against `domain`.** Add a CI check that
  greps `worker/src/` and `frontend-leptos/src/` for re-implementations of any
  function exported from `domain::policy`. This is the structural enforcement
  katgpt-rs gets from `katgpt-core` being the only place SIMD kernels live.
  **Note:** the check's value is **forward-looking** (catching *future*
  duplication). The Phase 3.1 audit showed the retroactive remediation list for
  deposit/refund predicates is near-empty — the existing "duplication" there is
  intentional (entry #8). Build the detector; don't expect a big cleanup payoff.
- [ ] **2.4 Type-state the escrow lifecycle** (`Created → DepositOpen →
  CheckedIn → Refundable → Claimable → Closed`). This is the *legitimate* sibling
  of katgpt-rs's `ConstraintPruner` trait — a compile-time FSM that makes invalid
  state transitions not compile. Limit to escrow (the one place state-machine
  correctness has monetary consequences).
- [ ] **2.5 Do NOT compile business logic to a standalone WASM module.** The
  worker already runs as WASM; the leptos client already runs as WASM; both link
  `domain`. A separate "logic WASM" would add a third WASM artifact with zero
  benefit. Document this decision in `.plans/014_no_transformer_vm.md`.

**Non-goals (explicitly):**
- No Transformer VM. No WASM-in-weights. No Percepta-style interpreter. These
  solve problems we don't have.

---

## Phase 3 — Type-State Policies (Objective 3, honestly reframed)

**The honest reframe:** katgpt-rs's neuro-symbolic functors
(`ConstraintPruner::is_valid(depth, token_idx, parent_tokens)`,
`SpeculativeGenerator`, `DominoPruner`, `CollapseDetector`) exist because LLM
decoding is *probabilistic* — you need to filter a distribution at each token.
Our policies are *deterministic* (`if role != SuperAdmin { deny }`). There is no
distribution to prune. Forcing functor/graph machinery onto deterministic rules
is the textbook definition of cargo-cult architecture.

The legitimate kernel of Objective 3 is: **where rules genuinely vary per-event
or per-org, parameterize them with traits — not graphs.**

### Tasks

- [x] **3.1 Inventory the conditional rules in `worker/src/`.** Categorize each
  as `universal` (same for all events — e.g. "only organizer can check in") vs
  `parameterized` (varies — e.g. deposit amount, refund deadline offset,
  walk-in policy). Produce table in `.plans/014_policy_audit.md`.
  **✅ DONE → `.plans/014_policy_audit.md`. Finding: no behavioral polymorphism;
  all "parameterized" rules are universal formulas reading per-event data fields
  on `EventConfig`. The real win is SSOT consolidation (Phase 2.3), not traits.**
- [ ] ~~**3.2 For `parameterized` rules only**, define a `Policy` trait in
  `domain/src/policy/mod.rs`~~ **❌ DEMOTED — negative-results entry #7.** The
  audit (3.1) found the trait would wrap a single formula in polymorphism:
  every event uses the same `is_refund_eligible` / `accepts_usdc_deposits`
  formula, differing only in field values. Inherent methods on `EventConfig`
  already provide the parameterization. Role-based access is already type-state
  via `UserRole`'s `Ord` derive. No per-org policy layer exists. The genuine
  value (14 duplicated `if !event.deposit_enabled` sites + 5 refund-deadline
  recomputations) is an SSOT violation folded into Phase 2.3.
- [x] **3.3 Do NOT introduce latent-space embeddings, sigmoid gates, or graph
  functors for policy decisions.** Document why in `.plans/014_no_neuro_symbolic.md`:
  our policies are decidable in O(1) with no uncertainty; adding a learned
  component would (a) require a training pipeline we don't have, (b) make
  monetary decisions non-auditable, (c) violate the deterministic-validator
  principle that makes the escrow flow trustworthy.
  **✅ DONE — covered by negative-results entry #2 (no separate doc needed; the
  deterministic discipline is also encoded in Phase 5.3).**
- [ ] **3.4 Reuse the audit log as the "trace".** Where katgpt-rs uses PTG
  (Primitive Transition Graph) traces for `closure_instrument`, our equivalent
  is the existing `audit_store.rs` (27 action types, append-only). Verify every
  policy decision path emits an audit entry — that's our deterministic
  equivalent of "every pruner decision is observable".
  **DEFERRED — partially covered by the D1/D2 duplication findings in
  `.plans/014_policy_audit.md`; full audit-coverage verification folded into
  Phase 2.3 scope when the SSOT consolidation touches policy call sites.**

**Non-goals (explicitly):**
- No Functors/Graphs/Embeddings for business rules.
- No "latent-space reasoning" over event state.
- No `ConstraintPruner`-style search tree — there is nothing to search.

---

## Phase 4 — Honest SIMD / Green-Software Assessment (Objective 4)

**The honest reframe:** katgpt-rs's SIMD work (`simd_sigmoid`, `simd_dot_f32`,
OCT+PQ KV codec, PlasmaPath bit-plane ternary) accelerates *dense f32 math over
fixed-size arrays* — matmuls, attention scores, KV rotations. **`event-checkin`
has no such workload.** The hot paths are:

1. KV reads (network-bound, not CPU)
2. Solana RPC calls (network-bound)
3. Google Sheets API (network-bound)
4. JSON (de)serialization (branchy, string-heavy — *anti*-SIMD)
5. Ed25519 / HMAC (single-shot, already optimized in `crypto.rs`)

SIMD would provide **measurably zero** benefit. The "green software" win here is
*eliminating wasted I/O*, not vectorizing arithmetic.

### Tasks

- [ ] **4.1 Profile before optimizing.** Run `wrangler tail` against a staged
  event with 200 attendees through a full check-in + claim flow. Capture p50/p99
  per endpoint. Confirm (or refute) the I/O-bound hypothesis with data, not
  assertion. Output → `.plans/014_profile.md`.
- [ ] **4.2 If 4.1 reveals a CPU-bound hot spot** (unlikely), measure it with
  `cargo flamegraph` on the x86_64 target before considering SIMD. Only
  vectorize a loop that is (a) >5% of total CPU and (b) over contiguous f32/u64.
  Do not SIMD-ify string/JSON/branchy code — it will be slower (katgpt-rs's own
  "DFlare Progressive Budget: GOAT FAILED" is the cautionary tale).
- [x] **4.3 Real green-software wins to pursue instead** (these are the
  I/O-bound equivalents of katgpt-rs's "skip dead compute" philosophy):
  *(Phase 4.3 audit concluded 2026-06-27 — see per-task notes. 2 of 4 were
  already satisfied by existing code; 1 shipped; 1 demoted as unsafe.)*
  - [x] **4.3.1** KV cache the public event-series endpoint (Plan 013) at 120s —
    **ALREADY SATISFIED (no code change needed).** The audit found the endpoint
    is already server-cached: `worker/src/handlers/mod.rs:50-57` registers
    `/public/event-series/{event_id}` under the `public_events_detail` sub-router,
    which applies `cache_public_120_layer` — exactly the 120s cache this task
    asked for. The plan's "currently uncached" premise was written before Plan
    013 (issue #060) shipped the endpoint with the cache layer attached.
  - [x] **4.3.2** Collapse the 3 sequential KV reads in `get_public_ticket`
    (event → attendee → lock) into `join!`. Each sequential read is ~5–50ms;
    parallelizing is a pure latency win with zero compute cost.
    **SHIPPED (commit `c6f89d2`) — scope corrected by audit.** The plan's
    "3 sequential reads" framing was overstated: `event`→`attendee` is a
    dependency chain (attendee needs `event.sheet_id`) and cannot be
    parallelized. Only the two post-attendee reads are genuinely independent:
    `get_deposit_status_with_fallback` (USDC) + `get_thb_deposit_with_fallback`
    (THB). Those are now `join!`'d in `worker/src/handlers/attendee.rs`,
    following the established pattern at `worker/src/handlers/deposit/escrow/
    status.rs:88`. Two sequential D1/KV round-trips collapse to one concurrent
    step; zero behavior change.
  - [x] **4.3.3** Batch the quiz/adventure KV writes — currently one PUT per
    answer; batch into a single PUT per submit.
    **ALREADY SATISFIED (no code change needed).** The audit found the
    per-answer-write anti-pattern does not exist: `worker/src/quiz.rs:356`
    (`submit_quiz`) grades all answers in-memory then calls
    `save_quiz_progress` **once**. Adventure is the same — one write per
    `/adventure/{token}/save` request (one per level completion, the natural
    granularity). The plan's "one PUT per answer" premise was wrong.
  - [ ] **4.3.4 — DEMOTED (unsafe).** Promote the Solana blockhash cache TTL
    from 30s to the ~90s effective lifetime (blockhash valid ~120s) — halves
    RPC calls.
    **DEMOTED after audit.** The plan's "blockhash valid ~120s" premise is
    factually wrong — it confuses `MAX_HASH_AGE_IN_SECONDS=120` (sizes the
    recent-blockhash ring buffer) with `MAX_PROCESSING_AGE=150 blocks` (the
    actual transaction validity limit, from `solana-sdk/clock/src/lib.rs`).
    Real validity is **~60–90s wall-clock**. BeThere also fetches with
    `"commitment": "finalized"`, which Solana docs note *"effectively reduces
    the expiration of your transactions by about 13 seconds."* A 90s cache TTL
    would let the worker hand the frontend a blockhash up to ~90s (cache) +
    ~13s (finalized staleness) old — past the 150-block window, causing
    intermittent "Blockhash not found" / "block height exceeded" failures.
    The current 30s is the correct, defensible value. Full reasoning in
    `.plans/014_negative_results.md` entry #9.
- [ ] **4.4 Do NOT add `wide`, `pulp`, or `std::simd` dependencies.** Document
  the decision in `.plans/014_no_simd.md` with the profile evidence from 4.1.

**Non-goals (explicitly):**
- No SIMD matmul/attention/sigmoid kernels. We have no matrices.
- No "PlasmaPath"-style bit-plane ternary encoding. We have no weights.

---

## Phase 5 — Cross-cutting: katgpt-rs engineering disciplines that DO transfer

These are the *process* and *discipline* patterns from katgpt-rs that apply
regardless of workload. They are the highest-ROI part of this plan.

### Tasks

- [ ] **5.1 GOAT-gate every perf claim.** Adopt katgpt-rs's discipline: any
  optimization (Phase 1 zero-copy, Phase 4.3 cache wins) must have a measurable
  target gate (e.g. "≥3× decode speedup") and a documented "GOAT FAILED → demote"
  outcome if it misses. No "it feels faster".
- [ ] **5.2 Negative-results log.** Create `.plans/014_negative_results.md`
  modeled on katgpt-rs's `.docs/20_negative_results.md`. Record every demoted
  idea (Transformer VM, neuro-symbolic policies, SIMD) with the *reason*. Future
  agents/readers save time by not re-proposing them.
- [x] **5.3 Sigmoid-not-softmax discipline → deterministic-not-stochastic.**
  katgpt-rs has a hard rule "sigmoid, never softmax" for sound mathematical
  reasons. Our equivalent hard rule for monetary code: **deterministic, never
  stochastic**. No RNG in policy decisions, no probabilistic gates on refunds.
  Encode as a clippy lint or `#[deny]`-style test where feasible.
  **DONE** — custom clippy lints require nightly, so the discipline is encoded
  as a multi-layer regression guard in `worker/tests/deterministic_monetary_code.rs`
  (23 tests). Audit-first pass confirmed the codebase is already compliant:
  no `rand` / `fastrand` / `getrandom` / `rand_core` dependency anywhere in
  the workspace; every monetary decision path (refund verify, claim lock
  acquisition, escrow status, deposit verify, THB slip verify) already uses
  purely deterministic business rules. The guard then locks that state in:
  (1) **dependency layer** — asserts `worker/Cargo.toml` declares no direct
  RNG crate (`rand`, `fastrand`, `getrandom`, `rand_core`, `rand_chacha`,
  `rand_pcg`, `rand_xorshift`); (2) **source-scan layer** — recursively
  scans every `.rs` file under the monetary module tree
  (`claim/`, `solana_escrow/`, `escrow_indexer/`, `handlers/deposit/`, plus
  explicit `claim.rs`, `escrow_index.rs`, `checkin.rs`, `register.rs`,
  `walkin.rs`, `wallet.rs`) for forbidden direct RNG patterns
  (`rand::thread_rng`, `OsRng`, `StdRng`, `ChaCha*Rng`, `RngCore`,
  `Math::random`, `getRandomValues`, `get_random_values`, `gen_range`,
  `fill_bytes`); (3) **scope-sanity layer** — catches the silent-regression
  case where a refactor moves monetary code out of the scanned tree. 20
  self-tests prove the patterns catch real RNG introductions (rand crate,
  OsRng, ChaCha20Rng, JS `Math::random`, Web Crypto `getRandomValues`,
  `RngCore` trait) while rejecting deterministic lookalikes
  (`Uuid::now_v7`, `chrono::Utc::now`, BLAKE3, SHA-256, FNV-1a,
  deterministic shuffles). A live injection test confirmed the guard fires
  with a clear file:line message when a forbidden pattern is introduced.
  `Uuid::now_v7()` is explicitly allowed — it generates identifiers, not
  decisions; its random tail is collision-avoidance for same-millisecond
  UUIDs, never a policy input.
- [x] **5.4 Zero-allocation hot-path audits.** katgpt-rs measures allocs/call on
  every hot kernel. Apply the same to the zero-copy decode path from Phase 1
  (`#[cfg(feature = "alloc_count")]` + a test asserting 0 allocs after warmup).
  **DONE** — `alloc_count` feature added to `domain/Cargo.toml`;
  `domain/tests/alloc_count.rs` installs a counting global allocator and
  asserts 0 allocs after warmup. Audit measured 10 shapes (single value,
  empty/1/3/4-row slice boundary, 50/500/10000-row slice, 100× repeats)
  on blake3 1.8.5: **every shape is 0 allocs in steady state**, including
  the 10000-row / 160 KB stress test. blake3's `Hasher` uses
  `InlineSubCtxStack` (fixed-size stack array, not a `Vec`), and
  `bytemuck::cast_slice` / `from_bytes` are pointer reinterprets. The
  decode path is genuinely zero-alloc. Corrected the `unpack_slice` doc
  comment that previously overstated allocations as "BLAKE3 hasher's
  internal scratch" — there are none.
- [ ] **5.5 Feature-flag every Phase 1–4 change.** Every optimization ships
  behind a Cargo feature (`wire`, `policy-traits`, `batch-kv-writes`) and a
  runtime config flag. Default-off until GOAT-gated; default-on only after
  proof. This is how katgpt-rs keeps 359 flags manageable.

---

## Sequencing & Dependencies

```
Phase 5.1 (GOAT discipline) ──┐
                              ├─► Phase 1 (zero-copy wire) ──► 1.8 rollout gate   [CONCLUDED]
Phase 2.1 (SSOT audit) ───────┤
                              ├─► Phase 2 (SSOT migration) ──► 2.4 type-state escrow
Phase 3.1 (policy audit) ─────┤   [CONCLUDED — Phase 3.2 trait demoted; value → Phase 2.3]
                              ├─► Phase 3 (policy traits, scoped)                 [CONCLUDED]
Phase 4.1 (profile) ──────────┴─► Phase 4 (I/O wins, conditional on profile)

Phase 5.2 (negative log) ← updated throughout
```

- **Phase 1** delivers a measurable performance win aligned with the user's
  framing — format infra ships opt-in; production rollout demoted. **[CONCLUDED]**
- **Phase 2** is the highest *correctness* win (SSOT reduces bugs). The Phase
  3.1 audit confirmed its highest-value target: consolidate the 14 duplicated
  `if !event.deposit_enabled` sites and 5 refund-deadline recomputations.
- **Phase 3** is a *don't-do-this* exercise — the trait was demoted too.
  **[CONCLUDED]**
- **Phase 4** is almost entirely a *don't-do-this* — with 4 concrete I/O wins
  that *are* worth doing under the "green software" banner.
- **Phase 5** is process discipline that makes 1–4 trustworthy.

## Acceptance Criteria

- [x] **Phase 1.7 benchmark passes its GOAT gate (≥3× decode, ≥40% size) OR
  Phase 1 is documented as a negative result and demoted.**
  **SATISFIED (both clauses).** 1.7 cleared the gate at every row count
  (smallest: 4.7× decode, 71.8% size reduction at 50 rows). The production
  rollout (Tasks 1.4–1.6) is additionally documented as demoted in
  `.plans/014_negative_results.md` — every nominated production type has
  variable-length strings that defeat pure Pod. The format infrastructure
  ships as opt-in (`domain/src/wire.rs`, feature `wire`).
- [ ] Phase 2.3 CI check passes — no business predicate is duplicated across
  `worker/` and `frontend-leptos/`.
  *(Scope clarified by Phase 3.1 audit: the check is forward-looking; the
  deposit/refund predicates were investigated and found to be a deliberate
  two-stage guard, not duplication — see negative-results entry #8. Task 2.1's
  full cross-crate audit of the remaining candidates is still pending.)*
- [ ] Phase 3.3 and 4.4 negative-result docs exist and are linked from this plan.
  *(Phase 5.2 negative-results log created early as `.plans/014_negative_results.md`
  — covers Phase 1 demotions + the original Phase 3/4 reframes. Phase 3.3's
  conclusion is folded into negative-results entry #2 (no separate doc needed);
  Phase 3.2's trait demotion is entry #7; Phase 4.4's SIMD doc still pending
  Phase 4.1 profile data.)*
- [ ] Phase 4.1 profile data is captured and cited in every Phase 4 decision.
- [ ] Phase 5.2 negative-results log has entries for: Transformer VM,
  neuro-symbolic policies, SIMD — each with the reason.
  *(DONE — `.plans/014_negative_results.md` entries 1, 2, 3 cover these. Entry 7
  additionally covers the Phase 3.2 trait demotion.)*

## What this plan deliberately refuses to do

To honor the user's personal rule "Make only essential changes. Don't fix
unrelated bugs unless asked" and the global rule "Don't lie. Be honest. Don't
overclaim":

1. **No Transformer VM.** We have no transformer. Building one to host
   deterministic CRUD logic would be the definition of over-engineering.
2. **No neuro-symbolic policy graphs.** Our policies are decidable in O(1).
   Adding learned components to monetary decisions would violate auditability.
3. **No SIMD kernels.** The workload is I/O-bound; profile data will confirm.
4. **No rewrites.** Every change ships behind a feature flag, JSON stays the
   default, existing endpoints keep working. katgpt-rs's "opt-in seam" discipline.

The single genuinely-transferable katgpt-rs idea for this codebase is
**zero-copy flat wire structs with BLAKE3 commitments for the batch Worker↔Leptos
path** — Phase 1. Everything else is either a reframing (Phase 2: SSOT, Phase 3:
type-state) or an honest refusal (Phase 4: no SIMD).
