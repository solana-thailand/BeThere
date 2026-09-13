# 092 — `cargo fmt --all` never reached the frontend, and six files had drifted

**Status:** fixed 2026-09-13
**Found:** 2026-09-13, while shipping `.issues/091`
**Severity:** low (formatting only) — but it is a gate that reads as covering
more than it does, which is the part worth fixing

## What

CI runs `cargo fmt --all -- --check` from the repo root. `--all` means *all
workspace members*, and the root `Cargo.toml` says:

```toml
members = ["domain", "worker"]
exclude = ["frontend-leptos"]
```

So the gate covers `domain` and `worker` and silently skips the largest crate in
the tree. By today six files had drifted:

```
src/pages/deposit/types.rs   (3 hunks)
src/pages/landing/page.rs
src/pages/public/feedback.rs
src/utils/mod.rs
```

Two of them were introduced in the last hour by `.issues/091` — including one in
a file I had checked and called clean, because I read the first 20 lines of a
`--check` diff instead of all of it.

## The justification no longer held

The step carried its own reason for the scope:

> rustfmt hard-errors on whitespace-only lines inside Leptos `view!` macros,
> which would make a --check gate there fail on formatting it cannot itself fix.

That does not reproduce on the current toolchain, and the check is cheap:

| | result |
|---|---|
| `cargo fmt` in `frontend-leptos` | exit 0, **no stderr at all** |
| what it wrote | ordinary formatting — one import reorder, one blank line, three call-arg wraps. No `view!` macro touched |
| `cargo fmt --check` on a second pass | exit 0 — idempotent |
| `cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings` | clean |
| `cargo test` (host) | 116 + 55 + 9 + 8 + 5 + 3 passed, 0 failed |

A stale reason is worse than no reason: it is the thing that stops the next
person re-testing the assumption. The comment now records what was measured and
when, and says to delete the step rather than reformat around it if a future
rustfmt regresses on `view!`.

## The fix

1. `cargo fmt` applied to `frontend-leptos` — 4 files, formatting only.
2. A `cargo fmt --check` step added to the existing `frontend-clippy` job, which
   already has the crate as its working directory. `rustfmt` added to that job's
   toolchain components.
3. The workspace step's comment corrected to say what it actually covers and
   where the other crate is gated.

## Not covered, deliberately

`flow-harness` and `bethere-escrow` are also outside the workspace. Neither is
gated here — `flow-harness` is not in CI at all, and the escrow job runs its own
toolchain. Left alone rather than swept in: this issue is about a gate whose
stated scope was wrong, not about maximising fmt coverage.
