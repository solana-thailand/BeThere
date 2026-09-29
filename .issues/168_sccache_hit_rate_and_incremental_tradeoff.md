# 168: sccache works, but only for the same target-dir path, and `incremental = false` costs the edit loop

**Status:** closed (2026-09-29, session `event-checkin-ba`). Decision: keep sccache and correct the misleading comment in `~/dotfiles/cargo/config.toml`. The global `incremental = false` trade-off is the owner's call (see "Open question"). This was D3 of the GOAT-hardening handoff.

## Claim checked

The global cargo config said sccache caches "across projects AND across
target switches (host <-> wasm32)". The lifetime stats showed a 1.30% hit rate
(75 hits / 5,680 misses). The 10 GiB cache was full.

## Experiment (2026-09-29, no other cargo running)

`cargo check --workspace --locked`, cold, stats zeroed before each run:

| Run | Target dir | Rust hits | Rust misses | Wall |
|---|---|---|---|---|
| 1 | `/tmp/ec-ba/sc1` (fresh) | 0 | 135 | 16.3 s |
| 2 | `/tmp/ec-ba/sc2` (fresh, different path) | 0 | 135 | 16.3 s |
| 3 | `/tmp/ec-ba/sc1` again, after `rm -rf` | **135** | 0 | **9.5 s** |

42 calls per run are non-cacheable by design (35 `crate-type`, i.e. bin,
proc-macro and cdylib crates; 4 `missing input`; 3 other).

Running `cargo check` twice on an unchanged tree, as the handoff suggested,
measures nothing. Cargo's fingerprints skip every crate, so sccache gets zero
requests.

## Why the lifetime rate is 1.3%

1. **The key includes the absolute target-dir path** (`--out-dir`, `-L`,
   `--extern`). Agent sessions and worktrees use their own
   `CARGO_TARGET_DIR` (`target-5f`, `target-31`, …), so each one starts at 0%.
2. **The target triple is in the key.** Host and wasm32 builds never share
   entries, so the "across target switches" claim was wrong.
3. **On the shared `~/.cargo/target`, cargo's own fingerprints skip unchanged
   crates.** What reaches sccache is mostly real changes, which miss by
   definition.
4. **The cache is full** (10 GiB of 10 GiB) and shared by every Rust project
   on the machine, so LRU eviction also removes entries.

## What changed

- `~/dotfiles/cargo/config.toml` (the target of the `~/.cargo/config.toml`
  symlink): the comment now states the real keying and the numbers above.
  Only the comment changed. This file is not in this repo and nothing was
  committed there.

## Open question (owner)

`incremental = false` is set globally so that dev builds stay cacheable. D1
measured what that costs: a one-line view edit in the frontend took 73 s
without incremental and 10 s with it (and without sccache).
`frontend-leptos/serve.sh` opts out for the frontend loop. Given points 1–4,
local sccache mostly misses anyway. Dropping `rustc-wrapper` and
`incremental = false` globally would likely speed up every edit loop on this
machine. It would cost cold rebuilds in the same path about 7 s per workspace
check. This affects every project, so it's not changed here.
