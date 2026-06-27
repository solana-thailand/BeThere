# Plan 014 Phase 2.5 — No Transformer VM, No Standalone Logic WASM Module

> **Decision:** Do NOT compile business logic into a standalone WASM module.
> The worker already runs as WASM; the leptos client already runs as WASM;
> both link `domain`. A third "logic WASM" artifact would add a build target
> with zero behavioral benefit.
>
> **Status:** Decided at plan-creation time. This document is the durable
> record referenced by Plan 014 Phase 2.5 task (`014_katgpt_rs_paradigm_migration.md:262-265`).
>
> **Related:** negative-results entry #1 (Transformer VM / WASM-in-weights)
> covers the *original* Objective 2 framing. This document covers the
> *narrower* Phase 2.5 concern — a plain standalone logic WASM shared by both
> runtimes. The two are related but distinct demotions; see §3.

---

## 1. The decision

Plan 014 Objective 2 was originally framed as "WASM/Transformer-VM
integration" — port katgpt-rs's micro-Transformer VM (WASM interpreter
embedded in transformer weights, "Percepta") to host event-checkin business
logic.

The honest reframe (Plan 014 Phase 2 preamble) is: we have no transformer,
so there is no Transformer VM to build. The legitimate kernel of Objective 2
is the **Single Source of Truth** principle — pure logic lives in one crate
that compiles to every target. That is what `domain/` already half-does, and
what Phases 2.1–2.4 finish.

Phase 2.5 is the negative form of that reframe: explicitly do NOT introduce a
**third WASM artifact** ("logic WASM" / "shared WASM core") on top of the two
that already exist. The decision is to stop at SSOT, not to build a VM.

---

## 2. The architecture (verified against the codebase)

Three crates, two WASM artifacts, one shared core:

| Crate | Compiles to | WASM artifact | Links `domain`? |
|---|---|---|---|
| `domain/` | `x86_64` + `wasm32` (per `domain/Cargo.toml:5`) | — (a library, not an artifact) | — (it IS the core) |
| `worker/` | `wasm32-unknown-unknown` via `worker-build` / `wrangler` | `worker.wasm` (`crate-type = ["cdylib", "rlib"]`, `worker/Cargo.toml:75`) | ✅ `event-checkin-domain = { path = "../domain", features = ["qr", "wire"] }` |
| `frontend-leptos/` | `wasm32-unknown-unknown` via `trunk` | `frontend.wasm` (bound to `index.html` by `Trunk.toml`) | ✅ `event-checkin-domain = { path = "../domain", features = ["wire"] }` |

Both WASM runtimes already share the exact same business logic — the
`domain` crate — compiled into each artifact. There is no logic
duplication between the worker and the leptos client; the duplication that
exists (e.g. `frontend-leptos/src/api/types.rs` mirrors) is a UI-layer
concern addressed by Phase 2.3's SSOT mirror guard, not by a logic WASM.

### Why a third WASM adds nothing

A hypothetical `logic.wasm` compiled from `domain` and loaded at runtime by
both the worker and the leptos client would:

1. **Duplicate what static linking already gives us for free.** Both runtimes
   already link `domain` at build time. The compiler inlines, dead-code-
   eliminates, and tree-shakes per target. A runtime-loaded `logic.wasm`
   would re-introduce a dynamic dispatch boundary where there is currently a
   static call — strictly slower and larger.

2. **Add a third build pipeline.** A separate `logic.wasm` needs its own
   `crate-type`, its own target, its own `Cargo.toml`, its own CI step, its
   own versioning story (what happens when the worker ships `logic.wasm v3`
   but the leptos client was built against `v2`?). The two existing build
   pipelines (`worker-build` for the cdylib, `trunk` for the leptos client)
   already share the `domain` path dep — adding a third artifact cannot
   reduce the build surface, only grow it.

3. **Add a runtime ABI surface where there is none.** Statically linked
   `domain` has no ABI — it's Rust types moving across Rust function
   boundaries, fully checked at compile time. A `logic.wasm` would need an
   ABI (function signatures, calling convention, error marshaling across the
   WASM boundary). Every ABI is a place for version skew and silent
   miscompilation. The Cloudflare Workers runtime and the browser do not
   even share a WASM ABI convention (`worker-build` vs `wasm-bindgen`
   bindings differ).

4. **Solve a problem we don't have.** The motivation for a shared logic WASM
   in other projects is usually *cross-language sharing* (a JS frontend and a
   Python backend both calling the same Rust logic). Here both consumers are
   already Rust, already in the same repo, already sharing the same crate via
   path dep. There is no language boundary to bridge.

### What this decision is NOT

- **NOT** a demotion of `wasm-bindgen`, `trunk`, `worker-build`, or any
  existing build tooling. Those produce the two legitimate WASM artifacts
  and are unaffected.
- **NOT** a demotion of the `domain` crate compiling to `wasm32`. That is
  the SSOT foundation — Phase 2's entire premise. Keep it.
- **NOT** a prohibition on `domain` gaining new logic. The decision is about
  *artifacts* (what gets compiled and shipped), not about where business
  logic lives (which should always be `domain`).

---

## 3. Relationship to negative-results entry #1

Negative-results entry #1 (`.plans/014_negative_results.md` §1) demotes the
**Transformer VM / Percepta** idea — a WASM interpreter embedded in
transformer weights, hosting probabilistic inference.

Phase 2.5 demotes a **narrower, blander** idea: a plain standalone
`logic.wasm` compiled from `domain` and loaded by both runtimes, with no
transformer and no probabilistic component. This is the version that might
be re-proposed by someone who reads the neg-result #1 demotion as "okay, no
transformer, but what about a plain shared logic WASM?"

The two demotions are distinct:

| | Neg-result #1 (Transformer VM) | Phase 2.5 (this doc) |
|---|---|---|
| **Idea** | WASM interpreter inside transformer weights | Standalone `logic.wasm` loaded by worker + leptos |
| **Why demoted** | No transformer exists; nothing to inference | Static linking already does this; third artifact adds nothing |
| **Re-open precondition** | A real ML inference workload is added | (see §4 below) |

If someone re-proposes either, point them at the corresponding demotion.

---

## 4. Preconditions that would re-open this decision

A standalone logic WASM would only make sense if one of the following becomes
true:

1. **A non-Rust consumer of business logic appears.** If a JS-only tool
   (e.g. a CLI, a CI script, a Lambda function written in TypeScript) needs to
   run the same validation logic as the worker and leptos client, a
   `logic.wasm` callable from JS via `wasm-bindgen` would be justified.
   Today there is no such consumer; the Sheets API integration and the
   Solana RPC integration are both server-side Rust.

2. **The `domain` crate grows a hot path that benefits from runtime-loading.**
   Today `domain` is type definitions, serializers, and O(1) predicates.
   If it ever grows a heavy, infrequently-changed computation (e.g. a Merkle
   proof generator, a large lookup table) that the leptos client wants to
   lazy-load rather than ship in the initial WASM bundle, a separate
   `logic.wasm` for *that subset* could reduce initial download size. Not on
   any current roadmap.

3. **The two runtimes diverge enough that a shared artifact becomes the
   cheaper sync path.** Today the path dep keeps them in lockstep. If the
   worker and leptos client were ever in separate repos with separate release
   cadences, a versioned `logic.wasm` could replace the path dep. Not
   planned; would be a major architectural change.

None of these are on the current roadmap. Until one is, the decision stands:
**two WASM artifacts, one shared `domain` crate, no third artifact.**

---

## 5. How this closes Phase 2.5

Plan 014 Phase 2.5 task (`014_katgpt_rs_paradigm_migration.md:262-265`)
reads:

> **2.5 Do NOT compile business logic to a standalone WASM module.** The
> worker already runs as WASM; the leptos client already runs as WASM; both
> link `domain`. A separate "logic WASM" would add a third WASM artifact with
> zero benefit. Document this decision in `.plans/014_no_transformer_vm.md`.

This document is that artifact. Phase 2.5 is now `[x]` DONE — documentation
only, no code change, no test change.