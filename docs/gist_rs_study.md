# What event-checkin can take from gist-rs (riir-reflex, riir-infer, reflex-site)

**Date:** 2026-09-24 · **Sources:** read-only clones in `~/src/gist-rs/`:
`riir-reflex` @ `374d9af`, `riir-infer` @ `b8f665e`, `reflex-site` @ `12b8d92`,
plus the live site `https://reflex.gist.rs/` and `gh run list` on the three
GitHub repos. Nothing was built. One read-only probe was run
(`node scripts/arena_head_parity.mjs`, §5 row 8), and the public staging
frontend wasm was downloaded once and scanned (§3).
**Companion:** `docs/katgpt_rs_study.md` (the upstream these repos consume).
Already adopted or queued from katgpt-rs is in `.plans/030`; this doc does not
re-propose those items.

## 1. What each repo is

| | riir-reflex | riir-infer | reflex-site |
|---|---|---|---|
| Purpose | A localhost "modelless decision engine" (`reflex` bin): typed `choice`/`score`/`noul` answers with calibrated confidence and first-class abstain, plus a benchmark harness comparing it with the **laya** encoder models (ModernBERT/mmBERT, Apache-2.0 weights downloaded at runtime) | LLM inference substrate: GGUF/safetensors loaders, quant formats, gemma/llama/deltanet architectures, a wgpu/CubeCL GPU crate | Static arena site on Workers static assets: install copy, bench tables, a live Tetris "arena", and a 72 KB zero-dep wasm game head |
| Size | ~17.9 k lines Rust, 143 `#[test]` | ~32.9 k (root `src/`) + ~161 k (`crates/riir-infer-gpu`) Rust, 817 `#[test]`; plus ~74.6 k lines of vendored, patched `wgpu-hal` and `cubecl-runtime` | ~1.6 k lines Rust (`wasm-head/`), ~4.3 k lines JS/HTML/CSS |
| License | **MIT** (`LICENSE`, © Todsaporn Banjerdkit); `THIRD_PARTY_LICENSES.md` via `cargo about` | **MIT** (`LICENSE`); vendored crates keep their MIT/Apache files | **No LICENSE file**; GitHub reports `licenseInfo: null`. The code is all-rights-reserved by default, which **blocks copying** anything from it, including `wasm-head/` |
| Activity | 23 commits, 2026-09-21 → 09-24, one author | 11 commits, 09-22 → 09-23, one author | 20 commits, 09-22 → 09-23, one author |
| Deps | `katgpt-core` (path, `../katgpt-rs`); optional tokenizers, gemm (rayon), metal, objc2 | `katgpt-core`, `-transformer`, `-speculative`, `-forward` (path); rayon, memmap2, wgpu 30 | wasm-head: none (blake3 for dev only) |
| Users | reflex-site (via `data/bench.json`); binary releases on `gist-rs/reflex`, a brew tap and a scoop bucket | `../riir-ai` (not public; per `AGENTS.md`) | public site; 0 stars and 0 forks on all three |

All three repos are days old. riir-reflex and riir-infer only build with a
sibling checkout of `katgpt-rs` at `../katgpt-rs`; CI clones its `develop`
branch **unpinned** (`riir-reflex/.github/workflows/ci.yml:17-27`). Neither
crate is on crates.io (`publish = false`; HISTORY records the choice as
deliberate). Neither can go into the Worker: `katgpt-core` carries rayon and
~322 k lines (see the katgpt study §1), and the laya lane needs ~650 MB of
weights.

## 2. Engineering process, CI and gates

| Mechanism | Where | Worth it here? |
|---|---|---|
| **Artifact leak scan:** fail if a shipped binary has a symbol table, absolute `/Users/`/`/home/`/drive-letter paths, or secret-shaped strings (`ghp_`, `AKIA`, `sk-`, `xox?-`). Tunings are written down, and so are the things it deliberately does not check. | `riir-reflex/scripts/binary_leak_scan.sh` (80 lines, POSIX sh) | **Yes.** It found a real leak in our staging wasm (§3). |
| **Import fence with membership pins.** Path deps must resolve inside an allowlist; `.rs` files are scanned for foreign crate names after comments and strings are masked. Pins fail in **both** directions (an undefended finding, or a stale pin). A file-walk floor exits 2 on "blindness". `--self-test` runs on every invocation. | `riir-infer/scripts/fence_gate.py` (13.6 KB) + `fence_expected.txt` | **Lite version** for `domain`'s contract of no worker/frontend deps (§6 #4). The blindness floor is the new idea; we already self-test gates. |
| **Final line says PARTIAL** when a layer was skipped, and names the skipped layer | `riir-reflex/scripts/ci_feature_guard.sh:50-80` | Already adopted (`.plans/030` §2 audit). |
| **Canary before zero:** the counting allocator must count a known allocation before a "0 allocs" result is trusted | `riir-reflex/benches/decision_set_goat.rs:112-119` | Already queued (`.plans/030` §3, counting-allocator item). |
| **Percentile with tail support:** p99 is printed as "(tail support 3/300)" | `decision_set_goat.rs:151-168`; TABLES.md `p99 (support)` column | Already queued (030 §3 A/B helper). |
| **Generated numbers only:** "a hand-typed number on the site is a defect"; `publish_bench.py` copies harness JSON and drops machine-local fields (`datasets_dir`) | `reflex-site/scripts/publish_bench.py`, `README.md` | **Take the lesson, not the rule.** The site's own homepage teaser is hand-typed and has been stale since the seed commit `ab48eb5` (§5). A rule without a check rotted within a day. |
| **Toolchain pin that declares components** (`clippy`, `rustfmt`); commit `6688b59` records every CI lane going red without them | `riir-reflex/rust-toolchain.toml` | Add to 030's queued toolchain pin (post-RTM#6): list `components` **and** `targets`. |
| **Third-party notices** generated by `cargo about` into each release archive | `riir-reflex/scripts/build-release.sh:32-36`, `.github/about/` | **Yes** (§6 #3). Our `deny.toml` has only `[advisories]`, and event-checkin has no root `LICENSE` either. |
| **Browser prod smoke** with Playwright against the live page | `reflex-site/scripts/arena_prod_smoke.mjs` | No. We already have `post_deploy_smoke.sh` plus the open-the-page rule. This script also only *prints* page errors (exit is 0 if the flow passes) and hard-codes `/Users/katopz/...`. |

**Weaker than ours; do not copy:**
- Actions are tag-pinned (`@v4`, `@v2`), not SHA-pinned.
- No `permissions:` blocks, no cargo-deny, cargo-audit or gitleaks.
- `riir-infer` CI runs on `main` pushes only.
- reflex-site has no CI at all.
- The live site sends **no CSP, XFO or HSTS** headers (checked with `curl -sI`
  on 2026-09-23, 17:38 UTC).
- The CI test step passes with **three 0-test binaries** (two bins and the
  doc-tests), and the G5 parity and G2/G4 bench gates never run in CI (run
  `35896236010`). Our `test_count_floor.py` would flag the first case.
- riir-infer commit `eb3438f` fixed a test that was "arithmetically
  unsatisfiable… never green on any backend". That test existed but was
  never executed.

## 3. Perf and security craft for a Workers/WASM/Leptos app

**Finding: build-host paths in our shipped wasm.**
- I downloaded the public staging frontend (`event-checkin-frontend-954ff6d3b31087b2_bg.wasm`,
  5,490,729 bytes decoded) and ran the leak-scan regexes over it.
- It contains **112 unique absolute paths** (~12 KB raw), e.g.
  `/Users/ozone/.rustup/toolchains/1.97.1-aarch64-apple-darwin/...` and
  `/Users/ozone/.cargo/registry/src/.../leptos_...`. These are panic-location
  strings from std and dependencies.
- Paths from two toolchains appear (`1.97.1` and `stable`), which suggests the
  build is not on one pinned toolchain (inferred).
- No secret-shaped strings were found. Repo-relative paths from our own crates
  are not absolute, so they were not flagged.
- Fix: pass `--remap-path-prefix` for `$HOME` (and the rustup/cargo roots) in
  both wasm builds. It saves a few KB after brotli (unmeasured) and stops
  publishing the builder's username and toolchain.
- The Worker bundle is not downloadable, so it was not scanned; it probably has
  the same paths (unverified).
- Constraints: 3 MiB bundle and 10 ms CPU are both unaffected, since this is a
  build flag.

**Browser-side inference (the wasm head).**
`reflex-site/wasm-head/` is `no_std` on wasm32, has zero dependencies and no
wasm-bindgen, and exposes a C ABI (`head_init`, `head_alloc`, `head_score`).
- Size: `opt-level=3`, `strip`, `panic=abort`, then `wasm-opt -Oz
  --enable-bulk-memory`. That gives 72,004 bytes raw and **22,608 bytes** as
  served (br, measured with curl).
- Startup: at boot it **re-runs the ridge fit** in the tab and refuses to start
  unless the published anchor (44/120) reproduces.
- The pattern that transfers is **parity before use** (`assets/arena_head.js:39-96`).
  The page replays every recorded (input → f32 output) pair and requires
  bit-exact agreement (`Math.fround` equality) before the live path turns on.
  If any check fails, the page keeps the server-recorded result.
- For bethere this fits exactly one queued item: the lucky-draw re-run in the
  browser (`.plans/030` §4). It extends 030 §3's golden vectors to the client
  side.
- Constraints: client-side only, so the Worker's 10 ms and 3 MiB caps do not
  apply. The cost is frontend transfer.
- Do **not** copy the code: the repo has no license, and `lib.rs:66-75` uses
  `static mut` plus `unsafe`, which conflicts with our
  `#![forbid(unsafe_code)]`. Its MIT upstream is `riir-reflex/src/game_heads.rs`,
  but that is Tetris-specific.

**Benchmark and kernel-ladder method (for `.plans/028` M1).**
The riir-Metal ladder in `.benchmarks/001_phase1_harness.md`, addenda 2-7, and
commit `374d9af` has a good shape:
1. Record a v1 baseline.
2. Keep a correctness gate (G5 parity) green at **every** posture before a
   number is published.
3. Measure lanes in the same session, interleaved.
4. Write down box load.
5. Record a retraction in the bench file when an earlier claim was a load
   artefact (addendum 1: the port was first claimed faster than torch).

Their own lapses mark the limits:
- The latest rung lives only in a commit message.
- The README chart mixes columns measured on different days while labelling
  them "same-session".

For us the gate is `wrangler tail` `cpuTime` on staging, not native wall time.

**Not transferable:**
- The whole ML stack: laya, riir-infer's GPU kernels, and the modelless engine,
  which is a text classifier with no bethere use case.
- The localhost CORS/Private-Network-Access seam (`riir-reflex/src/serve.rs:26,
  439-552`): our API is same-origin.
- Fat LTO: we optimise for size.

## 4. Code that could be reused

| Source | Lines | Deps | License | Use for bethere |
|---|---|---|---|---|
| `riir-reflex/scripts/binary_leak_scan.sh` | 80 | POSIX sh, `nm`, `grep` | MIT | Port as `scripts/verify/wasm_leak_scan.sh`. Drop the `nm` arm (wasm has a name section instead: check that `strip` removed it), keep the path and secret arms, and add `--self-test`. |
| `riir-infer/scripts/fence_gate.py` | 349 | python3 stdlib | MIT | The skeleton for a `domain` fence: comment/string masking, pins that fail in both directions, the blindness floor. |
| `riir-reflex/.github/about/about.{toml,hbs}` + `build-release.sh --licenses` | small | `cargo-about` | MIT | Generate `THIRD_PARTY_LICENSES.md` for the served wasm. |
| `reflex-site/scripts/publish_bench.py` | 69 | stdlib | **none** | Pattern only (sanitize meta keys before publishing). Nothing to copy. |
| `reflex-site/wasm-head/`, `assets/arena_head.js` | 1,579 + 129 | none | **none** | Pattern only (parity-before-use). Unlicensed, and uses `unsafe`. |

Nothing else transfers: all the remaining Rust is ML-specific or katgpt-bound.

## 5. Audit of headline claims

| # | Claim (where) | Evidence traced | Verdict |
|---|---|---|---|
| 1 | "G5 laya parity 88/88, top-1 1.000, drift ≤ 3.1e-6" (README gates table; site badge) | `tests/laya_riir_parity.rs` asserts top-1 ≥ 99.9% and drift ≤ 1e-3 against a BLAKE3-pinned fixture (`tests/fixtures/laya_parity_v1.jsonl`); 26+26+36 = 88 forwards | **Backed**, workstation-only (needs weights; not in CI). The 3.1e-6 is the **CPU** posture (addendum 3). The now-default **Metal** posture recorded 5.98e-6 (addendum 6). Both are within the gate. |
| 2 | "G2 p99 0.06 ms per 8-question set" | `benches/decision_set_goat.rs` asserts p99 ≤ 1 ms. Workload: 2 experts with one short doc each, the **same** request repeated 300×, tail support 3 | **Toy-scale**. The 0.06 figure is in no file; addendum 5 records p99 53 µs. Not run in CI. |
| 3 | "G4 zero allocs on the hot path" (site badge) | Same bench, lines 175-186, with a live canary | **Backed but narrower.** It covers `solve_into` only; wire materialization is "excluded by design", and so is the `decide_with` path that G2 times. |
| 4 | 15-suite results table, e.g. typed 0.3190 vs 0.7445, "regenerated by `harness_tables.yml`, never hand-typed" (README) | The README links to `.benchmarks/001_phase1_tables/TABLES.md`. At HEAD that file is a **2-suite partial** (ag_news and banking77, laya capped at 100 questions, run on `m3`). The 15-suite numbers match `reflex-site/data/bench.json` (`777f6b6`, 16:00Z, host `m3`). | **Partly.** The numbers can be traced, but the link is stale and the CI provenance is unevidenced: the committed tables came from a local box, and the workflow is dispatch-only. |
| 5 | G1 calibration: README "PASS 7/14 · FAIL 2 · NO CLAIM 5"; site badge "**8/9 suites**" | `bench.json` has `g1_pass: true` on 6 of 14 suites. Addendum 7 gives 7/2/5 from a later run. | **Inconsistent**: three counts across three surfaces. The site badge is hand-typed. |
| 6 | Homepage teaser "0.475 ms modelless p50", typed 0.278 / AG News 0.258 / laya 4493 ms (`reflex-site/index.html:106-115`) | Unchanged since the seed commit `ab48eb5`. `bench.json` says 0.319 / 0.51. | **Stale**, and it contradicts the site's own "hand-typed = defect" rule. |
| 7 | "riir Metal 28.3/28.1/12.6 ms, beats torch MPS on multilingual" (README chart) | Only in the README, `AGENTS.md:292` and commit `374d9af`. There is no Bench 001 addendum. The torch and candle columns come from addendum 6 (09-22); candle is frozen. | **Partly.** A real ladder with parity reported green, but it is **not** the "same-session interleaved" run the label claims, and the baseline differs (commit 77.1 vs addendum 79.0). |
| 8 | Game head "44/120 in-corpus and LOO"; "836/836 bit-exact, ~1 µs/decision" in-tab (site, arena) | 44/120 is in `katgpt-rs/.benchmarks/881_template_decode_losslessness.md:53` and asserted in `tests/game_heads_serve.rs` and `wasm-head/src/gen.rs:227`. **I re-ran** `arena_head_parity.mjs` on Node 24: PASS 836/836, boot 2.5 ms, ~1.5 µs/decision. | **Backed.** Two caveats: 44/120 measures *agreement with laya's pick* (imitation), not Tetris skill, and the multiplier over chance is "2.7×" in the README but "3.4× the 10.8% floor" on the site. 3.4× follows from Bench 881's constant-pick 13/120; I could not trace 2.7×. |

**Summary:**
- These repos are more honest than katgpt-rs's README. The losses are published,
  retractions are recorded, NO CLAIM is kept separate from FAIL, and tail
  support is printed.
- Still, 5 of 8 numbers are partly backed, stale or inconsistent.
- None of the 8 is regression-gated in CI.

## 6. Verdict and ranked adoption list

**Verdict:** there is nothing to depend on. riir-reflex and riir-infer are
katgpt-bound ML stacks that don't fit wasm or the Worker, and reflex-site is
unlicensed. The value is in three small MIT gates, one browser pattern, and a
concrete leak their scanner exposed in our own staging build.

"Gated" means it waits until after RTM#6 (2026-09-27), because it changes a
shipped artifact or runtime behaviour.

| Rank | Item | Size | Gate |
|---|---|---|---|
| 1 | **`scripts/verify/wasm_leak_scan.sh`** (port of `binary_leak_scan.sh`): absolute paths plus secret shapes over `frontend-leptos/dist/*.wasm` and the worker bundle, with `--self-test`. Report-only in CI at first; today it would go red on the 112 paths. | S | **Ungated** (CI/script only) |
| 2 | **`--remap-path-prefix`** for `$HOME`, rustup and cargo roots in both wasm builds, then make #1 blocking. Measure the brotli delta and verify by opening the staging page. | S | **Gated** (changes shipped bytes) |
| 3 | **License hygiene**: `cargo about` → `THIRD_PARTY_LICENSES.md` for the served wasm, and `[licenses]` in `deny.toml`. The missing root `LICENSE` for this public repo is an **owner decision**. | S | Ungated (CI/docs); the LICENSE choice is the owner's |
| 4 | **`domain` import fence**: `cargo tree -p event-checkin-domain --target wasm32-unknown-unknown -e normal` must not reach `worker`/`frontend-leptos`, pinned in both directions, with a blindness floor. Verified 2026-09-24: `chrono`'s default `wasmbind` does pull `js-sys` + `wasm-bindgen*` on wasm32. They are pinned (see `.plans/031`). | S | Ungated |
| 5 | **Toolchain pin detail** for 030 §3's queued pin: declare `components = ["clippy","rustfmt"]` **and** `targets`. §3 found two toolchains in one staging build. | S (note) | Gated (already post-RTM#6 in 030) |
| 6 | **Parity-before-use** folded into the lucky-draw spec (030 §4): the browser replays pinned server outputs bit-exactly before showing its own re-run, and falls back to the server result otherwise. | S (spec) / M (build) | Owner decision (product) |
| 7 | **Bench-record rules for 028 M1**: numbered file per rung; correctness gate green before a number is quoted; one session, interleaved; retractions recorded in place; published numbers generated with a check (rows 5-6 show the rule alone rots in a day). | S (docs) | Ungated |
| 8 | Build stamp on `/api/health` (git sha, `BUILD_TAG`, "stale" if built outside `deploy.sh`), after `riir-reflex/src/build_stamp.rs`. Today health shows no build identity. | S | **Gated** (runtime) |

**Not adopting:**
- the modelless engine, laya, riir-infer and the GPU kernels;
- the localhost CORS/PNA seam;
- the install-copy parity check (we have the headers parity test);
- dispatch-only table CI;
- Playwright prod smoke;
- the wasm-head code (unlicensed, `unsafe`).

A standalone no-bindgen Rust QR decoder to replace self-hosted jsQR (53 KB br)
is an **unmeasured** idea. It could only land as a separate crate outside
`forbid(unsafe_code)`, so I don't recommend it without a size and speed probe.

## 7. Checked and acted on by the parent session (2026-09-24)

- **Prod wasm leaks too.** `/event-checkin-frontend-9ca2d4c145396600_bg.wasm`
  on prod holds 116 unique `/Users/...` paths (75 under `~/.cargo/registry`,
  41 under `~/.rustup/toolchains`). Low severity: the repo and `Cargo.lock`
  are public. The fix (#2) waits for RTM#6.
- **The `/api/health` aside was real, and worse.** Prod also returned row
  counts, and each hit read ~890 D1 rows. Fixed on develop as
  `.issues/147` (`SELECT 1`, no counts).
- The adoption queue is `.plans/031_gist_rs_adoption.md`.
