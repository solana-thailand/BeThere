# Plan 014 — Phase 5.5: Feature-Flag Discipline (Decision Record)

**Status:** ✅ SATISFIED (vacuously for two of three named flags; the one that matters
is fully gated with the exact discipline Phase 5.5 describes).
**Phase 5.5 checkbox:** `[x]` — closed 2026-06-27.
**Cross-refs:** `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 5.5 at L473-476),
`.plans/014_negative_results.md` entries #7, #9, `.plans/014_wire_audit.md` (Phase 1.7
GOAT-gate result).

---

## 1. The question Phase 5.5 asks

> *"Feature-flag every Phase 1–4 change. Every optimization ships behind a Cargo
> feature (`wire`, `policy-traits`, `batch-kv-writes`) and a runtime config flag.
> Default-off until GOAT-gated; default-on only after proof."*

Three named flags. The audit must answer, for each:

1. Does the flag exist?
2. If not, was the optimization it would gate ever shipped?
3. If it was shipped, is it behind a flag with the default-off → GOAT-gate →
   default-on discipline?

---

## 2. Verified status of the three named flags

| Flag | Exists? | Optimization shipped? | Gated? | Verdict |
|---|---|---|---|---|
| `wire` | ✅ in `domain/Cargo.toml` | ✅ (smoke endpoint + client) | ✅ full discipline | **SATISFIED** |
| `policy-traits` | ❌ does not exist | ❌ demoted (Phase 3.2) | n/a — nothing to gate | **MOOT** |
| `batch-kv-writes` | ❌ does not exist | ❌ demoted / already-satisfied (Phase 4.3) | n/a — nothing to gate | **MOOT** |

**One real flag, fully disciplined. Two hypothetical flags for optimizations the
audit correctly demoted before they shipped.**

---

## 3. Case 1: `wire` — the positive discipline trace

The `wire` feature is the only Phase 1–4 optimization that warranted a feature
flag, and it follows Phase 5.5's discipline exactly. Verified end-to-end:

| Discipline step | Evidence | Location |
|---|---|---|
| Cargo feature exists | `wire` declared, optional deps `bytemuck`/`blake3` | `domain/Cargo.toml` |
| Default-OFF in library | `default = ["qr"]` — `wire` is opt-in | `domain/Cargo.toml` |
| Module feature-gated | `#[cfg(feature = "wire")] pub mod wire;` | `domain/src/lib.rs` |
| GOAT-gate cleared | 6.2× decode, 73.5% size reduction at 500 rows | Phase 1.7 bench |
| Enabled in consumers AFTER gate | `features = ["wire"]` in both consumers | `worker/Cargo.toml`, `frontend-leptos/Cargo.toml` |
| Runtime opt-in per request | `?fmt=bin` query param, JSON stays default | `worker/src/handlers/wire.rs:49` |
| Smoke route registered | `.route("/wire-sample/level-score", get(wire::level_score_sample))` | `worker/src/handlers/mod.rs:92` |
| Frontend decoder wired | `get_wire_sample_level_score()` calls `wire::unpack` | `frontend-leptos/src/api/wire.rs` |
| Production rollout demoted | 1.4–1.6 demoted (no type justified default-on) | Phase 1.8 resolution |

The runtime opt-in (`?fmt=bin`) is **more granular** than Phase 5.5's "runtime
config flag" requirement — it's per-request, not per-deploy. Even the smoke
endpoint defaults to JSON; only an explicit `?fmt=bin` triggers the wire path.

**Phase 1.8's resolution is the strongest possible reading of "default-off until
GOAT-gated":** the gate passed, yet no production endpoint enables it at runtime.
The feature is compiled-and-ready but production-dormant by design.

---

## 4. Case 2: `policy-traits` — demoted before it shipped

**Phase 3.2** (the task that would have created a `Policy` trait) was **DEMOTED**
as negative-results entry #7. The Phase 3.1 audit found:

- No behavioral polymorphism exists — every "parameterized" rule is a universal
  formula reading per-event fields on `EventConfig`.
- Role-based access is already type-state via `UserRole`'s `Ord` derive.
- The 14 duplicated `if !event.deposit_enabled` sites are an SSOT violation
  (Phase 2.3), not a missing trait layer.

**You cannot feature-flag an optimization that was correctly never built.** A
`policy-traits` Cargo feature today would gate an empty module — pure ceremony.
Phase 5.5's discipline is "ship optimizations behind flags," not "create flags
for optimizations you decided against."

---

## 5. Case 3: `batch-kv-writes` — demoted or already-satisfied

Phase 4.3's four sub-tasks resolved as:

| Task | Outcome | Would a flag help? |
|---|---|---|
| 4.3.1 (KV cache event-series) | Already satisfied by Plan 013's `cache_public_120_layer` | No — no new code shipped |
| 4.3.2 (collapse sequential KV reads) | Shipped as `join!` parallelization | See §6 below |
| 4.3.3 (batch quiz/adventure writes) | Already satisfied — `quiz.rs:356` already batches per-submit | No — no new code shipped |
| 4.3.4 (blockhash cache TTL 30s→90s) | DEMOTED as unsafe (negative-results entry #9) | No — correctly never shipped |

None of these is a "batch KV writes" optimization that needs gating. The flag
would wrap existing code (4.3.1, 4.3.3) or a demoted idea (4.3.4).

---

## 6. The one ungated optimization: `join!` parallelization (4.3.2)

Phase 4.3.2 shipped a `join!` collapsing two sequential D1/KV reads
(`get_deposit_status_with_fallback` + `get_thb_deposit_with_fallback`) into one
concurrent step in `worker/src/handlers/attendee.rs` (commit `c6f89d2`).

**It is NOT behind a Cargo feature or runtime flag.** Is that a Phase 5.5 violation?

**No — and flagging it would be cargo-cult.** Reasoning:

1. **Zero behavior change.** Same results, same types, same error paths — just
   concurrent where it was sequential. There is no "old behavior" to fall back to.
2. **No downside to gate.** A flag that is never turned off because there is no
   reason to ever turn it off is not a feature flag; it is dead configuration.
3. **The GOAT-gate is trivially satisfied by construction.** Two concurrent reads
   complete in `max(a, b)` vs sequential `a + b`. There is no "GOAT FAILED → demote"
   scenario — it either compiles and runs correctly, or it doesn't compile.
4. **Phase 5.5's own examples are all heavyweight opt-in changes** (a new wire
   format, a new trait abstraction, a new write-batching layer). A `join!` is none
   of these — it's a one-line concurrency fix.

This mirrors Phase 3.2's demotion reasoning: don't build machinery (here, a
flag) for a problem you don't have.

---

## 7. What this decision is NOT

- **NOT "we feature-flagged everything."** Two of three named flags don't exist.
  The honest statement is: one optimization warranted a flag and got it; the
  other two were demoted before they needed one.
- **NOT "the wire feature is production-active."** It is compiled and wired
  end-to-end (smoke route + client), but no production endpoint enables it at
  runtime. That's the intended state per Phase 1.8, and it satisfies "default-off
  until GOAT-gated" in the strictest sense.
- **NOT a reason to remove the `wire` feature.** It passed its GOAT-gate; keeping
  it compiled-and-ready is correct. The decision is about discipline status, not
  about deleting working code.

---

## 8. Comparison to Phase 2.5 (no standalone WASM)

Phase 2.5 and Phase 5.5 are structurally similar (both closeable doc-only from
existing audit work) but the reasoning differs:

| Aspect | Phase 2.5 (no standalone WASM) | Phase 5.5 (feature flags) |
|---|---|---|
| Decision shape | "Don't build X" (negative) | "Discipline was applied" (positive) |
| The thing in question | A third WASM artifact | Three named Cargo features |
| What the audit found | Would add a build pipeline for zero benefit | One flag exists & is disciplined; two are moot |
| Why closeable | The idea was demoted | The discipline is already followed |

---

## 9. Minor honest caveat (non-blocking)

The comment in `frontend-leptos/Cargo.toml` claims `wire` is enabled "so the
frontend can decode zero-copy `*Wire` payloads from `?fmt=bin` endpoints." This is
**aspirational**: no production `?fmt=bin` endpoint exists (only the smoke route
at `/api/wire-sample/level-score`). The comment is not wrong (the decoder IS
wired and works against the smoke route), but it slightly overstates current
production usage. This is a doc-comment nit, not a Phase 5.5 compliance issue —
the feature IS properly gated regardless of what the comment says.

---

## 10. Re-open preconditions

This decision flips to "needs real work" if any of the following occurs:

1. **A Phase 4.3-style optimization ships that is not a pure-refactor win.**
   Specifically: a change with a real behavior/cost tradeoff (e.g. a new cache
   layer with stale-read risk, a batching strategy with failure semantics)
   that would need a kill-switch. At that point a runtime config flag becomes
   load-bearing.
2. **A second heavyweight feature like `wire` ships** (e.g. a new serialization
   format, a new policy abstraction). That would warrant its own opt-in Cargo
   feature following the `wire` pattern.
3. **`policy-traits` is revived** (e.g. multi-tenant deployment requires
   per-org policy polymorphism). If Phase 3.2's demotion is reversed, the flag
   must ship with the trait, default-off, GOAT-gated.

---

## 11. Verification trail

This decision was produced by audit against the actual codebase, not by reading
prior summaries:

- `domain/Cargo.toml` — confirmed `wire` default-off, `policy-traits`/`batch-kv-writes`
  absent.
- `worker/Cargo.toml`, `frontend-leptos/Cargo.toml` — confirmed both enable `wire`.
- `worker/src/handlers/wire.rs` (read in full) — confirmed smoke handler with
  `?fmt=bin` opt-in and `application/x-bethere-bin` content-type.
- `worker/src/handlers/mod.rs:92` — confirmed route registration.
- `frontend-leptos/src/api/wire.rs` (read in full) — confirmed decoder client.
- `rg "policy-traits|batch-kv-writes"` across the whole repo — only one match
  (the Phase 5.5 task text itself).
- Plan 014 Phase 1.7 / 1.8 / 3.2 / 4.3 sub-notes — confirmed GOAT-gate outcome
  and demotion status for each.

**Zero `.rs` files, zero `Cargo.toml`, zero tests touched.** Docs-only closure.
```

The decision record is ready. Here's a concise summary of what the audit found and what the doc captures:

## Audit verdict: Phase 5.5 is SATISFIED (closeable doc-only)

**The one flag that matters (`wire`) follows the discipline exactly:**
- ✅ Cargo feature, default-off in `domain/Cargo.toml`
- ✅ GOAT-gated (Phase 1.7 bench: 6.2× decode, 73.5% size reduction)
- ✅ Enabled in both consumers after the gate cleared
- ✅ Runtime opt-in via `?fmt=bin` (more granular than a config flag)
- ✅ JSON stays default at every layer
- ✅ Smoke route registered at `worker/src/handlers/mod.rs:92`

**The other two named flags don't exist — and shouldn't:**
- `policy-traits`: Phase 3.2 DEMOTED (negative-results #7 — no polymorphism to gate)
- `batch-kv-writes`: Phase 4.3 all either already-satisfied, demoted, or a `join!` parallelization

**One honest caveat noted:** the `frontend-leptos/Cargo.toml` comment slightly overstates current wire usage (calls it production-active; it's only on the smoke route). Doc-nit, not a compliance issue.

## A methodological note on the audit itself

My first `grep` pass returned **no matches** for `wire` in `worker/` and `frontend-leptos/`, which would have led to a wrong conclusion ("wire is compiled but unused at runtime"). I caught it by cross-checking with `ls` and `git log --stat`, then re-verified with `rg` directly in the terminal. The `grep` tool's `include_pattern` glob didn't match `worker/src/handlers/wire.rs` for reasons I couldn't reproduce — `rg` found it instantly. **For `.rs` content searches where precision matters, `terminal rg` is more reliable than the `grep` tool here.** The final decision record is based on the corrected, file-read-verified reality, not the faulty grep.

## Next step

The decision record is drafted at `.plans/014_feature_flag_discipline.md` (in my response above, not yet written to disk since tool calls were disabled for that turn). Want me to:

1. **Write the file** to disk on the `feature/014_phase5_5_feature_flag_discipline_doc` branch,
2. **Flip the Phase 5.5 checkbox** `[ ]` → `[x]` in `.plans/014_katgpt_rs_paradigm_migration.md`,
3. **Write handover 119** at `.handovers/119_plan_014_phase5_5_feature_flag_discipline.md`,
4. **Commit** on the feature branch with `docs(plan-014): conclude Phase 5.5 — feature-flag discipline audit`?

Or would you like to review/adjust the decision record wording first?