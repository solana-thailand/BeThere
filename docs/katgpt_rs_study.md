# What event-checkin can take from katgpt-rs

**Date:** 2026-09-23 · **Source:** `~/projects/katgpt-rs` (the owner's fork, branch
`develop` @ `ecce691c`). `~/src/katgpt-rs` is a stale July clone and was not used.
**Method:** five read-only study passes, one each on governance, CI/tooling,
reusable crates, perf/security craft, and agent tooling. Every citation below
was read, not inferred, unless it is marked *(inferred)*.
**Adoption queue:** `.plans/030_katgpt_rs_adoption.md`.

## 1. What katgpt-rs is

- A Rust workspace of about 1.04 M lines: 31 crates and a root, MIT licensed.
- It holds modelless ML-inference primitives: transformer kernels,
  speculative decoding, KV-cache codecs, constraint pruners, and research
  substrates. It has 641 feature flags, 204 of them on by default.
- It is upstream of the `riir-*` repos and has no workspace-internal
  dependencies of its own.

About 95% of the code is ML machinery with no counterpart in a check-in app.
Much of it is also wasm-hostile for our Worker (it pulls in rayon, Metal and
CoreML). The value for us is in how the repo is **run**, plus about 1.7 k lines
of blake3 integrity helpers.

**Never add `katgpt-core` as a dependency.** It carries about 322 k lines,
an unconditional rayon dependency and about 350 features, which would put the
3 MiB bundle cap at risk. Copy individual files, keeping the MIT notice.
Nothing we would want is published: `katgpt-device-verify`, `-proof-cert` and
`-pruners` are all `publish = false`.

## 2. Engineering process

| Mechanism | Where (katgpt-rs) | Worth it here? |
|---|---|---|
| **Close = HISTORY entry + delete the issue file, in one commit.** HISTORY entries read `## YYYY-MM-DD — Issue NNN CLOSED (<verdict>): <lesson>`. | `HISTORY.md:1-44`; commit `ecce691c` | **Yes.** We have 144 issue files on disk, and closed issues keep adding noise. |
| **Fixed terminal verdicts:** CLOSED, CLOSED NEGATIVE, PARKED (with named reopen triggers), PULL-GATED, DECLINED, and "instrument-broken, not claim-refuted". | `HISTORY.md:45-71, 867-892` | **Yes.** `issue_ledger.py` (`.issues/140`) could enforce this vocabulary. |
| **`.highwater` counter per directory** (read it, add one, write it back), plus a gate that catches two sessions taking the same number. | `.issues/.highwater`; `scripts/numbering_gate.py`, `dual_allocation_gate.py` | **Yes.** We already have collisions: issues 091/095/097, plan 014, handovers 001/124/126. |
| **BOUNDARY.md:** Owns / Does not own / May depend on / drift ledger. File the issue before the fix; the drift row is removed in the commit that closes it. | `BOUNDARY.md`; `AGENTS.md:9-18` | **Lite version.** One page covering what worker, frontend, escrow and D1 each own. |
| **A green result is not a whole-repo claim.** A gate that skipped an axis prints PARTIAL, never PASSED. "0 tests ran" counts as a failure. An EXIT trap can launder a `set -e` abort into exit 0. | `AGENTS.md:61-120, 1148, 1747` | **Yes.** Audit `scripts/verify/*.sh` for all three. |
| **A gate's failure path must be tested** (a negative test proves it can fail). | `AGENTS.md:2319`; `lean_proofs.yml:118` | **Adopted 2026-09-23.** See §6. |
| **Promotion review before turning something on by default:** G1 correctness, G2 perf (`--release`), G3 no regression, G4 alloc-free, in a numbered write-up. | `AGENTS.md:3143-3160`; `.benchmarks/675_*` | **Lite version**, for production toggles only, not every function. |
| **Peer-session hygiene:** stage named files only; add a `Session: <name>, <epoch>` commit trailer; `git fetch` and cross-run the peer's fixtures before recording a negative result. | `AGENTS.md:2920-3018, 3243-3262` | **Yes.** It matches our `[[check-for-peer-sessions-in-same-tree]]` memory. |
| **Mandatory "Honest caveats" section** in proposals; a second model reviews in AGREE/REVISE rounds (at most 3). | `.agents/skills/proposal/SKILL.md:289`; research skill §5 | **Yes, for risky plans** (escrow, merge strategy, money paths). |
| **Three-way retirement:** pending / kept for A/B / dead-and-exiled, with evidence links. | `.docs/10_audits/loser_sweep_audit.md:17-28` | Maybe, for stale experiment branches. |

**Do not copy:**
- The 3,400-line AGENTS.md. It is incident narrative living inside a rule file,
  and it grew back after being compacted once.
- The ~157 scripts, ~20 drift sweeps with floor files, and meta-gates.
- Per-primitive GOAT tiers and Lean proofs.
- The `main`-only CI (a spending-limit workaround).
- Committing straight to `develop` with no feature branches.

## 3. CI and tooling

What katgpt-rs does well:
- **CI is a thin wrapper around local scripts** (`scripts/full_gate.sh`,
  `docs_gate.sh`, `test_gate.sh`), so a local run and a CI run are the same
  command.
- **Every workflow lists its own file and its script in its path triggers**
  (`wasm32_gate.yml:44-51`).
- **Rust caches are keyed per target triple**, with
  `cache-on-failure: true` (`wasm32_gate.yml:93-98`).
- **A warm cache cannot produce a vacuous lint green.** It runs `cargo clean -p`
  on each workspace member after restoring the cache, then fails if zero units
  compiled (`full_gate.yml:160-166`, Layer 3b).
- **Minimum passed-test counts per target** that can only ratchet down, with a
  `--canary` mode proving the floors bite (`test_gate.sh:147-151`).
- **The toolchain is pinned, plus one stable-channel "rot" lane** that catches
  new deny-level lints early (`rust-toolchain.toml:22-24`, `full_gate.yml:89-101`).
  - The trap it records: dtolnay installs the wasm32 target into `stable`, but
    cargo then honours the pin, which lacks the target. Its first 5 CI runs
    failed on this (`wasm32_gate.yml:68-77`).

What katgpt-rs does **not** have:
- SHA-pinned actions, `permissions:` outside `release-plz.yml`, deny.toml or
  cargo-audit, gitleaks, CODEOWNERS, a bundle-size gate. We now have all six.

## 4. Perf and security craft

- **Counting allocator per test binary**, gated by a *feature*, not
  `debug_assertions`. Gating it on `debug_assertions` alone made `--release`
  runs empty ("0 passed"). Each test first proves the allocator is installed
  (`katgpt-core/src/alloc.rs:21-88`, `tests/cross_res_g4_zero_alloc.rs:29-43`).
- **`*_into(out: &mut [T])` APIs** that write into caller-owned buffers
  (about 286 files), with explicit scratch structs.
- **Interleaved median-of-ratios A/B timing** (`tests/common/ab_timing.rs`).
  Interleaving is needed because two back-to-back runs of the same work
  differed by up to 21.7%. It also varies the input per iteration and treats
  0 ns as an instrument failure, not a result.
  - Fat LTO removed timed work even through `black_box`: 7 of 34 audited
    regions "passed" on work that never ran (`AGENTS.md:592`).
  - Below about 100 samples, "p99" is just the maximum.
- **Pinned cross-implementation vectors.** One fixture is asserted by value in
  every consumer, and a generator binary rebuilds it. Tests also check the
  fixture itself, e.g. that it still reaches the fallback branch
  (`katgpt-device-verify/tests/vectors_pinned.rs`).
- **Total functions on wire input.** `checked_*` variants return `Option`;
  `verify_*` returns `false` on malformed input and never panics; proofs have a
  length bound (`verify_proof_bounded`, `MAX_PROOF_SIZE`). It recomputes
  derived values instead of trusting them ("verification consumes entropy,
  never produces it").
- **Snapshots are MAGIC + VERSION + BLAKE3 checksum**, with
  `blake3::derive_key("<ctx>-v1", …)` for domain separation
  (`katgpt-sense/src/serialize.rs:6-66`, `fair_roll.rs:171`).
- **Error enums are `Copy`** with structured fields that fail closed ("refuse
  rather than truncate").

It does not transfer:
- NEON/AVX dispatch, fat LTO for speed (we optimise for size), or Lean proofs
  of ML math.

## 5. Code we could actually reuse

All of these are blake3-only, wasm32-clean and MIT. `blake3` with
`default-features = false` is already in `worker/Cargo.toml`.

| # | Source | What it could give bethere | Effort |
|---|---|---|---|
| 1 | `katgpt-device-verify/src/fair_roll.rs` (no_std, forbid(unsafe), pinned vectors) | **A verifiable lucky draw.** The organiser commits `blake3(secret)` before check-in closes. The seed is `combine_seed(secret, Solana blockhash at close)`, and winners come from rejection sampling (widen `u8` sides to `u32`). Attendees can re-run the draw in the browser. | 1–1.5 d; **product decision** |
| 2 | `katgpt-core/src/content_store/merkle.rs` plus `device-verify/src/merkle_verify.rs` | **A tamper-evident check-in log.** Take a Merkle root over the check-in rows at event close, store it (optionally as a Solana memo), and show an inclusion proof on the ticket. Add `0x00`/`0x01` leaf/node prefixes when porting. **Not** compatible with the Bubblegum keccak tree. | 1–2 d; product |
| 3 | `katgpt-core/src/lthash.rs` (438 lines, alloc-free) | **An incremental, order-independent checksum** for the deposits/credits ledger. Update it in O(1) per write; the nightly cleanup recomputes it and alerts on drift. | ~1 d |
| 4 | `katgpt-proof-cert` (pattern only: it uses `SystemTime::now()`, which panics on wasm32) | A shape for the refund/escrow evidence trail: a certificate is valid only if its dependency chain is valid, with a checksummed bundle. | 0.5 d |
| 5 | `katgpt-pruners/src/count_min_sketch.rs` (2 KB fixed) | Heavy-hitter IP detection behind 401/429 alerts. Only useful inside a Durable Object; per-isolate memory is short-lived. | 0.5 d, optional |

Checked and **not** useful:
- `katgpt-moka-wasm` is a Go engine, not a cache.
- `katgpt-validator` checks Rust syntax.
- The bandits are tied to the search engine.
- The papaya LRU needs long-lived memory, which Workers don't have.
- `rate_control.rs` has the wrong shape for spike alerts; a 30-line fast/slow
  EMA is simpler.
- We already have a constant-time compare (`worker/src/crypto.rs`).

## 6. Adopted in this session (2026-09-23)

| katgpt-rs practice | Landed as |
|---|---|
| A gate's failure path must be tested | `security_headers_parity.rs` went red when XFO was flipped. `verify_security_headers` passes on staging and fails on prod `/`. The new `forbid(unsafe_code)` rejects a probe `unsafe` block. cargo-deny failed before the ignore list existed. |
| CI is a thin wrapper around local commands | `security.yml` runs exactly the `cargo deny`, `cargo audit` and `gitleaks` commands documented in `deny.toml` and `.gitleaks.toml` |
| One canonical home, detect drift instead of copying | `SECURITY_HEADERS` is a single table; `_headers` mirrors it and a parity test fails on drift. The escrow cargo-audit reads its ignore IDs from `deny.toml`. |
| Toolchain / supply-chain hygiene (we went further than katgpt-rs) | SHA-pinned actions, least-privilege tokens, concurrency, CODEOWNERS |
| `#![forbid(unsafe_code)]` (`katgpt-device-verify/src/lib.rs:59`) | Added to `domain` and `worker` |
