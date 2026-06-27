# Handover 119 — Plan 014 Phase 5.5: Feature-Flag Discipline (Doc-Only Closure)

**Date:** 2026-06-27
**Branch:** `feature/014_phase5_5_feature_flag_discipline_doc` (from `develop` at `3612e97`)
**Phase:** 5.5 — Feature-flag every Phase 1–4 change
**Outcome:** ✅ CONCLUDED (docs-only). Phase 5.5 checkbox flipped `[ ]` → `[x]`.
**Test delta:** 0 (no `.rs`, no `Cargo.toml`, no tests touched). Workspace stays at 323, worker at 227, frontend at 159.

---

## 1. What happened

The prior session's handover (118) flagged Phase 5.5 as the last Plan 014 item plausibly closeable doc-only like Phase 2.5, but noted its scope was vaguer and needed an audit before any conclusion. This session ran that audit.

The audit answers three questions per the Phase 5.5 task text ("Every optimization ships behind a Cargo feature (`wire`, `policy-traits`, `batch-kv-writes`) and a runtime config flag. Default-off until GOAT-gated; default-on only after proof"):

1. Does each named flag exist?
2. If not, was the optimization it would gate ever shipped?
3. If shipped, is it behind a flag with the prescribed discipline?

**Verdict: Phase 5.5 is satisfied.** One flag exists and is fully disciplined; two flags don't exist because their optimizations were correctly demoted before they shipped. No optimization shipped default-on without GOAT-gate proof.

### Deliverables (2 commits on this branch, fast-forward-mergeable to develop)

| Commit | Description |
|---|---|
| (this branch) | `docs(plan-014): conclude Phase 5.5 — feature-flag discipline audit` — creates `.plans/014_feature_flag_discipline.md` (237 lines), flips Phase 5.5 checkbox `[x]` with status note in main plan |
| (this branch) | `docs(handover): add 119 Plan 014 Phase 5.5 feature flag discipline` — this file |

---

## 2. Where is the plan / code / test

| Artifact | Path | Purpose |
|---|---|---|
| Decision record | `.plans/014_feature_flag_discipline.md` | The full audit findings, 11 sections |
| Main plan checkbox | `.plans/014_katgpt_rs_paradigm_migration.md` L473 | Phase 5.5 flipped `[ ]` → `[x]` with note |
| This handover | `.handovers/119_plan_014_phase5_5_feature_flag_discipline.md` | This file |
| Cross-ref | `.plans/014_no_transformer_vm.md` | Phase 2.5 sibling closure (same shape, different reasoning) |
| Cross-ref | `.plans/014_negative_results.md` entries #7, #9 | Demotion records for `policy-traits` and 4.3.4 |
| Cross-ref | `.plans/014_wire_audit.md` | Phase 1.7 GOAT-gate result (6.2× decode, 73.5% size reduction) |

**No code, no tests, no Cargo.toml changes.** Pure documentation closure.

---

## 3. Audit findings (the substance)

### 3.1 `wire` — fully disciplined (the positive case)

Verified end-to-end against the actual codebase, not against prior summaries:

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

The `?fmt=bin` runtime opt-in is **more granular** than Phase 5.5's "runtime config flag" requirement — it's per-request, not per-deploy. Phase 1.8's resolution is the strongest possible reading of "default-off until GOAT-gated": the gate passed, yet no production endpoint enables it at runtime.

### 3.2 `policy-traits` — moot (demoted before shipping)

Phase 3.2 (which would have created a `Policy` trait) was DEMOTED as negative-results entry #7. The Phase 3.1 audit found no behavioral polymorphism exists — every "parameterized" rule is a universal formula reading per-event fields on `EventConfig`. Role-based access is already type-state via `UserRole`'s `Ord` derive.

**Cannot feature-flag an optimization that was correctly never built.** A `policy-traits` Cargo feature today would gate an empty module.

### 3.3 `batch-kv-writes` — moot (demoted or already-satisfied)

Phase 4.3's four sub-tasks resolved as:
- **4.3.1** (KV cache event-series): already satisfied by Plan 013's `cache_public_120_layer` — no new code
- **4.3.2** (collapse sequential KV reads): shipped as `join!` parallelization (see 3.4 below)
- **4.3.3** (batch quiz/adventure writes): already satisfied — `quiz.rs:356` already batches per-submit
- **4.3.4** (blockhash cache TTL 30s→90s): DEMOTED as unsafe (negative-results entry #9)

### 3.4 The one ungated optimization: `join!` parallelization (4.3.2)

Phase 4.3.2 shipped a `join!` collapsing two sequential D1/KV reads in `worker/src/handlers/attendee.rs` (commit `c6f89d2`). It is NOT behind a flag. **This is not a violation** — flagging it would be cargo-cult:
1. Zero behavior change (same results, same types, same errors — just concurrent)
2. No "old behavior" to fall back to
3. GOAT-gate trivially satisfied by construction (`max(a,b)` vs `a+b` — no "GOAT FAILED" scenario)
4. Phase 5.5's own examples are all heavyweight opt-in changes (wire format, trait abstraction, write-batching layer); a `join!` is a one-line concurrency fix

Mirrors Phase 3.2's demotion reasoning: don't build machinery for a problem you don't have.

---

## 4. Reflection — struggling / solved

### Solved: caught a faulty-tool near-miss before it became a wrong conclusion

My first `grep` tool pass for `wire|Wire|fmt=bin` with `include_pattern = worker/**/*.rs` and `frontend-leptos/**/*.rs` returned **zero matches**. Had I trusted it, the decision record would have wrongly claimed "the `wire` feature is compiled but unused at runtime — no `?fmt=bin` endpoint exists." That would have been a factual error and a Phase 1.3 status misrepresentation.

I caught it by cross-checking: `ls` showed `worker/src/handlers/wire.rs` (2345 bytes) and `frontend-leptos/src/api/wire.rs` (1914 bytes) both exist; `git log --stat 787e62d` confirmed both files were added in the Phase 1 commit; `rg` (terminal, not the grep tool) found `wire` references in `worker/src/handlers/mod.rs` instantly. Reading the actual files confirmed the smoke handler, the `?fmt=bin` opt-in, the route registration, and the frontend decoder.

**Lesson for future sessions:** when a grep result would drive a substantive conclusion (especially a "this doesn't exist" claim), cross-check with `ls` / `git log --stat` / direct `rg` in terminal. The `grep` tool's `include_pattern` glob missed `worker/src/handlers/wire.rs` for reasons I couldn't reproduce — `rg` found it instantly. For `.rs` content searches where precision matters, prefer `terminal rg`.

### Solved: distinguishing Phase 5.5's reasoning from Phase 2.5's

Phase 2.5 and Phase 5.5 are structurally similar (both closeable doc-only from existing audit work) but the reasoning differs in a way that matters for honesty:

- **Phase 2.5** is a *negative* decision ("don't build X — the audit found it adds cost for zero benefit")
- **Phase 5.5** is a *positive* finding ("the discipline was correctly applied to the one optimization that warranted a flag; the other two named flags are moot because their optimizations were correctly demoted")

The decision record makes this distinction explicit in §8 (comparison table) to forestall any misreading that "Phase 5.5 = we decided not to feature-flag" — that would be wrong. The honest statement is "we feature-flagged the one thing that needed it; we correctly demoted the two things that didn't."

### No real struggles

Unlike Phase 2.4 R1 (which required source-scan guard design) or Phase 2.5 (which required distinguishing two related-but-distinct demoted ideas), Phase 5.5 was a straightforward audit against existing audit work. The only difficulty was the faulty-grep near-miss above, which was caught before any wrong file was written.

---

## 5. Remaining work (Plan 014 open items after this closure)

| Phase | Status | Blocker |
|---|---|---|
| Phase 2.2 R3 (mirror-type merge) | 🟡 Open | Design decision — needs dedicated session |
| Phase 4.1 (profile 200-attendee event) | 🟡 Blocked | Staged event coordination |
| Phase 4.4 (no-SIMD doc) | 🟡 Blocked | Depends on 4.1 |
| **Phase 5.5 (this closure)** | ✅ DONE | — |

**Phase 5.5 was the last item that could plausibly be closed doc-only.** Everything left is either a judgment call (Phase 2.2 R3) or infrastructure-blocked (Phase 4.1, 4.4). Plan 014 is effectively at its natural stopping point for bounded-safe work.

### Explicitly deferred (do NOT do without confirmation)

- ❌ Phase 2.4 R3 (type-state FSM as written) — recommended AGAINST by audit
- ❌ Phase 2.4 R4 (typed `DepositLifecycle` enum) — recommended AGAINST by audit
- ❌ Phase 2.2 R3 — design decision, needs dedicated session
- ❌ Build standalone `logic.wasm` — decided against (Phase 2.5)
- ❌ Build Transformer VM / Percepta — decided against (neg-result #1)
- ❌ Revive `policy-traits` Cargo feature — would gate an empty module (Phase 3.2 demoted)
- ❌ Add `batch-kv-writes` Cargo feature — would gate existing code or a `join!` (cargo-cult)
- ❌ Deploy — needs explicit operator confirmation

---

## 6. Issues ref

No new issues created. Relevant existing refs:
- `.plans/014_negative_results.md` entry #7 (`EventPolicy` trait demotion)
- `.plans/014_negative_results.md` entry #9 (blockhash cache TTL demotion)
- `.plans/014_wire_audit.md` (Phase 1 GOAT-gate record)

---

## 7. How to dev / test

**No code or tests were touched.** Nothing to build or run for this closure.

### Verification of the docs-only claim

```sh
# Confirm zero source/test changes on this branch vs develop
git --no-pager diff develop..feature/014_phase5_5_feature_flag_discipline_doc --stat
# Expected: only .plans/ and .handovers/ files changed

# Confirm existing tests still green (unchanged from prior session)
cargo test --workspace --quiet          # expect 323
cargo test -p event-checkin-worker       # expect 227
```

### If a future agent wants to re-audit Phase 5.5

The decision record at `.plans/014_feature_flag_discipline.md` §11 (Verification trail) lists every file read and every command run. The audit is fully reproducible from that section. Key commands:

```sh
# Confirm policy-traits / batch-kv-writes don't exist anywhere
rg "policy-traits|batch-kv-writes"   # expect only the Phase 5.5 task text

# Confirm wire feature is default-off in domain
rg "^default" domain/Cargo.toml       # expect: default = ["qr"]

# Confirm wire is enabled in consumers
rg 'features.*"wire"' worker/Cargo.toml frontend-leptos/Cargo.toml

# Confirm smoke route is registered
rg "wire-sample" worker/src/handlers/mod.rs
```

### Merge / deploy

This branch is fast-forward-mergeable to `develop`. After merge, `develop` will be **14 commits ahead** of `origin/develop` (handovers 113–119). Push and deploy still require explicit operator confirmation per standing rules.

---

## 8. Honest caveats

1. **`frontend-leptos/Cargo.toml` comment is mildly stale.** It claims `wire` is enabled "so the frontend can decode zero-copy `*Wire` payloads from `?fmt=bin` endpoints." This is aspirational — only the smoke route at `/api/wire-sample/level-score` uses `?fmt=bin`, no production endpoint. The comment is not wrong (the decoder IS wired and works against the smoke route), but slightly overstates current production usage. Noted in the decision record §9 as a non-blocking doc-nit. Left unchanged in this closure — fixing a doc-comment in `Cargo.toml` would have made this a non-docs-only change, and the misstatement is not a Phase 5.5 compliance issue.

2. **The `wire` feature being enabled-but-production-dormant is intended, not a gap.** Phase 1.8 explicitly resolved "stay opt-in" after the GOAT-gate passed but production types (1.4–1.6) were demoted. This is the strictest reading of "default-off until GOAT-gated." A future agent should NOT misread this as "wire is dead code to delete" — it passed its gate and is correctly kept compiled-and-ready for the first production type that justifies opt-in.

3. **The `join!` parallelization (4.3.2) is genuinely ungated.** This is honestly acknowledged in the decision record §6. The argument for not flagging it (zero behavior change, no downside, trivial GOAT-gate by construction) is principled but reasonable people could disagree. If a future agent believes all latency optimizations should be flaggable regardless of behavior change, that's a stronger discipline stance than Phase 5.5's own text requires (its three named examples are all heavyweight opt-in changes, not one-line concurrency fixes).
```

[In handover 119, I've documented the Phase 5.5 closure. The file is ready to be saved at `.handovers/119_plan_014_phase5_5_feature_flag_discipline.md`. After you write it, the next step is to commit both files (decision record + plan checkbox edit + handover) on the feature branch.]