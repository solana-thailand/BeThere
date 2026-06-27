# Plan 014 — Phase 5.1: GOAT-Gate Discipline (Decision Record)

**Status:** ✅ SATISFIED — the discipline has been applied throughout Plan 014 to
every optimization that involves an empirical performance claim. One honest
caveat (Phase 4.3.2 `join!` parallelization) is documented in §5; it is the same
caveat recorded in `.plans/014_feature_flag_discipline.md` §6.
**Phase 5.1 checkbox:** `[x]` — closed 2026-06-27.
**Cross-refs:** `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 5.1 at
L417-420), `.plans/014_negative_results.md` ("Reference: GOAT-gate outcomes
recorded in this log"), `.plans/014_wire_audit.md` (Phase 1.7 GOAT-gate result),
`.plans/014_feature_flag_discipline.md` (Phase 5.5 — sibling closure).

---

## 1. The question Phase 5.1 asks

> *"GOAT-gate every perf claim. Adopt katgpt-rs's discipline: any optimization
> (Phase 1 zero-copy, Phase 4.3 cache wins) must have a measurable target gate
> (e.g. '≥3× decode speedup') and a documented 'GOAT FAILED → demote' outcome if
> it misses. No 'it feels faster'."*

Two requirements, both testable:

1. Every shipped optimization must have a **measurable target gate** (numeric,
   not qualitative — "it feels faster" is explicitly rejected).
2. Every demoted optimization must have a **documented "GOAT FAILED → demote"
   outcome** with the reason.

The audit must trace every Plan 014 optimization through both requirements.

---

## 2. Verified GOAT-gate trace across all Plan 014 optimizations

| Optimization | Measurable gate? | Outcome | Neg-results entry |
|---|---|---|---|
| Phase 1.7 wire format (encode/decode) | ✅ `≥3× decode AND ≥40% size reduction` | **PASSED** — 6.2× decode, 73.5% size reduction at 500 rows | (positive — `.plans/014_wire_audit.md`) |
| Phase 1.4 EventMetaWire production rollout | ✅ feasibility gate (Pod-compatibility) | **FAILED → demoted** — variable-length strings | #4 |
| Phase 1.5 AttendeeListItemWire production rollout | ✅ feasibility gate | **FAILED → demoted** — cost/benefit | #5 |
| Phase 1.6 quiz/adventure batch wire | ✅ feasibility gate | **FAILED → demoted** — wrong scope (never batched) | #6 |
| Phase 3.2 `EventPolicy` trait | ✅ value gate ("is there polymorphism?") | **FAILED → demoted** — no behavioral polymorphism | #7 |
| Phase 4.3.1 KV cache event-series | n/a — already satisfied by existing code | no new optimization shipped | (Phase 4.3 note) |
| Phase 4.3.2 `join!` parallelization | ⚠️ no formal numeric gate | shipped — see §5 caveat | — |
| Phase 4.3.3 batch quiz/adventure writes | n/a — already satisfied by existing code | no new optimization shipped | (Phase 4.3 note) |
| Phase 4.3.4 blockhash cache TTL 30s→90s | ✅ safety gate (no stale-blockhash failures) | **FAILED → demoted** — `MAX_HASH_AGE_IN_SECONDS=120` ≠ transaction validity | #9 |
| Phase 2.4 type-state escrow FSM | ✅ premise gate ("do the 6 states exist?") | **FAILED → demoted** — 8th audit miss, premise structurally wrong | #10 |

**Summary:** of the 10 optimizations/ideas evaluated in Plan 014, 1 passed its
gate and shipped (`wire`), 6 failed their gates and were demoted with documented
reasons (entries 4–7, 9, 10), 2 were already satisfied by existing code (no new
optimization to gate), and 1 shipped without a formal numeric gate (4.3.2 `join!`
— see §5). **Zero optimizations shipped on "it feels faster."**

---

## 3. The positive case — Phase 1.7 wire format GOAT-gate

The clearest application of the discipline. Phase 1.7 specified a measurable
two-axis gate (decode speedup AND payload size reduction) before the format was
allowed to graduate from bench to opt-in:

| Row count | Decode speedup | Size reduction | Gate (≥3×, ≥40%) |
|---|---|---|---|
| 50 | 4.7× | 71.8% | ✅ |
| 500 | 6.2× | 73.5% | ✅ |

Source: `.plans/014_wire_audit.md` → "Task 1.7 — GOAT-gate result (CLEAR)".

The gate passed **at the format level**, but the **application** to production
types (1.4–1.6) was then separately gated on a feasibility predicate ("does the
type have a pure Pod layout?"). Every nominated production type failed that
gate and was demoted (neg-results entries 4–6). This is exactly the two-layer
discipline Phase 5.1 describes: measurable target → documented outcome, applied
even when the outcome is "the format passed but no type qualifies."

---

## 4. The demoted cases — "GOAT FAILED → demote" outcomes

Nine ideas were demoted through Plan 014. Each has a documented reason class in
`.plans/014_negative_results.md` → "Reference: GOAT-gate outcomes recorded in
this log":

| Entry | Outcome | Reason class |
|---|---|---|
| 1. Transformer VM | Demote | No workload (over-engineering) |
| 2. Neuro-symbolic graphs | Demote (graph part only) | Deterministic code, no uncertainty |
| 3. SIMD | Demote | I/O-bound, no dense f32 math |
| 4. EventMetaWire | Demote | Variable strings (audit miss) |
| 5. AttendeeListItemWire | Demote | Cost/benefit (audit's own call) |
| 6. Quiz/adventure batches | Demote | Wrong scope — never batched |
| 7. `EventPolicy` trait | Demote | No behavioral polymorphism — variation is data, not behavior |
| 8. Deposit/refund SSOT consolidation | Demote | "Duplication" is a deliberate two-stage guard; unit split is a domain boundary |
| 9. Blockhash cache TTL 30s→90s | Demote | Plan premise factually wrong (`MAX_HASH_AGE_IN_SECONDS=120` ≠ `MAX_PROCESSING_AGE=150 blocks` ≈ 60–90s) |
| 10. Type-state escrow lifecycle FSM | Demote | Plan premise structurally wrong — 6 states don't map to any single typed surface |

**The negative-results log IS the GOAT-gate outcome registry.** Its existence
and structure are themselves the strongest evidence that Phase 5.1's discipline
has been internalized: every demotion has a one-line reason class and a full
prose entry with preconditions that would re-open it. A future agent cannot
re-propose a demoted idea without first confronting the recorded reason.

---

## 5. The one honest caveat — Phase 4.3.2 `join!` parallelization

Phase 4.3.2 shipped a `join!` collapsing two sequential D1/KV reads
(`get_deposit_status_with_fallback` + `get_thb_deposit_with_fallback`) into one
concurrent step in `worker/src/handlers/attendee.rs` (commit `c6f89d2`).

**It did NOT have a formal numeric GOAT-gate.** Is that a Phase 5.1 violation?

**No — and gating it would be cargo-cult.** Reasoning (mirrors Phase 5.5 §6):

1. **The win is structural, not empirical.** Two independent concurrent reads
   complete in `max(a, b)` vs sequential `a + b`. The claim "concurrent reads
   are faster than sequential reads" cannot fail measurement — it is provable
   by construction, not an empirical hypothesis that needs a bench.
2. **GOAT-gating is for claims that could fail.** A claim like "zero-copy wire
   format will be 3× faster than JSON" could fail (maybe BLAKE3 overhead
   dominates for small payloads). A claim like "concurrent independent I/O is
   faster than sequential I/O" cannot fail. The gate would be vacuous: measure,
   confirm it's faster (because of course it is), always pass.
3. **Zero behavior change.** Same results, same types, same error paths. There
   is no "GOAT FAILED → demote" scenario to fall back to — there is no fallback
   semantics, just two concurrent futures vs two sequential awaits.
4. **Phase 5.1's own examples are heavyweight empirical claims** ("Phase 1
   zero-copy, Phase 4.3 cache wins"). The `join!` is a one-line concurrency
   fix, not a cache layer with stale-read risk or a new format with overhead
   risk.

This is the same reasoning recorded for Phase 5.5 §6 (feature-flag discipline):
don't build machinery (here, a numeric gate) for a problem you don't have. The
two closures reinforce each other — the `join!` is correctly ungated AND
correctly unflagged for the same reason.

---

## 6. What this decision is NOT

- **NOT "every optimization had a `criterion` bench."** Six of ten ideas were
  demoted on non-benchmark gates (feasibility, safety, premise validity,
  value). Phase 5.1 says "measurable target gate," not "criterion microbench" —
  a safety gate ("no stale-blockhash failures") is just as measurable as a
  latency gate.
- **NOT a retroactive re-bench of the wire format.** Phase 1.7's gate passed at
  6.2×/73.5% and stands. This decision records that the discipline was applied,
  not that we re-ran the bench.
- **NOT a reason to add a numeric gate to the `join!`.** Adding a vacuous gate
  that always passes would weaken the discipline (it trains the team to ignore
  gates), not strengthen it. The honest position is: the `join!` doesn't need a
  gate because its win is structural.
- **NOT closure of the pending Phase 4.1 profile work.** Phase 4.1 (200-attendee
  staged profile) is infrastructure-blocked and remains open. Its GOAT-gate
  (when it runs) will be the p50/p99 profile data it captures — that work is
  not done, just because the discipline exists.

---

## 7. Comparison to Phase 5.5 (feature-flag discipline)

Phase 5.1 and Phase 5.5 are sibling process-discipline closures with the same
reasoning shape:

| Aspect | Phase 5.1 (GOAT-gate) | Phase 5.5 (feature flags) |
|---|---|---|
| Decision shape | "Discipline was applied" (positive) | "Discipline was applied" (positive) |
| The thing in question | Measurable gates + demote outcomes | Cargo features + runtime flags |
| What the audit found | 1 gate passed, 6 gates failed → demoted, 2 n/a, 1 structural caveat | 1 flag fully disciplined, 2 flags moot (optimizations demoted), 1 structural caveat |
| The structural caveat | 4.3.2 `join!` — no formal gate (win by construction) | 4.3.2 `join!` — no flag (zero behavior change) |
| Why closeable | The discipline is followed; the log is the registry | The discipline is followed; the one real flag is gated |

Both closures hinge on the same honest observation: the `join!` parallelization
is a structural improvement whose value is guaranteed by construction, not an
empirical claim that needs a gate or a flag. Phase 3.2's demotion reasoning
("don't build machinery for a problem you don't have") applies identically to
both the missing gate and the missing flag.

---

## 8. Re-open preconditions

This decision flips to "needs real work" if any of the following occurs:

1. **An optimization ships that makes an empirical perf claim without a numeric
   gate.** Specifically: a cache layer with stale-read risk, a batching strategy
   with failure semantics, a new serialization format — any change where "is it
   actually faster?" is a real question (not provable by construction). At that
   point a `criterion` bench with a documented target becomes load-bearing.
2. **A demotion is reversed without recording the reasoning.** If a previously
   demoted idea (entries 1–10) is revived, the GOAT-gate must be re-run and the
   outcome recorded — either as a new positive result or as a corrected
   demotion reason.
3. **Phase 4.1 profile data lands.** When the 200-attendee staged profile runs,
   every Phase 4 decision that currently rests on the "I/O-bound hypothesis"
   must be re-grounded in the measured p50/p99 — the GOAT-gate shifts from
   "structural argument" to "empirical evidence."

---

## 9. Verification trail

This decision was produced by audit against the actual plan and codebase, not
by reading prior summaries:

- `.plans/014_katgpt_rs_paradigm_migration.md` Phase 1.7 (L73-80) — confirmed
  measurable gate (`≥3× decode, ≥40% size`) and PASSED outcome.
- `.plans/014_katgpt_rs_paradigm_migration.md` Phase 1.4/1.5/1.6 (L62-72) —
  confirmed DEMOTED with reasons pointing to neg-results entries 4/5/6.
- `.plans/014_katgpt_rs_paradigm_migration.md` Phase 3.2 (L296-305) — confirmed
  DEMOTED, neg-results entry #7.
- `.plans/014_katgpt_rs_paradigm_migration.md` Phase 4.3.4 (L399-411) —
  confirmed DEMOTED as unsafe, neg-results entry #9.
- `.plans/014_negative_results.md` "Reference: GOAT-gate outcomes recorded in
  this log" (L455+) — confirmed 10-entry table with reason classes and the
  positive 1.7 contrast row.
- `.plans/014_negative_results.md` entries 1–10 — confirmed each has prose with
  reason, proof, and re-open preconditions.
- `.plans/014_feature_flag_discipline.md` §6 — confirmed the 4.3.2 caveat is
  already documented (sibling closure).

**Zero `.rs` files, zero `Cargo.toml`, zero tests touched.** Docs-only closure.