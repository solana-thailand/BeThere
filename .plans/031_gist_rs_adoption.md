# Plan 031: Adopt what transfers from gist-rs (riir-reflex, riir-infer, reflex-site)

**Created:** 2026-09-24 · **Study:** `docs/gist_rs_study.md`
**Rule:** as in `.plans/030`, nothing here changes shipped bytes or runtime
behaviour before RTM#6 (2026-09-27). Nothing is taken as a dependency.
reflex-site has no license, so its code is a pattern only.

## 1. Done 2026-09-24

- [x] Study written (`docs/gist_rs_study.md`), including an audit of 8
  headline claims.
- [x] `.issues/147`: public `/api/health` returned D1 row counts. Fixed on
  develop. This came from a side finding of the study.

## 2. Ungated (S)

- [x] **`scripts/verify/wasm_leak_scan.sh`** (2026-09-24). `--self-test` 8/8.
  Prod wasm: 116 paths; local dist: 112; no secrets. CI runs it report-only
  after the frontend size budget (actionlint clean; the first real run comes
  with the owner's push). Paths are reported and exit 0; secrets always exit 1.
  **Left:** scan the worker bundle too. The dry-run bundle lives in a temp dir
  inside `worker_size_budget.sh`, so it needs a `--keep` or a hook there.
  Original item: a port of
  `riir-reflex/scripts/binary_leak_scan.sh` (MIT). It checks absolute
  build-host paths and secret shapes in `frontend-leptos/dist/*.wasm` and the
  worker bundle, and has a `--self-test`. It starts report-only, because
  today's build has 116 paths.
- [x] **License hygiene** (2026-09-24):
  - **Gate:** `[licenses]` in `deny.toml` allows only permissive licences
    (MIT, Apache-2.0 ± LLVM-exception, BSD-2/3, BSL-1.0, CC0-1.0,
    Unicode-3.0, Zlib). First-party crates are now `publish = false` and are
    skipped by `[licenses.private]`. CI runs `check licenses` as its own step
    in the `advisories` matrix (workspace + frontend-leptos).
  - **A/B:** dropping `Zlib` gave 5 errors on the frontend graph; removing
    `publish = false` from `domain` gave `event-checkin-domain is
    unlicensed`. Both restored → `licenses ok` on both graphs.
  - **Notices:** `scripts/verify/third_party_licenses.sh` renders
    `THIRD_PARTY_LICENSES.md` with cargo-about 0.9.2 (`about.toml`,
    `scripts/licenses/about.hbs`), wasm32 only, no build/dev deps, offline.
    Two sections: worker bundle (126 crate-licence entries) and frontend dist (187).
    `--check` fails on drift; `--self-test` 4/4; two runs are byte-identical.
    CI job `third-party-notices` runs both.
  - **Left:** the notices are in the repo, not served. Shipping them in
    `dist/` (and the Content-Type Cloudflare gives `.md`) is a follow-up.
    `bethere-escrow` is not covered: its graph needs `quasar build`. The root
    `LICENSE` stays the owner's decision.
- [x] **`domain` import fence** (2026-09-24):
  `scripts/verify/domain_import_fence.py`, run in CI `build-test`.
  - **What it checks:** `domain`'s wasm32 normal graph (31 crates, all
    features) must not reach our app crates or platform crates (workers-rs,
    leptos*, web-sys, gloo*, solana-*, tokio, reqwest, axum).
  - **The chrono question:** chrono's default `wasmbind` *does* pull `js-sys`
    plus the 4 `wasm-bindgen*` crates. That is pinned, not fixed:
    `Utc::now()` on wasm32 needs it, so removing it would change runtime.
  - **The pin fails both ways:** a new bridge crate fails, and so does a
    pinned crate leaving the graph.
  - **Blindness floor:** exits 2 when `domain`, `serde` or `chrono` is
    missing, or the graph has fewer than 10 crates.
  - **Proof:** `--self-test` passes 11/11. A/B on real `cargo tree` output: a
    planted `web-sys` gave exit 1, and a removed `js-sys` gave exit 1 as a
    stale pin.
- [ ] **Bench-record rules** for `.plans/028` M1:
  - a numbered file per rung;
  - a correctness gate that is green before a number is quoted;
  - one session, interleaved;
  - retractions recorded in place;
  - published numbers generated, with a check.

## 3. After RTM#6

- [ ] `--remap-path-prefix` for `$HOME` and the rustup/cargo roots in both wasm
  builds, then make the leak scan blocking. Measure the brotli delta and open
  the staging page.
- [ ] Toolchain pin (`.plans/030` §3): declare `components` and `targets`.
- [ ] Build stamp on `/api/health`: git sha, `BUILD_TAG`, and "stale" when
  built outside `deploy.sh`.

## 4. Owner decisions

- [ ] Parity-before-use in the lucky-draw spec (`.plans/030` §4): the browser
  replays pinned server outputs bit-exactly before showing its own re-run.
- [ ] Whether to keep `dev_mode` and the Solana readiness block public on
  `/api/health` (`.issues/147`, "Not in scope").
