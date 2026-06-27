# Handover 118 — Plan 014 Phase 2.5 No Transformer VM / No Standalone Logic WASM

**Branch**: `feature/014_phase2_5_no_transformer_vm_doc` (from `develop` @ `e1f0f9c`)
**Phase**: Plan 014 Phase 2.5 (documentation-only deliverable)
**Scope**: Create the missing `.plans/014_no_transformer_vm.md` decision record; mark Phase 2.5 `[x]` in the main plan.
**Risk profile**: Documentation only — no code, no tests, no behavior change.

---

## 1. What Happened

Plan 014 Phase 2.5 task (`014_katgpt_rs_paradigm_migration.md:262-265`) read:

> **2.5 Do NOT compile business logic to a standalone WASM module.** The worker already runs as WASM; the leptos client already runs as WASM; both link `domain`. A separate "logic WASM" would add a third WASM artifact with zero benefit. Document this decision in `.plans/014_no_transformer_vm.md`.

The decision was made at plan-creation time, but the **documentation artifact was never created** — `.plans/014_no_transformer_vm.md` did not exist (verified via `find_path`). The Phase 2.5 checkbox was still `[ ]`.

The prior session's handover 117 noted Phase 2.5 was "mostly satisfied by neg-result #1." The audit-first pass in this session found that claim **partially correct but incomplete**:

- **Neg-result #1** (`.plans/014_negative_results.md` §1) demotes the **Transformer VM / Percepta** idea — a WASM interpreter embedded in transformer weights, hosting probabilistic inference. That is the original Objective 2 framing.
- **Phase 2.5** demotes a **narrower, blander** idea: a plain standalone `logic.wasm` compiled from `domain` and loaded at runtime by both the worker and the leptos client, with no transformer and no probabilistic component.

The two demotions are related but distinct. A reader who accepts neg-result #1's "no transformer" verdict could still re-propose a plain shared `logic.wasm` — and that proposal needs its own "no, and here's why" answer. Phase 2.5 closes that gap.

### The audit-first verification before writing

Before writing the doc, the following architectural facts were verified against the actual codebase:

| Crate | Compiles to | WASM artifact | Links `domain`? |
|---|---|---|---|
| `domain/` | `x86_64` + `wasm32` (per `domain/Cargo.toml:5`: "compiles x86_64 + wasm32") | — (a library, not an artifact) | — (it IS the core) |
| `worker/` | `wasm32-unknown-unknown` via `worker-build` / `wrangler` | `worker.wasm` (`worker/Cargo.toml:75`: `crate-type = ["cdylib", "rlib"]`) | ✅ `event-checkin-domain = { path = "../domain", features = ["qr", "wire"] }` |
| `frontend-leptos/` | `wasm32-unknown-unknown` via `trunk` | `frontend.wasm` (bound to `index.html` by `Trunk.toml`) | ✅ `frontend-leptos/Cargo.toml`: `event-checkin-domain = { path = "../domain", features = ["wire"] }` |

Both WASM runtimes already share the exact same business logic via the `domain` path dep, statically linked and tree-shaken per target. There is no logic duplication between the worker and leptos client that a third WASM artifact could resolve; the duplication that exists (e.g. `frontend-leptos/src/api/types.rs` mirrors) is a UI-layer concern addressed by Phase 2.3's SSOT mirror guard (handover 112), not a logic-WASM concern.

This matches exactly what Phase 2.5's task description asserted — the audit confirmed the premise before documenting it.

### The four-argument case against a third WASM

The new doc presents four distinct reasons a `logic.wasm` adds nothing here:

1. **Static linking already does it for free.** Both runtimes link `domain` at build time; the compiler inlines, dead-code-eliminates, and tree-shakes per target. A runtime-loaded `logic.wasm` re-introduces a dynamic dispatch boundary where there is currently a static call.
2. **A third build pipeline.** A separate `logic.wasm` needs its own `crate-type`, its own target, its own `Cargo.toml`, its own CI step, its own versioning story (worker ships `logic.wasm v3` but leptos client was built against `v2`?).
3. **A runtime ABI surface where there is none.** Statically linked `domain` has no ABI — Rust types across Rust function boundaries, fully checked at compile time. A `logic.wasm` needs an ABI (function signatures, calling convention, error marshaling across the WASM boundary). The Cloudflare Workers runtime and the browser do not even share a WASM ABI convention (`worker-build` vs `wasm-bindgen` bindings differ).
4. **Solves a problem we don't have.** The usual motivation for a shared logic WASM is *cross-language sharing* (JS frontend + Python backend both calling the same Rust logic). Here both consumers are already Rust, already in the same repo, already sharing the same crate via path dep. No language boundary to bridge.

---

## 2. Changes (1 new doc, 2 small edits)

### New: `.plans/014_no_transformer_vm.md` (166 lines)

Five-section decision record:

1. **The decision** — no third WASM artifact; Phase 2.5 is the negative form of the SSOT reframe.
2. **The architecture (verified against the codebase)** — the three-crates / two-WASM / one-shared-core table above; the four-argument case against a third WASM; an explicit "what this decision is NOT" section (NOT a demotion of `wasm-bindgen`/`trunk`/`worker-build`; NOT a demotion of `domain` compiling to `wasm32`; NOT a prohibition on `domain` gaining new logic).
3. **Relationship to negative-results entry #1** — comparison table distinguishing the Transformer VM / Percepta demotion from this narrower plain-logic-WASM demotion.
4. **Preconditions that would re-open this decision** — three concrete triggers: (a) a non-Rust consumer of business logic appears, (b) `domain` grows a heavy lazy-loadable subset, (c) worker and leptos client move to separate repos. None on the current roadmap.
5. **How this closes Phase 2.5** — explicit pointer back to the task line and the `[x]` mark.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md` (+4/-2)

Phase 2.5 task checkbox flipped from `[ ]` to `[x]`, with the doc reference expanded to note the relationship to neg-result #1 and that no code or test change was made.

### Edited: `.plans/014_negative_results.md` (+5)

Reverse cross-reference added at the end of entry #1: a "Related (narrower) demotion" paragraph pointing from neg-result #1 (Transformer VM) to the new Phase 2.5 doc (plain standalone logic WASM). Improves discoverability in both directions.

### NOT changed (deliberately)

- No code change (no `.rs` files touched).
- No test change (test counts unchanged: 323 workspace, 227 worker crate).
- No `Cargo.toml` change.
- No frontend change.

---

## 3. Validation Done

### Docs-only sanity checks

```
# Confirm the new doc renders with clean heading structure:
rg -n "^#" .plans/014_no_transformer_vm.md
→ 1 H1 + 5 H2 + 2 H3 — well-formed

# Confirm Phase 2.5 is the only checkbox flipped in the plan:
git diff develop -- .plans/014_katgpt_rs_paradigm_migration.md
→ 1 line changed: [ ] 2.5 → [x] 2.5 (plus 3-line pointer expansion)

# Confirm cross-references resolve:
eza .plans/014_no_transformer_vm.md .plans/014_negative_results.md .plans/014_katgpt_rs_paradigm_migration.md
→ all three exist
```

### Tests (regression check — should be unchanged)

```
cargo test -p event-checkin-worker --test escrow_transition_contract
→ 12 passed; 0 failed  (Phase 2.4 R1 guard from handover 117 still green)
```

No workspace test run was repeated — this PR touches only `.plans/*.md`; no `.rs` files, no `Cargo.toml`. The prior session's count (323 workspace, 227 worker crate) stands.

---

## 4. Plan / Code / Test Locations

| Artifact | Path |
|---|---|
| **New decision doc** | `.plans/014_no_transformer_vm.md` |
| Phase 2.5 task (now `[x]`) | `.plans/014_katgpt_rs_paradigm_migration.md:262` |
| Related neg-result (Transformer VM) | `.plans/014_negative_results.md` §1 |
| Reverse cross-ref added | `.plans/014_negative_results.md` after §1's preconditions |
| Audit baseline (architecture facts) | `worker/Cargo.toml:75`, `domain/Cargo.toml:5`, `frontend-leptos/Cargo.toml` (domain path dep) |

No code, no tests to run.

---

## 5. Reflections

### What went well

- **Audit-first caught that neg-result #1 does not actually close Phase 2.5.** The prior session's "mostly satisfied" note was directionally right but missed that the two demotions address different proposals. The audit pass (read Phase 2.5 verbatim, read neg-result #1 verbatim, compare scopes) caught this before writing.
- **The four-argument case against a third WASM is concrete, not hand-wavy.** Each argument cites a specific cost (build pipeline, ABI surface, version skew) rather than a vague "we don't need it." Future re-proposals have to engage with the actual tradeoffs, not just restate the conclusion.
- **The "what this decision is NOT" section forestalls over-reading.** Without it, someone could misread this as "stop compiling `domain` to `wasm32`" — which would contradict Phase 2's entire premise. Explicit non-goals prevent that.
- **The reverse cross-reference** makes the two related demotions discoverable from either starting point.

### What was harder than expected

- **Drawing the precise line between neg-result #1 and Phase 2.5.** The two ideas are close cousins (both "no, don't build a WASM thing"). The clean separation turned out to be: neg-result #1 is about a *specific* WASM thing (the Percepta transformer-weight interpreter), Phase 2.5 is about the *blandest possible* WASM thing (a plain shared `logic.wasm`). Once that framing was written down, the comparison table in §3 of the doc made the distinction crisp.
- **Resisting scope creep into "should we eliminate `frontend-leptos/src/api/types.rs` mirrors."** Those mirrors are a genuine SSOT concern (Phase 2.3, handover 112), but they are not what Phase 2.5 is about. Mentioning them as context is correct; trying to address them here would be wrong scope. Left as a one-line pointer ("addressed by Phase 2.3's SSOT mirror guard") and moved on.

### Where the result differs from prior session's note

- **Handover 117 said Phase 2.5 was "mostly satisfied by neg-result #1."** This session's audit found that was an over-simplification. The narrow "plain standalone logic WASM" proposal is not covered by neg-result #1, and the missing `.plans/014_no_transformer_vm.md` was a real gap (the task explicitly named that file). Handover 117's "mostly satisfied" is now upgraded to "fully satisfied" with the proper artifact in place.

---

## 6. Remaining Work

### Plan 014 status after this handover

| Phase | Status | Notes |
|---|---|---|
| **Phase 1** (wire format) | ✅ CONCLUDED | |
| **Phase 2.1** (SSOT audit) | ✅ CONCLUDED | Handover 113 |
| **Phase 2.2** (move predicates) | 🟡 PARTIAL | R1 done (114), R2 done (115), only R3 open |
| **Phase 2.3** (CI dup-check) | ✅ CONCLUDED | Handover 112; scope gap closed by R1 (114) |
| **Phase 2.4** (type-state escrow) | ✅ CONCLUDED | Audit: handover 116. R1: handover 117 |
| **Phase 2.5** (no standalone WASM) | ✅ **CONCLUDED (this handover)** | Doc created; checkbox `[x]` |
| **Phase 3** (policy traits) | ✅ CONCLUDED | |
| **Phase 4.1** (profile) | 🟡 Blocked | Needs 200-attendee staged event |
| **Phase 4.3** (I/O wins) | ✅ CONCLUDED | |
| **Phase 4.4** (no-SIMD doc) | 🟡 Blocked | Depends on 4.1 |
| **Phase 5.3** (deterministic guard) | ✅ CONCLUDED | |
| **Phase 5.4** (zero-alloc audit) | ✅ CONCLUDED | |
| **Phase 5.5** (feature-flag discipline) | 🟡 Open | Mostly satisfied |

### Other open Plan 014 work (priority order)

1. **Phase 2.2 R3** — Substantive mirror-type merge decision for `EventFormat`/`EscrowStatus`/etc. Design decision; needs dedicated session.
2. **Phase 4.1** — Profile a staged 200-attendee event end-to-end. Blocked on infrastructure.
3. **Phase 4.4** — Document the "no SIMD" decision. Blocked on 4.1.
4. **Phase 5.5** — Feature-flag discipline documentation. Mostly satisfied.

### Immediate operator actions

- [ ] **Push** `develop` to `origin` — `git push origin develop`. After this handover's branch is merged, `develop` will be **12+ commits ahead** of `origin/develop`.
- [ ] **Deploy** via `develop → main → deploy.sh`. **Requires explicit operator confirmation.** No schema changes; rollback is `wrangler rollback`.

### Housekeeping

- [ ] After merge, delete the feature branch: `git branch -d feature/014_phase2_5_no_transformer_vm_doc`
- [ ] Older feature branches still pending deletion: `feature/014_ssot_audit`, `feature/014_phase2_2_guard_widen`, `feature/014_phase2_2_r2_deposit_from_str`, `feature/014_phase2_4_audit`, `feature/014_phase2_4_r1_transition_contract`.

### Explicitly deferred (do NOT do without confirmation)

- ❌ **Build a standalone `logic.wasm`** — explicitly decided against (this handover + neg-result #1).
- ❌ **Build a Transformer VM / Percepta interpreter** — explicitly decided against (neg-result #1).
- ❌ **Phase 2.4 R3 / R4** — explicitly recommended against by Phase 2.4 audit.
- ❌ **Phase 2.2 R3** — design decision, needs dedicated session.
- ❌ **Deploy** — needs explicit operator confirmation.
- ❌ **Phase 4.1 / 4.4** — blocked on infrastructure.

---

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md`
- Phase 2.5 decision doc: `.plans/014_no_transformer_vm.md` (new)
- Negative results log: `.plans/014_negative_results.md`
- Prior handovers: 112 (Phase 2.3 SSOT mirror), 113 (Phase 2.1 SSOT audit), 114 (Phase 2.2 R1), 115 (Phase 2.2 R2), 116 (Phase 2.4 audit), 117 (Phase 2.4 R1 transition contract).

---

## 8. How to Dev / Test

### Read the decision

```sh
bat .plans/014_no_transformer_vm.md
```

### Confirm Phase 2.5 is checked

```sh
rg "2\.5" .plans/014_katgpt_rs_paradigm_migration.md
# → [x] 2.5 ... Decision documented in .plans/014_no_transformer_vm.md ...
```

### Confirm cross-references resolve

```sh
# New doc references neg-result #1:
rg "negative-results entry #1" .plans/014_no_transformer_vm.md

# Neg-result #1 references the new doc:
rg "Phase 2.5.*014_no_transformer_vm" .plans/014_negative_results.md
```

### Verify the architecture facts (if the build setup ever changes)

```sh
# Worker compiles to WASM (cdylib):
rg "crate-type" worker/Cargo.toml
# → crate-type = ["cdylib", "rlib"]

# Domain compiles to x86_64 + wasm32:
rg "compiles x86_64 \+ wasm32" domain/Cargo.toml

# Frontend links domain:
rg "event-checkin-domain" frontend-leptos/Cargo.toml

# Worker links domain:
rg "event-checkin-domain" worker/Cargo.toml
```

If any of these four facts change (e.g. a third runtime is added, or `domain` stops compiling to `wasm32`), the Phase 2.5 decision should be re-evaluated against the new preconditions in §4 of the doc.

### Confirm no code or tests were touched

```sh
git diff develop --stat
# → only .plans/*.md files changed
```

### Relationship to Phase 2.4 R1 (the prior session's guard)

This handover is documentation-only and does not touch the Phase 2.4 R1 transition contract test from handover 117. The escrow allowlist guard (`worker/tests/escrow_transition_contract.rs`, 12 tests) remains the authoritative runtime-contract test for the monetary state machine. Phase 2.5 is about *build artifacts* (what gets compiled and shipped as WASM); Phase 2.4 R1 is about *runtime behavior* (what transitions are allowed). They do not interact.