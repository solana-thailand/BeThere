# Handover 108 — Plan 014 Phase 1 Wire Format Infrastructure (Committed)

> **Branch**: `feature/014_wire_format_infra` → merged to `develop` (fast-forward)
> **Status**: ✅ **Committed + merged to `develop`**. Builds clean, 285 tests pass, clippy zero warnings, frontend compiles on x86_64 + wasm32-unknown-unknown. **NOT deployed, NOT exercised on a live worker** — the wire path is opt-in infrastructure with no production endpoints wired yet.
> **Commit**: `787e62d` — `feat(domain): opt-in zero-copy wire format with BLAKE3 commitment (Plan 014 Phase 1)`
> **Predecessor**: handover #107 (Solana Mobile Demo — MWA + PWA; Demo Day 2026-06-23 has passed)
> **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 1 only; Phases 2–5 still open)
> **Created**: 2026-06-27

---

## 1. What Happened

Committed the previously-uncommitted Plan 014 Phase 1 work that was sitting in the `develop` working tree. This is a **documentation + infrastructure** commit — it lands the zero-copy wire format envelope as opt-in Cargo feature infrastructure, plus the audit trail that explains why the production rollout was demoted.

The single substantive code change beyond what the plan had already produced: **removed the only `expect()` from the decode hot path** (`domain/src/wire.rs::unpack_slice`). The `try_into().expect("count slice is exactly 4 bytes")` was provably safe (the bounds check above guarantees the slice length), but it was the lone panic site in the receiver. Converted to a panic-free fixed-index `u32::from_le_bytes([buf[N], buf[N+1], buf[N+2], buf[N+3]])` read so the decode path has **zero panic sites** — a hard requirement for production WASM receivers where a panic aborts the request.

A second cleanup fell out of the Pod-derive change: `LevelScore` gained `Copy`, so `worker/src/adventure.rs:122` switched from `prev.clone()` to `*prev` to clear the resulting clippy `clone_on_copy` warning.

**Key honesty note for future readers:** Phase 1 was a **mixed outcome**, not a win. The format infrastructure works (GOAT-gate cleared at every row count), but every production endpoint nominated for the rollout was audited and **demoted** — they all carry variable-length `String` fields that defeat pure Pod. The format ships as opt-in seam infrastructure; JSON remains the canonical wire format. There is currently **no production caller** of the wire path.

---

## 2. Changes (1 commit, 21 files, +2,697 lines)

### `787e62d feat(domain): opt-in zero-copy wire format with BLAKE3 commitment (Plan 014 Phase 1)`

#### New: wire envelope (`domain/src/wire.rs`, 364 lines)

The single source of truth for the binary envelope — compiles to both x86_64 (worker host, benches) and wasm32 (frontend).

- **`pack<T: Pod>` / `unpack<T: Pod + Copy>`** — single-value envelope.
  Layout: `[magic(4) | version(1) | reserved(3)] || payload || blake3(32)` = 8 + size_of::<T>() + 32 bytes.
- **`pack_slice<T: Pod>` / `unpack_slice<T: Pod + Copy>`** — batch envelope (the natural shape for list endpoints).
  Layout: `[magic(4) | version(1) | reserved(3)] || count_le(4) || payload || blake3(32)`.
  The little-endian row count sits **inside the hashed body** so a sender cannot change it without invalidating BLAKE3.
- **Constants**: `WIRE_MAGIC = *b"BTE1"`, `WIRE_VERSION = 1`, `WIRE_HEADER_LEN = 8`, `WIRE_COUNT_LEN = 4`, `WIRE_TAG_LEN = 32`, `CONTENT_TYPE = "application/x-bethere-bin"`.
- **`WireError` enum** — `Truncated { got, want }`, `BadMagic`, `UnsupportedVersion { got, want }`, `HashMismatch`. Implements `Display + std::error::Error`.
- **Decode invariants**: panic-free. Truncation, bad magic, version skew, and BLAKE3 mismatch all return typed errors; `count.checked_mul(size_of::<T>())` defends against adversarial overflow (returned as `Truncated { want: usize::MAX }`).
- **8 unit tests** — round-trip (single + slice, empty + 500 rows), size-invariants, truncation rejection, corruption rejection (hash mismatch).

Feature-gated under `#![cfg(feature = "wire")]` — the entire module compiles away when the feature is off.

#### Modified: Pod derives on flat domain types

- **`LevelScore`** (`domain/src/models/adventure.rs`) — 16 bytes: 3 × `u32` + `u8` + 3 explicit `_pad`. Added `#[repr(C)]`, `Copy`, `cfg_attr(feature = "wire", derive(Pod, Zeroable))`. The `_pad: [u8; 3]` is `#[serde(skip)]` so **JSON output is byte-identical** to the pre-wire form. Made `Copy` because the wire path dereferences rather than clones.
- **`FunnelSnapshot`** (`domain/src/models/event_summary.rs`) — 72 bytes: 9 × `u64`. Same `#[repr(C)]` + `Copy` + Pod derive pattern.
- **`FinancialSnapshot`** — 32 bytes: 4 × `u64`. Same pattern.

#### New: smoke endpoint (`worker/src/handlers/wire.rs`, 66 lines)

- `GET /api/wire-sample/level-score[?fmt=bin]` — public, no auth. Returns a fixed `LevelScore` sample as JSON (canonical default) or the BLAKE3-committed wire envelope with `Content-Type: application/x-bethere-bin` when `?fmt=bin` is requested.
- Wired into the router in `worker/src/handlers/mod.rs:89` with a comment noting it is safe to remove after the GOAT-gate clears (which it has — the endpoint stays as a live probe for now).

#### New: frontend client (`frontend-leptos/src/api/wire.rs`, 49 lines)

- `get_wire_sample_level_score()` — fetches the smoke endpoint with `?fmt=bin`, reads the body via `response_array_buffer`, decodes via the shared `domain::wire::unpack`. Returns an owned `LevelScore` (the 16-byte copy out of the response buffer is unavoidable at the WASM↔JS boundary; the win is skipping `serde_json` on the result).
- `response_array_buffer` helper added to `frontend-leptos/src/api/fetch.rs` — the WASM bridge that turns a `Response` into `Vec<u8>` via `js_sys::ArrayBuffer`.

#### New: criterion microbench (`domain/benches/wire_bench.rs`, 122 lines)

The GOAT-gate arbiter. Measures JSON vs wire encode **and** decode at row counts 50/200/500/1000. x86_64-only (criterion is not on wasm32); documented as a conservative lower bound for the frontend decode side (wasm JSON parsers are typically 1.3–2× slower than native, so a host "3× win" is a wasm "≥3× win").

Run: `cargo bench -p event-checkin-domain --features wire --bench wire_bench`.

#### New: tests

- `domain/tests/pod.rs` (167 lines) — 9 tests: type-specific round-trips for all 3 Pod types, tamper detection (truncation, wrong magic, version skew, payload corruption), compile-time layout assertions (16/72/32 bytes), `Zeroable` contract (zeroed bytes are valid).
- `frontend-leptos/tests/wire.rs` (84 lines) — 3 tests: frontend can reach domain wire types, decodes worker-shaped `FunnelSnapshot`, rejects tampered payloads.

#### Modified: Cargo manifests

- `domain/Cargo.toml` — added optional `bytemuck` (with `derive` feature) + `blake3`, gated under new `wire` feature (default-off). Added `criterion` as dev-dependency + `[[bench]]` entry with `required-features = ["wire"]`.
- `worker/Cargo.toml` — `event-checkin-domain` features changed from `["qr"]` to `["qr", "wire"]`.
- `frontend-leptos/Cargo.toml` — added path dep on `event-checkin-domain` with `features = ["wire"]`, plus direct `bytemuck` + `blake3` (decode runs in the frontend crate).
- `Cargo.lock` — +359 lines (new transitive deps: `bytemuck`, `blake3`, `criterion`, and their trees).

#### New: audit docs (`.plans/`)

- `014_katgpt_rs_paradigm_migration.md` (364 lines) — the master plan. Phase 1 concluded; Phases 2–5 still open.
- `014_wire_audit.md` (384 lines) — Phase 1 audit. Candidate inventory, the 3-of-4 audit miss log, GOAT-gate result, the byte-size table proving EventMeta forced-Pod is 26% larger than JSON.
- `014_policy_audit.md` (318 lines) — Phase 3.1 policy audit. The `EventPolicy` trait verdict (no behavioral polymorphism — variation is data, not behavior) and the ⚠️ CORRECTION section that caught the deposit/refund SSOT "duplication" was actually a deliberate two-stage guard.
- `014_negative_results.md` — 8-entry demotion log. Each entry has: original framing, why demoted, proof pointer, and the preconditions that would re-open it.

---

## 3. Validation Done

| Check | Method | Result |
|---|---|---|
| Workspace compiles | `cargo check --workspace --all-targets` | ✅ EXIT 0 (only pre-existing profile warning) |
| Workspace tests | `cargo test --workspace` | ✅ **285 tests, 0 failed** (84 domain unit + 9 pod + 144 worker + 15 + 33 misc) |
| Workspace clippy | `cargo clippy --workspace --all-targets` | ✅ **Zero warnings** after fixing the `clone_on_copy` on `LevelScore` |
| Wire feature tests | `cargo test -p event-checkin-domain --features wire` | ✅ 84 + 9 (pod) pass |
| Frontend compiles (native) | `cargo check --all-targets` in `frontend-leptos/` | ✅ EXIT 0 |
| Frontend compiles (wasm32) | `cargo check --target wasm32-unknown-unknown` | ✅ EXIT 0 |
| Frontend wire tests | `cargo test --test wire` in `frontend-leptos/` | ✅ 3/3 pass |
| Frontend clippy (wire files) | `cargo clippy --all-targets` in `frontend-leptos/` | ✅ Zero warnings on `wire.rs`/`fetch.rs` (pre-existing warnings elsewhere, unrelated) |
| Panic-free decode audit | Manual review of `domain/src/wire.rs` | ✅ Removed the lone `expect()`; zero panic sites in `unpack`/`unpack_slice` |
| Production-grade audit | Manual review of all new files | ✅ No `unwrap()`/`expect()`/`todo!`/`unimplemented!`/`match _` in production code; `.expect()` only in test/bench harness (acceptable) |

---

## 4. GOAT-gate Outcome (Phase 1.7)

The criterion microbench cleared both thresholds at **every** row count:

| Row count | Decode speedup (wire vs JSON) | Payload size reduction |
|---|---|---|
| 50 | 4.7× | 71.8% |
| 200 | 5.5× | 72.8% |
| **500** (headline) | **6.2×** | **73.5%** |
| 1000 | 6.9× | 74.1% |

Thresholds were **≥3× decode AND ≥40% size reduction**. Cleared on both axes at every measured scale.

**Caveat documented in the bench file:** these are x86_64 host numbers. The wasm32 frontend decode is likely **better**, not worse — wasm JSON parsers are typically 1.3–2× slower than native, and `bytemuck::cast_slice` on wasm32 is a literal pointer reinterpret (one bounds check, regardless of payload size). So the host gate is a conservative lower bound for the production frontend path.

---

## 5. The 8 Demoted Production Rollout Candidates

The format works. **None of the nominated production types do.** Full reasons in `.plans/014_negative_results.md`; summary:

| # | Candidate | Demotion reason |
|---|---|---|
| 1 | Transformer VM (Obj 2 original) | No transformer in workload — over-engineering |
| 2 | Neuro-symbolic policy graphs (Phase 3 original) | Policies are O(1) deterministic; learned component breaks auditability of monetary code |
| 3 | SIMD kernels (Phase 4 original) | No dense `f32` math in hot paths — all I/O-bound |
| 4 | `EventMetaWire` (Phase 1.4) | **Audit miss** — 8 variable-length `String` fields. Forced fixed arrays are 26% LARGER than JSON (1232 B vs 981 B) |
| 5 | `AttendeeListItemWire` (Phase 1 original) | 32 fields, 25 `Option<String>` — refactor cost enormous, per-row win muted |
| 6 | Quiz/adventure batches (Phase 1 original) | Wrong scope — no batch endpoint exists (`scores` is a `HashMap` inline) |
| 7 | `EventPolicy` trait (Phase 3.2 reframing) | No behavioral polymorphism — per-event variation is data fields, not different formulas |
| 8 | Deposit/refund SSOT consolidation (Phase 2.3) | **Audit miss #4** — the 14 `if !event.deposit_enabled` sites are a deliberate two-stage guard (the second stage varies per endpoint); the seconds/ms split is a Solana-vs-HTTP domain boundary |

**Meta-finding (recorded in negative-results entry #8):** Plan 014's audits reliably **over-estimate** how much of the codebase is wrong/consolidable. The GOAT-gate discipline caught all 4 misses **pre-merge** because each consolidation plan was itself gated against actual call-site semantics. This is the katgpt-rs "GOAT FAILED → demote" discipline working as designed.

**What stays in the codebase:**
- `domain/src/wire.rs` — infrastructure, not dead code. Future types get added per-PR when their specific cost/benefit clears the bar.
- Pod derives on `FunnelSnapshot` / `FinancialSnapshot` / `LevelScore` — these types ARE genuinely Pod-compatible; the derives are free.
- The smoke endpoint stays as a live probe.

---

## 6. Reflections

### What went well

- **The verification-before-commit order caught two issues.** Running clippy before staging surfaced the `clone_on_copy` warning (a direct consequence of adding `Copy` to `LevelScore`) — fixed before commit, so the committed state is clippy-clean.
- **The panic-free audit caught the lone `expect()`.** The `try_into().expect("count slice is exactly 4 bytes")` in `unpack_slice` was provably safe, but it was the only panic site in the receiver. Converting to fixed-index reads eliminates the entire panic surface from the decode path — a real production-grade improvement, not cosmetic.
- **The commit message was redone cleanly via `-F file`.** The first attempt used an inline heredoc-style message with backticks, and the shell interpreted `` `wire` `` / `` `?fmt=bin` `` as command substitutions (the `sh: wire: command not found` errors). Amending with `-F /tmp/commit_msg_014.txt` avoided all shell interpolation. Lesson: for any commit message containing backticks, dollar signs, or quotes, **always use `-F`**.
- **No scope creep.** The task was "commit existing work." The only code changes were the two cleanups required by the existing change (panic-free decode, clippy fix). No new features added.

### What was struggled with

- **Shell escaping in the commit message.** Backticks in the original message (`"wire"`, `"?fmt=bin"`) were executed as command substitutions. The commit succeeded but the message body had gaps. Caught on the post-commit verification of `git log -1`; fixed with `git commit --amend -F`.
- **The workspace vs frontend-leptos split.** `cargo check --workspace` does NOT cover `frontend-leptos` (it is excluded from the workspace `Cargo.toml` and built separately under `trunk`). Had to run `cargo check` inside `frontend-leptos/` separately — and additionally on `wasm32-unknown-unknown` since that is the production target.

### What was solved

- **The `LevelScore` `_pad` field is a non-breaking change.** Adding `_pad: [u8; 3]` with `#[serde(skip)]` keeps JSON byte-identical. Verified there are no other construction sites in the frontend that would fail to compile — the frontend has its own `LevelScore` type in `frontend-leptos/src/pages/adventure/types.rs` (no `_pad`), and the only `domain::LevelScore` construction sites are in the wire tests (which already specify `_pad: [0; 3]`).

---

## 7. Remaining Work

### Plan 014 — still open

- [ ] **Phase 2.1** — Cross-crate SSOT audit (participation-type normalization, deposit-status enum mapping). The deposit/refund subset (2.3) was investigated and demoted (negative-results entry #8); the remaining candidates need their own audit before any moves.
- [ ] **Phase 2.3** — Build the forward-looking CI check that flags *future* business-predicate duplication across `worker/` and `frontend-leptos/`. Value is forward-looking — the retroactive remediation list is near-empty.
- [ ] **Phase 4.1** — Profile a staged 200-attendee event end-to-end. Confirms (or refutes) the I/O-bound hypothesis with data. **Blocked on infrastructure coordination.**
- [ ] **Phase 4.3.1–4.3.4** — The genuine "green software" wins (KV cache event-series, parallelize `get_public_ticket` reads, batch KV writes, extend blockhash TTL). These are the only *positive* opportunities left in Plan 014; everything else was demoted.
- [ ] **Phase 5.3 / 5.4** — Deterministic-not-stochastic lint; zero-allocation hot-path audit.

### Operational

- [ ] **Deploy** the commit to dev/prod (standard `develop → main → deploy.sh` path). No D1/R2/KV schema changes; the wire path is additive and default-off, so rollback is `wrangler rollback` if anything breaks.
- [ ] **Demo Day retrospective gap** — Demo Day (2026-06-23) passed with no recorded outcome. Optional: a short note in `.handovers/` documenting what worked / what didn't on-device, if you want the history to be complete.

---

## 8. How to Dev/Test

### Build

```bash
git fetch origin
git checkout develop              # commit 787e62d is merged here
cargo check --workspace --all-targets
cd frontend-leptos && cargo check --target wasm32-unknown-unknown
```

### Run the wire tests

```bash
# Domain + worker (workspace)
cargo test --workspace --features wire

# Domain wire unit tests + Pod integration tests
cargo test -p event-checkin-domain --features wire

# Frontend reachability + decode tests
cd frontend-leptos && cargo test --test wire
```

### Re-run the GOAT-gate bench

```bash
cargo bench -p event-checkin-domain --features wire --bench wire_bench
# Output under domain/target/criterion/. Check decode/json vs decode/wire medians.
```

### Exercise the smoke endpoint locally

```bash
# Terminal 1
cd worker && wrangler dev

# Terminal 2 — JSON (default, canonical)
curl http://localhost:8787/api/wire-sample/level-score
# → {"moves":7,"puzzles_solved":2,"time_seconds":45,"stars":2}

# Binary envelope (opt-in)
curl -H "Accept: application/x-bethere-bin" \
     "http://localhost:8787/api/wire-sample/level-score?fmt=bin" \
     | xxd | head
# → 8-byte header (BTE1 + version) + 16-byte payload + 32-byte blake3 tag = 56 bytes
```

---

## 9. Issues Ref

- Plan: `.plans/014_katgpt_rs_paradigm_migration.md`
- Audit docs: `.plans/014_wire_audit.md`, `.plans/014_policy_audit.md`, `.plans/014_negative_results.md`
- Predecessor: handover #107 (Solana Mobile Demo — MWA + PWA)
- Branch: `feature/014_wire_format_infra` (merged to `develop` fast-forward at `787e62d`)
- Smoke endpoint: `GET /api/wire-sample/level-score[?fmt=bin]` (worker/src/handlers/wire.rs)
- Format module: `domain/src/wire.rs` (feature-gated under `wire`, default-off)
- No issue tracker entry — this is plan-driven work, not an issue remediation

---

## 10. Commit Plan

Single commit on `feature/014_wire_format_infra`, fast-forward merged to `develop`:

1. `787e62d feat(domain): opt-in zero-copy wire format with BLAKE3 commitment (Plan 014 Phase 1)`

**Status:**
- ✅ Committed at `787e62d`
- ✅ Merged to `develop` (fast-forward)
- ✅ Validated: `cargo check`/`test`/`clippy` clean on workspace; frontend compiles on x86_64 + wasm32; 285 + 3 tests pass; clippy zero warnings
- ⏳ Operator: deploy when ready (no schema changes; rollback is `wrangler rollback`)