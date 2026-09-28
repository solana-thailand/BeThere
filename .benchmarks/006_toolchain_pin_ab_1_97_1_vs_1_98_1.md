# 006 — Toolchain pin A/B: frontend and worker bytes, rustc 1.97.1 vs 1.98.1
Status: valid
Rung: frontend first-load bytes (raw, gzip-9, brotli q4 and q11) and worker bundle bytes (raw, gzip) built on rustc 1.97.1 vs 1.98.1, same tree (030 §3 toolchain pin, "re-measure the size budgets"; 031 §3 toolchain pin). Completes `.benchmarks/005`, which had lane A's frontend only.
Session: event-checkin-00, 1790609429
Commit: 83241bd9 plus the uncommitted `feature/030-toolchain-pin` tree in worktree `/tmp/ec-pin` (no Rust source changes; only `rust-toolchain.toml` `channel` differs between lanes)
Gate: `bash scripts/verify/frontend_size_budget.sh --dir /tmp/pin_ab/<lane>/dist` exit 0 and `bash scripts/verify/worker_size_budget.sh --keep /tmp/pin_ab/<lane>/worker` exit 0, in every pass (the `build.sh` and `wrangler deploy --dry-run` builds before them also exited 0)
Lanes: 1.97.1 vs 1.98.1, interleaved (A, B, A in one session; bytes, so one pass per slot)
Load: M5 Pro laptop, AC power, `CARGO_TARGET_DIR=~/.cargo/target-pin` (not the shared target), sccache on, peer sessions idle (size, not timing, so load does not change the bytes)

## Method

`/tmp/pin_ab/run.sh`, run as `1.97.1 1.98.1`, then again as `1.97.1`. This
time the main checkout's `worker/node_modules` was symlinked into the
worktree. For each lane the script:

1. sets the toml channel;
2. runs `frontend-leptos/build.sh`, which sources `scripts/pinned_toolchain.sh`,
   so the shell's `RUSTUP_TOOLCHAIN` is unset and the toml picks the compiler;
3. copies `dist/` aside and runs `frontend_size_budget.sh --dir` on it;
4. runs `worker_size_budget.sh --keep`.

Each step's exit code is logged, and the toml goes back to 1.98.1 on exit. The
build logs opened with `rustc 1.97.1 (8bab26f4f 2026-07-14)` and
`rustc 1.98.1 (48a229cea 2026-09-01)`.

## Result

| measure | 1.97.1 pass 1 | 1.98.1 | 1.97.1 pass 2 | B − A |
|---|---:|---:|---:|---:|
| frontend first load, br4 (38 files) | 1,827,599 | **1,826,829** | 1,827,597 | −770 B (−0.04%) |
| frontend wasm, raw | 5,655,306 | 5,662,478 | 5,655,306 | +7,172 B |
| frontend wasm, br4 | 1,704,756 | 1,703,991 | 1,704,756 | −765 B |
| frontend total, br11 | 1,431,511 | 1,431,813 | 1,431,482 | ≈ +300 B |
| worker bundle, raw | 4,875,676 | 4,872,350 | 4,875,676 | −3,326 B |
| worker bundle, gzip | 1,662,356 | **1,662,521** | 1,662,356 | +165 B (+0.01%) |

- **The bump from 1.97.1 to 1.98.1 does not change the size budgets.** The
  frontend passes on both lanes: 87.10% of the product budget on 1.98.1 and
  87.14% on 1.97.1. The worker passes on both: 52.85% of the 3 MiB
  free-plan ceiling on 1.98.1 and 52.84% on 1.97.1.
- Lane A's frontend wasm is byte-identical across both passes (same sha256),
  and pass 1 matches `.benchmarks/005` to the byte.
- The worker baseline delta, +88,235 B against the 2026-09-22 baseline, is the
  same on both lanes. It is tree growth since that baseline, not toolchain.

## Notes

- **Noise floor: the builds are not fully reproducible.** Between the two
  lane-A passes:
  - **Frontend:** Trunk wrote the `modulepreload` `<link>`s in `index.html` in
    a different order (same set, same SRI). That moves the br4 size by 2 B and
    the br11 size by 29 B. Because `build.sh` derives the SW `CACHE_VERSION`
    from a hash of `index.html`, the SW cache version also changes on every
    rebuild of identical assets.
  - **Worker:** the worker wasm had the same size but a different content
    hash in its file name, and 1 B more or less of gzip. **Corrected
    2026-09-28:** "identical raw bytes" was wrong. 2,166 bytes differ, all
    within offsets 3,963,476–3,965,877, and that range is the frontend
    `index.html` the worker embeds (`worker/src/lib.rs:68`, `include_str!`).
    Swapping lane A's `index.html` into lane B's wasm makes it byte-identical
    to lane A (SHA-1 `d2868841…`). So it is the same Trunk `modulepreload`
    order, not rustc or wasm-bindgen. Fixed by the same `build.sh` sort
    (plan 028, under F1).
  - So differences of a few bytes are noise. The −770 B / +165 B above are
    real but negligible.
- The frontend CSS here is the pin tree's (cut at `83241bd9`), not the
  `backdrop-filter` reorder on `develop`. It is unminified, and the reorder
  swaps two lines, so the size is the same.
