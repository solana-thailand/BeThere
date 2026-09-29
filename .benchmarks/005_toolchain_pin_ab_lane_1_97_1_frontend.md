# 005 — Toolchain pin A/B, lane A only: frontend first load on rustc 1.97.1
Status: valid
Rung: frontend first-load bytes (raw, gzip-9, brotli q4 and q11) of the release `dist/` built on rustc 1.97.1, as lane A of the 1.97.1 vs 1.98.1 pin comparison (030 §3 compat note "re-measure the size budgets", 031 §3 toolchain pin)
Session: event-checkin-00, 1790609429
Commit: 83241bd9 plus the uncommitted `feature/030-toolchain-pin` tree in worktree `/tmp/ec-pin` (no Rust source changes; `rust-toolchain.toml` set to `channel = "1.97.1"` for this lane)
Gate: `bash scripts/verify/frontend_size_budget.sh --dir /tmp/pin_ab/1.97.1/dist` exit 0 (the `trunk build --release` before it also finished, in 2m 17s)
Lanes: single
Load: M5 Pro laptop, AC power, `CARGO_TARGET_DIR=~/.cargo/target-pin` (not the shared target), sccache on; peer-session load not checked (size, not timing, so load does not change the bytes)

## Method

`/tmp/pin_ab/run.sh 1.97.1 1.98.1`. For each lane it sets the toml channel,
runs `frontend-leptos/build.sh` (which sources `scripts/pinned_toolchain.sh`,
so the shell's `RUSTUP_TOOLCHAIN` is unset and the toml decides), copies
`dist/` aside, and runs `frontend_size_budget.sh --dir` on the copy. It then
runs `worker_size_budget.sh --keep`. The build log's first line was
`🦀 rustc 1.97.1 (8bab26f4f 2026-07-14)`, so the toml, not the env var,
picked the compiler.

## Result

| file | raw | gzip | br4 | br11 |
|---|---:|---:|---:|---:|
| `event-checkin-frontend-*_bg.wasm` | 5,655,306 | 1,928,121 | 1,704,756 | 1,331,784 |
| `event-checkin-frontend-*.js` | 79,089 | 12,017 | 12,361 | 10,079 |
| `index.html` | 14,045 | 5,252 | 5,139 | 4,618 |
| **total, 38 files** | 6,164,304 | 2,046,312 | **1,827,599** | 1,431,501 |

- First load: 1,827,599 B at br4, 87.14% of the 2,097,152 B product budget,
  so it passes with 269,553 B of headroom.
- Against the committed baseline (1,824,024 B, recorded 2026-09-28) the delta
  is +3,575 B, inside the +25,600 B warn threshold. The baseline's tree and
  toolchain are not recorded next to it, so this delta cannot be pinned on the
  toolchain. Only lane B, run on this same tree, can show that. Lane B
  (1.98.1) did not run; see Notes.

## Notes

- **Lane B did not run, and the worker was not measured in either lane.**
  `worker_size_budget.sh` failed at `wrangler deploy --dry-run` because the
  worktree has no `worker/node_modules`, a known worktree trap
  (the main checkout's must be symlinked in). Under `set -o pipefail` the script
  stopped there, before the 1.98.1 lane and before it restored the toml.
  The toml was put back to `channel = "1.98.1"` by hand and read back.
- To finish the comparison, symlink `worker/node_modules` from the main
  checkout into the worktree, then re-run both lanes in one session (rule 3).
  Record the result as a new rung, not an edit to this one.
