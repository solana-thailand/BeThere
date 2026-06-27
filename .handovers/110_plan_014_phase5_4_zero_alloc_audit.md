# Handover 110 — Plan 014 Phase 5.4 Zero-Allocation Hot-Path Audit

> Branch: `feature/014_zero_alloc_audit` → `develop`
> Date: 2026-06-27
> Plan ref: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 5.4

## 1. What Happened

Plan 014 Phase 5.4 — the **zero-allocation hot-path audit** on the wire decode
path from Phase 1 — is complete. The audit followed the same audit-first
discipline that Phase 4.3 used: measure reality before claiming an outcome,
then ship assertions that match the measurement.

The plan's literal text asked for:

> `#[cfg(feature = "alloc_count")]` + a test asserting 0 allocs after warmup.

This was implemented as-shipped, after a measurement pass confirmed the
assertions are achievable.

### The audit question

Does the Phase 1 wire decode path (`domain/src/wire.rs::unpack` /
`unpack_slice`) perform zero heap allocations on the happy path? The decode
path is what runs in the Leptos frontend (compiled to `wasm32`); the encode
path runs on the worker (Rust). katgpt-rs measures allocs/call on every hot
kernel — Phase 5.4 is the equivalent discipline applied to BeThere.

### Initial hypothesis (proven WRONG by the audit)

I started the audit expecting one of two outcomes:

1. `unpack<T>` (single value, payload 16–72 B): zero-alloc — fits in one
   BLAKE3 chunk (64 B).
2. `unpack_slice<T>` (batch, payload up to 8 KB+): **non-zero allocs** —
   BLAKE3's chunk-merge tree has to materialise sub-tree chaining values
   for multi-chunk inputs, which I assumed grew an internal `Vec`.

This is the assumption baked into the existing `domain/src/wire.rs:135`
doc comment: *"On the happy path the only allocations are the BLAKE3 hasher's
internal scratch (one-time)."*

The audit disproved this. **Every shape is zero-alloc in steady state.**

### Why blake3 doesn't allocate

blake3 1.8.5's `Hasher` struct uses `InlineSubCtxStack` — a **fixed-size stack
array** of chaining values, not a `Vec`. For any input whose chunk count
satisfies `2^depth ≥ chunks`, the stack never grows the heap. The depth is
large enough to hash petabyte-scale inputs without ever allocating. Confirmed
empirically up to 10000 rows × 16 B = 160 KB (~2500 chunks): zero allocations.

`bytemuck::cast_slice` and `bytemuck::from_bytes` are pure pointer reinterprets
(a bounds check + a cast) — no allocation. The decode path therefore has
**zero heap allocations** in steady state on every realistic input.

## 2. Changes (3 edits, 1 new file)

### New: `domain/tests/alloc_count.rs` (293 lines)

Installs a counting global allocator (gated behind
`#[cfg(feature = "alloc_count")]`) and asserts 0 allocations after warmup
across 10 decode shapes:

- `unpack<LevelScore>` single value
- `unpack_slice<LevelScore>` at 0 / 1 / 3 / 4 / 50 / 500 / 10000 rows
- `unpack_slice` multi-chunk boundary (4 rows × 16 B = 64 B payload)
- 100× repeated `unpack<LevelScore>` (per-call regression guard)
- 100× repeated `unpack_slice<LevelScore>` 500 rows (slice regression guard)

The counting allocator wraps `std::alloc::System` and increments a process-
global `AtomicUsize` on every `alloc` / `alloc_zeroed` / `realloc`-grow. The
test file uses Rust 2024's `unsafe-op-in-unsafe-fn` discipline — every call
into `System.alloc` etc. is wrapped in an explicit `unsafe { ... }` block.

Tests must be run with `--test-threads=1` because the counter is process-
global; parallel tests would cross-contaminate each other's measurements.
This is documented at the top of the file.

### New: `domain/Cargo.toml` feature flag

```toml
# Allocation-counting instrumentation for Plan 014 Phase 5.4 zero-alloc audits.
# Test-only; never enabled in production builds.
alloc_count = []
```

Off by default. Required by `tests/alloc_count.rs` via
`#![cfg(feature = "alloc_count")]`. When off, the test file is empty (no items)
and `cargo test --workspace` skips it without error.

### Edited: `domain/src/wire.rs` doc comment on `unpack_slice`

The previous comment overstated the allocation count:

> On the happy path the only allocations are the BLAKE3 hasher's internal
> scratch (one-time)

This was inaccurate — the audit shows there are **no** allocations on the
happy path. Corrected to:

> **Zero allocations on the happy path** (verified by Plan 014 Phase 5.4 audit
> in `domain/tests/alloc_count.rs`, 2026-06-27, blake3 1.8.5). BLAKE3's
> `Hasher` uses an `InlineSubCtxStack` (a fixed-size stack array, not a
> `Vec`), so the chunk-merge tree never grows the heap — confirmed zero-alloc
> up to a 160 KB / 10000-row payload. The returned slice is a
> `bytemuck::cast_slice` of the input buffer (a pointer reinterpret, no
> allocation).

The `unpack` doc comment was already correct ("No allocation on the happy
path — `from_bytes` is a cast.") and left unchanged.

### Edited: `.plans/014_katgpt_rs_paradigm_migration.md`

Phase 5.4 marked `[x]` DONE with the audit outcome recorded inline.

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Audit measurement | `cargo test -p event-checkin-domain --features wire,alloc_count --test alloc_count -- --nocapture --test-threads=1` | ✅ 10 tests, every shape reports `alloc_count = 0` |
| Workspace check | `cargo check --workspace --all-targets` | ✅ EXIT 0 (only pre-existing profile warning) |
| Workspace tests | `cargo test --workspace` | ✅ 285 tests pass (84 + 9 + 144 + 15 + 33), 0 failed |
| Workspace clippy | `cargo clippy --workspace --all-targets` | ✅ Zero warnings |
| Domain clippy (with feature) | `cargo clippy -p event-checkin-domain --features wire,alloc_count --all-targets` | ✅ Zero warnings |
| Frontend native | `cargo check --all-targets` in `frontend-leptos/` | ✅ EXIT 0 |
| Frontend wasm32 | `cargo check --target wasm32-unknown-unknown` in `frontend-leptos/` | ✅ EXIT 0 |

### Audit measurement table (the actual results)

```
unpack<LevelScore> (16 B payload): alloc_count = 0
unpack_slice<LevelScore> (0 rows): alloc_count = 0
unpack_slice<LevelScore> (1 row, 16 B payload): alloc_count = 0
unpack_slice<LevelScore> (3 rows, 48 B payload): alloc_count = 0
unpack_slice<LevelScore> (4 rows, 64 B payload — multi-chunk boundary): alloc_count = 0
unpack_slice<LevelScore> (50 rows, 800 B payload): alloc_count = 0
unpack_slice<LevelScore> (500 rows, 8 KB payload): alloc_count = 0
unpack_slice<LevelScore> (10000 rows, 160 KB payload — stress): alloc_count = 0
100× unpack<LevelScore>: alloc_count = 0
100× unpack_slice<LevelScore> (500 rows): alloc_count = 0
```

## 4. Plan / Code / Test Locations

- **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` Phase 5.4 (now marked
  DONE with audit outcome recorded inline).
- **Audit test**: `domain/tests/alloc_count.rs` (293 lines, 10 tests).
- **Feature flag**: `domain/Cargo.toml` `[features] alloc_count = []`.
- **Doc corrected**: `domain/src/wire.rs` `unpack_slice` doc comment.

## 5. Reflections

### What went well

- **The audit-first discipline paid off again.** I started with a measurement
  pass (assertions replaced with `eprintln!` of the observed count) before
  committing to any `assert_eq!(count, 0)`. The first run showed 0 for every
  shape, so I tightened the assertions to match. Had I asserted 0 from the
  start and been wrong, the test would have failed loudly and I'd have
  rewritten it; had I asserted nonzero from the start (my hypothesis), I'd
  have shipped a wrong bound. Measure-then-assert was the right call.
- **This is the first Plan 014 phase that's a pure positive result.** Phases
  1.4–1.6 demoted (variable strings), Phase 3.2 demoted (no polymorphism),
  Phase 4.3 demoted 3 of 4 candidates (already-done / already-done / unsafe).
  Phase 5.4 is the rare case where the plan said "do X" and X was actually
  worth doing and the result was a clean win.
- **The doc correction is real value.** The previous comment claimed
  allocations that don't exist. Future readers (or agents) who trusted the
  doc would either avoid the path needlessly or, worse, "fix" the
  non-allocations by adding complexity. The corrected comment now reflects
  measured reality, not guesswork.

### What was harder than expected

- **Rust 2024's `unsafe-op-in-unsafe-fn` rule.** The `unsafe impl GlobalAlloc`
  block forwards to `System.alloc` / `dealloc` / `realloc`. Under edition
  2024 these unsafe calls now require an explicit `unsafe { ... }` block even
  though the surrounding function is already `unsafe fn`. The compiler
  produced 4 E0133 warnings on the first compile. Fixed by wrapping each
  forwarding call in `unsafe { ... }` with a SAFETY comment.
- **Parallel-test contamination.** The counter is process-global, so two
  tests running concurrently would mix each other's counts. Solved by
  documenting `--test-threads=1` at the top of the test file and in the
  `cargo test` invocation in the doc comment. An alternative would have been
  `serial_test`, but that adds a dependency for a single-file concern.
  Documenting the threading constraint is the production-grade call: no extra
  deps, and the failure mode (counter goes nonzero) is loud and obvious.

### Where the result differs from the plan

The plan said "test asserting 0 allocs after warmup." This shipped exactly
that — but the **scope** is wider than the literal text. The plan implies a
single test (one assertion). I shipped 10 tests across every realistic shape
because the audit's value is the **regression guard** across the shape space,
not a single point. If blake3 ever regresses (or is swapped for a different
hasher), the per-shape tests pinpoint which shape broke.

## 6. Remaining Work

### Plan 014 status after this handover

- **Phase 1** (wire format): ✅ CONCLUDED
- **Phase 2.1** (SSOT audit): 🟡 Open
- **Phase 2.3** (CI dup-check): 🟡 Open
- **Phase 3** (policy traits): ✅ CONCLUDED (trait demoted, entry #7)
- **Phase 4.1** (profile): 🟡 Blocked on infra
- **Phase 4.3** (I/O wins): ✅ CONCLUDED
- **Phase 4.4** (no-SIMD doc): 🟡 Blocked on 4.1
- **Phase 5.3** (deterministic-not-stochastic lint): 🟡 Open
- **Phase 5.4** (zero-alloc audit): ✅ **CONCLUDED (this handover)**
- **Phase 5.5** (feature-flag discipline): 🟡 Open (mostly satisfied; 5.4
  itself is an example — `alloc_count` is opt-in off-by-default)

### What's next (priority order)

1. **Deploy** the commits now sitting on `develop` (5 from handover 109 + this
   handover's commits) via `develop → main → deploy.sh`. Operator action.
2. **Phase 5.3** — deterministic clippy lint for monetary code. Buildable now,
   no infra dependency.
3. **Phase 2.3** — forward-looking CI dup-check. Buildable now.
4. **Phase 2.1** — SSOT audit. High audit-miss risk per Plan 014 track record.

### Audit re-open preconditions for Phase 5.4

The 0-alloc assertion is a regression guard. It will fire if any of these
become true:

- blake3 is upgraded to a version that re-introduces heap growth in
  `Hasher::update` (e.g. replaces `InlineSubCtxStack` with a `Vec`).
- The wire format switches hash functions.
- New code lands on the decode path between `unpack_slice` entry and
  `cast_slice` exit that allocates (e.g. a `serde_json::from_slice` sneak).
- `bytemuck` regresses and `cast_slice` starts allocating (would also break
  the Phase 1 GOAT-gate premise — extremely unlikely).

If any precondition fires, the right response is to investigate the root
cause, not to relax the assertion. The assertion documents the contract; the
contract is "decode is zero-alloc in steady state".

## 7. Issues Ref

- Plan 014: `.plans/014_katgpt_rs_paradigm_migration.md`
- Plan 014 Phase 1 wire format: `.plans/014_wire_audit.md`
- Plan 014 negative results log: `.plans/014_negative_results.md` (9 entries;
  Phase 5.4 is a positive result, so no entry added)
- Previous handover (Phase 4.3 I/O wins): `.handovers/109_plan_014_phase4_3_io_wins.md`
- Phase 1 wire format handover: `.handovers/108_plan_014_phase1_wire_format.md`

## 8. How to Dev / Test

### Run the audit

```sh
cargo test -p event-checkin-domain \
    --features wire,alloc_count \
    --test alloc_count -- --nocapture --test-threads=1
```

`--features wire,alloc_count` is required (the test uses `wire::unpack` /
`wire::unpack_slice` and is gated behind `alloc_count`).
`--test-threads=1` is required (the counter is process-global).
`--nocapture` shows the per-shape `alloc_count = N` print lines.

### Run the full workspace (without the audit feature)

```sh
cargo test --workspace
# → 285 tests pass; alloc_count.rs is empty (feature off) and skipped
```

### Add a new shape to the audit

1. Open `domain/tests/alloc_count.rs`.
2. Copy any existing test function as a template.
3. Update the fixture and the assertion.
4. Add a comment explaining what shape the test exercises and why.
5. Run the audit command above to confirm.

### Verify no regression on the decode path

The audit IS the regression guard. If `cargo test --features wire,alloc_count
--test alloc_count` ever fails a `count, 0` assertion, treat it as a real
regression — investigate the root cause before either fixing the underlying
code or, if the regression is intentional (e.g. a documented blake3 upgrade
with a known trade-off), updating both the assertion and the doc comment in
`domain/src/wire.rs::unpack_slice`.