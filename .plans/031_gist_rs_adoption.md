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
  **Worker bundle** (2026-09-24): `worker_size_budget.sh --keep <dir>` keeps
  the dry-run bundle, and CI scans it report-only right after the budget
  step. The first local run found a false secret: `worker/src/crypto.rs:89`
  ships the bare `-----BEGIN PRIVATE KEY-----` label (the PKCS#8 parser), with
  `-----END…` right after it and no key body. The PEM arm now needs 32+ base64
  chars after the header (raw newline or JSON `\n`); CR/LF are flattened for
  the secret pass so a multi-line PEM still matches. `--self-test` 11/11
  (label-only → 0, raw and JSON-escaped keys → 1). Local worker bundle: 201
  paths, no secrets. Frontend dist: 112 paths, no secrets.
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
- [x] **Bench-record rules** for `.plans/028` M1 (2026-09-24):
  - **Rules:** `.benchmarks/README.md`. One numbered file per rung, allocated
    by `numbering_gate.py --next .benchmarks` (the directory is now in its
    `DIRS`). A green correctness gate comes before any number. Lanes are
    measured in one session, interleaved. The load is written down.
    Retractions are recorded in place. Numbers are quoted by citation. When
    n < 100, report the tail support instead of "p99".
  - **Check:** `scripts/verify/bench_records.py` (CI `build-test`). It checks
    each record's header (Status vocabulary, Gate with a command and
    `exit 0`, `vs … interleaved` lanes, sha, session), and that every
    `.benchmarks/NNN` citation in plans, issues, docs, README and CLAUDE.md
    resolves and does not quote a retracted record as live. `--self-test`
    passes 16/16.
  - **First run:** the check flagged 5 unprefixed citations of *other*
    repos' `.benchmarks/` in `docs/gist_rs_study.md` and
    `docs/katgpt_rs_study.md`. Those now carry the repo prefix
    (`riir-reflex/`, `katgpt-rs/`), which the citation pattern skips.
  - **Not automated:** "published numbers generated". Nothing here publishes
    numbers yet. The citation check is the enforceable part until something
    does.

## 3. After RTM#6

- [ ] `--remap-path-prefix` for `$HOME` and the rustup/cargo roots in both wasm
  builds, then make the leak scan blocking. Measure the brotli delta and open
  the staging page.
  **Built on branch `feature/031-remap-path-prefix` (`132d82be`, rebased to `9c13c05e` on 2026-09-30; 2026-09-28,
  session `event-checkin-82`), not merged** (same RTM #6 hold as plan 028 §5,
  since it changes shipped bytes).
  **Blocked (2026-09-29, `event-checkin-8c`):** open only on its merge sub-step below (owner go, demo freeze).
  - [x] `scripts/wasm_rustflags.sh` (self-test 3/3) appends the remaps to the
    caller's RUSTFLAGS. `frontend-leptos/build.sh`, the wrangler `[build]`
    command and the CI trunk step use it. Cargo's `trim-paths` would be
    simpler, but it is still unstable on 1.98.1.
  - [x] CI leak scans for both wasm builds are blocking.
  - [x] Same-tree A/B on `develop` `8c7da908`: frontend 112 build-host paths →
    0, brotli q4 −642 B (first load 1,826,763 B, within budget); worker 187 → 0,
    gzip +89 B (within budget). A trap while measuring: `frontend_size_budget.sh`
    with no flag reuses a fresh `dist/`, so the first "B" was really A's build.
  - [x] Opened locally (`wrangler dev --local`, headless Chrome): `/`, `/admin`
    and `/privacy` render with no page errors.
  - [x] Found: `worker/.cargo/config.toml`'s curve25519 `fiat` cfg was dropped
    whenever RUSTFLAGS was set, which includes CI's `-D warnings`. So CI built
    and size-measured the serial backend while local deploys shipped fiat. The
    cfg now lives in the build command, and the config file is gone.
  - [x] Rebased onto `develop` `04ab57b5` (2026-09-30, session
    `event-checkin-aa`): now `9c13c05e` (was `132d82be`, 139 commits behind,
    and `git merge-tree` showed 3 conflicts). Resolutions:
    - `build.sh`: both trunk builds (staff shell, then attendee) take the
      remap, computed once;
    - CI: `develop`'s `bash build.sh` step stays, so CI gets the remap
      through `build.sh`; the frontend leak scan stays blocking;
    - `wrangler.toml`: remap plus fiat cfg, without `CARGO_BUILD_JOBS=1`,
      which `develop` dropped on purpose (`0e1c6fae`).
    Checked on the rebased tree: `wasm_rustflags.sh` and `wasm_leak_scan.sh`
    self-tests, ShellCheck gate, and CI YAML parse all pass; a real
    `build.sh` build gives a clean leak scan over both shells' wasm (9 files;
    `develop`'s build has path hits). Size A/B, same session and target dir,
    br4 first load: attendee 1,215,099 → 1,215,306 B (+207), staff
    1,997,238 → 1,999,364 B (+2,126). The raw wasm shrinks (−2,313 / −2,797
    B) and so does br11 (−14 / −272 B); only brotli q4 packs the remapped
    strings worse. Both gates pass. The staff shell is past the warn line on
    `develop` too (95.23%), not because of this branch (`.plans/038`).
  - [ ] Merge after RTM #6, then open the staging page (owner-gated deploy).
    **Blocked (2026-09-29, `event-checkin-8c`):** owner go for the merge + staging deploy; after the 6–8 Oct freeze.
    **Owner question:** "may `feature/031-remap-path-prefix` (`9c13c05e`) merge into `develop` after the 8 Oct take and go to staging, at +2.1 KB br4 on the staff shell?"
- [ ] Toolchain pin (`.plans/030` §3): declare `components` and `targets`.
  **Built on branch `feature/030-toolchain-pin` (2026-09-28, session
  `event-checkin-00`), not committed or merged** (RTM #6 hold). Details and
  the open size A/B are in `.plans/030` §3.
  **Blocked (2026-09-29, `event-checkin-8c`):** see `.plans/030` §3; the commit is owner-pending and edits peer `event-checkin-16`'s files.
  - [x] `components = ["clippy", "rustfmt"]` and
    `targets = ["wasm32-unknown-unknown"]` declared in `rust-toolchain.toml`.
  - [ ] Merge after RTM #6, after the remap and build-stamp branches.
    **Blocked:** owner go; after the 6–8 Oct freeze and the two merges before it.
- [ ] Build stamp on `/api/health`: git sha, `BUILD_TAG`, and "stale" when
  built outside `deploy.sh`.
  **Built on branch `feature/031-health-build-stamp` (`2bc919f0`, 2026-09-28,
  session `event-checkin-82`), not merged** (RTM #6 hold).
  **Blocked (2026-09-29, `event-checkin-8c`):** open only on its merge sub-step below (owner go, demo freeze).
  - [x] `deploy.sh` exports `BETHERE_BUILD` as its existing provenance string
    (`git:<sha>[+dirty]`, the same one the Version message carries), and
    `/api/health` returns it as `build`. Any other build returns `"unstamped"`,
    which covers the "stale" case.
  - [x] Checked with `wrangler dev --local`: set → `"git:test-sha-1"`, unset →
    `"unstamped"`. Cargo rebuilt the worker on each change (`option_env!` is
    tracked), so no `build.rs` is needed.
  - [x] Guard in `worker/tests/public_health_no_counts.rs`; a mutant on the
    fallback string turns it red. Workspace clippy is clean, 103 binaries pass,
    and the floor went up by 1.
  - Not done: `BUILD_TAG`. It is a frontend constant, and the stamp's git sha
    already identifies the tree it came from. Also not done: a
    `post_deploy_smoke.sh` assertion on the stamp, because prod is unstamped
    until the first deploy that carries this change.
  - [ ] Merge after RTM #6; the first deploy then shows the stamp on prod.
    **Blocked:** owner go + prod deploy; edits `worker/` (peer `event-checkin-16`); after the 6–8 Oct freeze.
    **Rechecked 2026-09-30 (`event-checkin-aa`):** `git merge-tree` against `develop` `04ab57b5` is clean, so no rebase is needed yet. The peer clause is stale (`event-checkin-16` is gone). **Owner question:** "may `feature/031-health-build-stamp` merge after the remap branch, after the 8 Oct take?"

### reflex, after 12 Oct (research only)

Probed live 2026-09-28 (reflex 0.2.3): it abstained on slip and no-show
questions (confidence 0.009 and 0.0002) and routed them to its built-in `ops`
corpus. It serves on localhost only, has no public API for loading our data,
and publishes 0.22–0.51 accuracy. So nothing ships in the product.
- [ ] Offline A/B: replay RTM #6 W1 shadow-mode decisions through reflex and
  compare agreement with the deterministic checks. Record under `.benchmarks/`.
  **Blocked:** data; the RTM #6 (4 Oct) W1 decisions don't exist yet, and they live in prod D1 (owner access). Dated after 12 Oct.
- [ ] No-show prediction: can past check-in history predict a no-show well
  enough to matter for D1a's T-48h confirm (`docs/deposit-commitment-model.md`)?
  Needs labeled data from at least two deposit events first.
  **Blocked:** data; two labeled deposit events don't exist yet.

**Checked 2026-09-29 (session `event-checkin-4e`), nothing taken:** the
three held merges (remap, toolchain pin, build stamp) change every shipped
build, and two edit `worker/`/`deploy.sh`, which peer `event-checkin-16` is
working in; the toolchain branch is still uncommitted pending the owner. With
the 6–8 Oct demo freeze, merge them after the take, on an owner go. The reflex
items are dated after 12 Oct.

**Rechecked 2026-09-30 (session `event-checkin-c7`), nothing taken:**
- `feature/031-remap-path-prefix` (`132d82be`) and `feature/031-health-build-stamp` (`2bc919f0`) are still not ancestors of `develop`.
- `feature/030-toolchain-pin` still points at its base, `83241bd9`, and its work is still uncommitted in `/tmp/ec-pin`. `git merge-base --is-ancestor` reports it as "merged" only because the branch has no commits of its own.
- Peer `event-checkin-16` is no longer running, so the peer-area clauses above no longer apply.
- The gates that still hold are: an owner go for each merge (and a deploy for the stamp), the 6–8 Oct freeze, reflex data dated after 12 Oct, and the two owner decisions in §4.

## 4. Owner decisions

- [ ] Parity-before-use in the lucky-draw spec (`.plans/030` §4): the browser
  replays pinned server outputs bit-exactly before showing its own re-run.
  **Blocked:** owner; depends on the lucky-draw product decision in `.plans/030` §4.
  **Owner question:** the same as `.plans/030` §4, "does any event run a lucky draw that attendees need to audit?" A yes opens this item with the draw.
- [ ] Whether to keep `dev_mode` and the Solana readiness block public on
  `/api/health` (`.issues/147`, "Not in scope").
  **Blocked:** owner: security/product decision.
  **Owner question:** "should unauthenticated `/api/health` keep returning `dev_mode` and the Solana readiness block, or should they move behind staff auth and leave the public body as status only?"
