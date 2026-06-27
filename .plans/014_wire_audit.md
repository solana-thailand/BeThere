# Plan 014 — Phase 1: Zero-Copy Wire Audit & GOAT-gate

> **Status:** **Phase 1 CONCLUDED** — see [Phase 1 conclusion](#phase-1--concluded)
> below for the final outcome. Smoke-test (Task 1.7) cleared the GOAT-gate;
> production rollout (Tasks 1.4–1.6, 1.8) demoted — every nominated production
> type turned out to have variable-length `String` fields that defeat Pod
> compatibility. See `.plans/014_negative_results.md` for the demotion record.
>
> **Method:** Read-only investigation across `worker/src/handlers/**`,
> `frontend-leptos/src/api/**`, and `domain/src/**`. No production code
> modified during the audit; smoke-test code (Tasks 1.2–1.3, 1.7) is in place
> as infrastructure.
> **GOAT-gate reminder:** Zero-copy only pays for `batch` shape, row count ≥ 50,
> and frequency ≥ once/event. Everything else stays JSON.

---

## Phase 1 — CONCLUDED

**Outcome:** GOAT-gate **PASSED** at the format level; production rollout
**DEMOTED**. The two parts of Phase 1 split:

| Sub-goal | Outcome | Evidence |
|---|---|---|
| Prove the wire format works (Tasks 1.2–1.3) | ✅ **PASS** | 93 tests, builds on x86_64 + wasm32, smoke endpoint live |
| Clear the GOAT-gate (Task 1.7) | ✅ **PASS** | 6.2× decode, 73.5% size reduction at 500 rows (LevelScore fixture) |
| Apply to a production type (Tasks 1.4–1.6) | ❌ **DEMOTE** | Every nominated type has variable-length strings — see audit miss log below |
| Promote `wire` to default-on (Task 1.8) | ⛔ **NO** | No production type justifies the maintenance surface; stays opt-in |

**Why demote:** Plan 014's acceptance criterion for Phase 1 was "benchmark
passes its GOAT gate **OR** Phase 1 is documented as a negative result and
demoted." We achieved the first clause. The demotion is *not* of Phase 1
itself — it's of the production rollout (Tasks 1.4–1.6), which the audit
over-promised. The format infrastructure ships as opt-in; future types get
added per-PR when their specific cost/benefit clears the bar.

### Audit miss log (3 of 4 production candidates failed)

The audit nominated four production types. Three don't clear the bar:

| # | Type | Audit said | Reality | Demotion reason |
|---|---|---|---|---|
| 1 | Quiz/adventure batches | top 3 batch target | No batch endpoint exists (`scores` is a `HashMap` inline) | Wrong scope — never batched |
| 2 | `AttendeeListItemWire` | top 3 batch target | 32 fields, 25 `Option<String>` | Audit's own call — too many strings |
| 3 | `EventMetaWire` | "~200 B wire vs ~600 B JSON, ~1 day" | **26% LARGER than JSON** when forced to fixed arrays | Audit missed 8 variable `String` fields (`name`, `tagline`, `location`, `video_url`, `nft_image_url`, `poster_url`, `created_at`, `organization_id`) |
| 4 | `DepositStatusWire` | "~120 B wire vs ~400 B JSON, ~1 day" | Viable but needs base58 conversion + ID length policy | Audit's 1-day estimate is wrong; ~1–2 days; **deferred** |

**Pattern:** the audit systematically underestimated how many `String` fields
production types carry. Pure Pod is right for fixed-size numerics and
ID-like strings, wrong for the human-readable text and URLs that dominate most
response payloads. This is the katgpt-rs "GOAT FAILED → demote" discipline in
action — the format itself is fine, the *applications* of it aren't.

**Concrete proof for EventMeta** (the most-cited candidate):

| Layout | Bytes/row | vs JSON |
|---|---|---|
| JSON (realistic fixture) | 981 | baseline |
| Fixed-array Pod | 1,232 | **+26% (WORSE)** |

The audit counted only ~80 bytes of small fixed fields and asserted
"~200 bytes wire"; it ignored 8 variable `String` fields totaling ~700 bytes
at realistic lengths. Worst-case fixed arrays (e.g. `[u8; 192]` for URLs)
inflate every row regardless of actual content.

### What stays in the codebase

- `domain/src/wire.rs` — the envelope (`pack`/`unpack`/`pack_slice`/`unpack_slice`),
  BLAKE3 commitment, content-type. **Infrastructure**, not dead code.
- `domain/Cargo.toml` — `wire` feature, opt-in. Default-off per Task 1.8 decision.
- `domain/benches/wire_bench.rs` — the GOAT-gate evidence. Re-runnable any time
  someone proposes adding a production type.
- `domain/tests/pod.rs`, `frontend-leptos/tests/wire.rs` — regression coverage.
- `worker/src/handlers/wire.rs` — smoke endpoint. Stays as a live probe; can be
  removed in a future cleanup if the format never lands on a production type.
- Pod derives on `FunnelSnapshot`, `FinancialSnapshot`, `LevelScore` — these
  types ARE genuinely Pod-compatible; the derives are free and stay.

### When to revisit

Add a new wire type when **all four** are true:
1. The type is genuinely fixed-size (no variable `String`, `Vec`, `HashMap`).
2. The endpoint is `batch` shape (≥50 rows) **or** KV-cached per-request.
3. Frequency ≥ once/event.
4. A quick re-run of `wire_bench.rs` on the candidate type clears both gates.

If the type has variable strings, the right answer is a hybrid format
(Pod header + string heap) — but that's reinventing FlatBuffers and is not
worth it for this I/O-bound codebase. JSON wins.

---

## Headline finding (honest)

**The plan's Phase 1.3 hypothesis is wrong.** The original plan proposed
zero-copy on "the top 3 batch payloads: attendee list row, quiz answer batch,
adventure checkpoint batch." The audit refutes two of those three:

| Hypothesized target | Reality | Verdict |
|---|---|---|
| Attendee list row | ✅ Real — `GET /api/attendees`, up to 200 rows, hot on staff dashboard | **KEEP** |
| Quiz answer batch | ❌ Batches of ~5–10 questions, user-once-per-claim path, never hot | **DROP** |
| Adventure checkpoint batch | ❌ No batch endpoint exists. `AdventureProgress::scores` is a `HashMap<String, LevelScore>` shipped inline in a small-struct response | **DROP** |

**Revised, evidence-based Phase 1 target set:** `attendee-list-row`,
`event-meta-list`, and `deposit-status-cached-row`. All three have real batch /
hot-cache leverage. The adventure `LevelScore` becomes a *proof-of-concept*,
not a production target.

---

## TIER 1 — Real production candidates (evidence-based)

Strict filter: `batch` shape AND (row count ≥ 50 OR KV-cached per-request) AND
frequency ≥ once/event.

| # | Endpoint | File:line | Element type | Rows | Why it matters |
|---|---|---|---|---|---|
| 1 | `GET /api/attendees` | `worker/src/handlers/attendee.rs:153` | `AttendeeListItem` | up to **200** | Staff dashboard workhorse. Largest per-response payload in staff UX. |
| 2 | `GET /api/events` | `worker/src/handlers/events/list.rs:108` | `EventMeta` | ~10–50 | Page-load hot path on events index. KV→D1 fallback. |
| 3 | `DepositStatus` KV cache | `domain/src/models/deposit.rs:36` | `DepositStatus` (cached) | 1 row, but **read on every check-in + refund** | KV-cached per-attendee; currently `serde_json`-decoded on every read. Skip decode entirely on cache hit. |

### TIER 1B — Large but admin-only (defer)

| Endpoint | Element type | Rows | Why deferred |
|---|---|---|---|
| `GET /api/contacts/audience` | `AudienceRow` | **~1000+** | Largest absolute payload, but export-only (admin opens it once). Bandwidth win yes; latency win no. |
| `GET /api/events/{id}/audit` | `AuditEntry` | up to **100** | Admin-only, infrequent. |
| `GET /api/wallet/{addr}/nfts` | `NftItem` | up to **100** | Helius-backed, secondary page. |
| `GET /api/wallet/leaderboard` | `LeaderboardEntry` | 50 hard cap | Already KV-cached 5 min. |

### TIER 2 — Hot but tiny arrays (not worth the format overhead)

| Endpoint | Element type | Rows | Verdict |
|---|---|---|---|
| `GET /api/dashboard/live` | `FunnelStage` + `ActivityEntry` | ~24 total | Polled every 2.5s ⚡, but per-response decode is tiny. The win is **edge cache + delta protocol**, not zero-copy. Document as negative result for zero-copy. |

---

## Critical blockers discovered

These were not visible in the original plan. Both must be resolved before Phase
1.2 produces working code.

### Blocker A — Frontend does NOT depend on `domain`  *(RESOLVED in Task 2.0)*

`frontend-leptos/Cargo.toml` (verified) had **no `event-checkin-domain`** entry.
All response types were **hand-mirrored** across `api/types.rs` and ~10
per-domain modules (comments like `/// Mirrors domain::models::attendee::CheckInStatus`).

**Resolution (Task 2.0, completed):** Added `event-checkin-domain` as a path
dependency with the `wire` feature. The frontend now compiles for
`wasm32-unknown-unknown` with `domain` reachable — see
`frontend-leptos/tests/wire.rs` (3 passing tests) for the smoke proof.
Blocker A is cleared; Phase 1.3+ can decode wire payloads on the client.

**Important scope correction:** The original Task 2.0 proposal was to "replace
hand-mirrored types one module at a time." Investigation during implementation
revealed this is a much larger and riskier effort than the audit implied:

1. **Defensive `#[serde(default)]` on every field** — e.g. `AttendeeResponse`
   has 22 fields, all defaulted. The frontend tolerates partial responses.
   Domain types don't have this. A blind swap would lose the tolerance.
2. **UI helper methods** — frontend `CheckInStatus` has `as_str()`, `label()`,
   `is_approved()`; `DepositMethod` has `icon_name()`. These couple to UI
   concerns that don't belong in `domain`. Moving them in couples domain to
   UI; keeping them out means newtype wrappers or extension traits.
3. **Local game-engine types** — `pages/adventure/types.rs::LevelScore` is a
   local game state type, not a network mirror. The API uses a separate
   `AdventureLevelScore`. These cannot be unified without restructuring.

**Decision:** The full JSON-mirror → `domain` migration is deferred to a
separate plan. Task 2.0 only adds the dep + smoke test, which is the actual
prerequisite for Phase 1.3+. The existing JSON mirrors stay — they have
legitimate defensive purposes and the migration would touch every call site.

The wire-format decode path (Phase 1.3+) does NOT need the JSON mirrors
replaced: it will import `domain::models::*Wire` types directly for the new
`?fmt=bin` endpoints, while existing JSON endpoints keep using the mirrors.
Both coexist cleanly.

### Blocker B — `cached_get` caches raw JSON `String`

`frontend-leptos/src/api/mod.rs:117` `cached_get` returns `Result<String,
ApiError>` — it caches the **raw JSON text** keyed by path (30s SWR).

**Consequence:** A binary response cannot flow through this cache as-is. Two
options:
1. **Parallel cache key** — `cached_get_bin(path)` caches `Vec<u8>` keyed by
   `"{path}#bin"`. Simple but doubles cache surface.
2. **Tagged cache entry** — wrap as `enum CacheEntry { Json(String), Bin(Vec<u8>) }`.
   Single cache surface, but every existing caller must be touched.

**Recommended action:** Option 1 (parallel cache) for the rollout. Lower
blame-radius; existing JSON callers are untouched.

---

## Proof-of-concept targets (Pod-compatible with trivial changes)

Three types in `domain` are **Pod-compatible with trivial changes** —
only attribute additions, plus one explicit-padding field. These validate the
entire pipeline (bytemuck on `wasm32`, round-trip encode/decode, BLAKE3
commitment, content-type negotiation) **before any production type is touched**.

| Type | File:line | Size | Pod work |
|---|---|---|---|
| `FunnelSnapshot` | `domain/src/models/event_summary.rs:36` | 72 bytes (9 × u64) | ✅ attributes only — already no padding |
| `FinancialSnapshot` | `domain/src/models/event_summary.rs:72` | 32 bytes (4 × u64) | ✅ attributes only — already no padding |
| `LevelScore` | `domain/src/models/adventure.rs:55` | 16 bytes (3 × u32 + u8 + **3 bytes explicit pad**) | ⚠️ requires `_pad: [u8; 3]` field — see correction below |

> **Correction (verified during Phase 1.2a implementation):** the original audit
> draft claimed `LevelScore` was Pod-compatible "as-is". That was wrong.
> `bytemuck::Pod` rejects implicit compiler padding (uninitialized bytes) at
> compile time — `LevelScore` under `#[repr(C)]` has 3 trailing pad bytes after
> `stars: u8`, so the derive failed with `E0080: evaluation panicked: derive(Pod)
> was applied to a type with padding`. Fixed by adding a public `#[serde(skip)]
> _pad: [u8; 3]` field. JSON output is byte-identical. This is exactly the kind
> of finding the proof-of-concept was designed to surface before touching
> production types.

**Use `LevelScore` as the smoke-test.** It's small, self-contained, and (unlike
the snapshots) is not yet on any hot endpoint — perfect for proving the format
without risking production responses.

---

## Refactor-effort map for production candidates

For each TIER 1 element, the field-level work to reach Pod compatibility.
Pattern: introduce a parallel `*Wire` Pod type + `From<&T>` / `Into<T>`. The
JSON type stays unchanged.

### `EventMetaWire` (highest batch leverage — `GET /api/events` list)

`domain/src/models/event.rs:186` — 11 `String` + 2 `i64` + 3 `bool` +
`Vec<String>` (organizer emails) + `u32` + 4 fieldless enums + 2 `Option<u32>`.

| Field | Current | Wire form | Notes |
|---|---|---|---|
| `slug`, `event_id` | `String` | `[u8; 32]` | slug ≤ 32 chars by validation |
| `sheet_id` | `String` | `[u8; 44]` | Google Sheet IDs are 44 chars |
| `escrow_address` | `String` | `[u8; 32]` | Solana pubkey |
| `organizer_emails` | `Vec<String>` | `[Email; 4]` + `u8 len` | Cap at 4 organizers (validate against max) |
| 4 enums | fieldless | `#[repr(C)]` → 1 byte | See audit Group 2 |
| `start_ms`, `end_ms` | `i64` | `i64` | direct |
| `capacity`, `registered` | `u32` | `u32` | direct |

Estimated wire size: **~200 bytes** vs. ~600-byte JSON. Effort: ~1 day.

### `DepositStatusWire` (KV hot path — every check-in/refund)

`domain/src/models/deposit.rs:36` — 4 `String` + `DepositMethod` (1 enum) +
`u64` + `Option<String>` + `bool` + `String` + `Option<String>` + `u32` + 2 `bool`.

| Field | Current | Wire form |
|---|---|---|
| `tx_signature` | `String` | `[u8; 64]` (Solana sig) |
| `wallet_address` | `String` | `[u8; 32]` (Solana pubkey) |
| `attendee_id`, `event_id` | `String` | `[u8; 32]` (UUIDv7 → 16, pad to 32) |
| `currency` | `String` | `[u8; 4]` ("USDC" / "THB\0") |
| `confirm_tx`, `error` | `Option<String>` | inline `[u8; 64]` + present-flag |

Estimated wire size: **~120 bytes** vs. ~400-byte JSON. Effort: ~1 day. Plus KV
schema migration (store `Vec<u8>` next to JSON during rollout).

### `AttendeeListItemWire` — DEFERRED

`AttendeeListItem` mirrors `Attendee` (32 fields, 25 `Option<String>` sheet
columns). Refactor cost is **enormous** and the per-row win is muted by the
fact that most rows are small in practice. The honest call: do **not** Pod-ify
this in v1. Even though `GET /api/attendees` is the largest batch endpoint, the
cost/benefit doesn't clear the GOAT-gate relative to `EventMeta` and
`DepositStatus`. Document as a v2 candidate.

---

## Revised Phase 1 sequencing

The original 1.1 → 1.8 sequence is reordered based on the two blockers above.

| Step | Action | Prerequisite | GOAT-gate |
|---|---|---|---|
| **1.1** ✅ | This audit | — | — |
| **1.2a** | Add `bytemuck` to `domain/Cargo.toml` behind `wire` feature; derive `Pod`/`Zeroable` on `FunnelSnapshot`, `FinancialSnapshot`, `LevelScore`. | — | compiles on `wasm32` + `x86_64` |
| **1.2b** | Round-trip test in `domain/tests/pod.rs` — encode, decode, BLAKE3 commit, validate magic + version | 1.2a | test passes |
| **2.0 (was Phase 2)** ✅ | **Added `event-checkin-domain` path dep + `wire` feature to `frontend-leptos/Cargo.toml`.** Smoke test in `frontend-leptos/tests/wire.rs` proves reachability on `wasm32`. **JSON mirror types deliberately kept** (defensive serde + UI helpers) — full SSOT migration deferred to a separate plan. | 1.2b | frontend builds on `wasm32`; 3 wire smoke tests pass |
| **1.3** ✅ | Defined shared `domain::wire` module (envelope + BLAKE3 + content-type); added worker smoke endpoint `GET /api/wire-sample/level-score?fmt=bin`; added frontend `response_array_buffer` decode helper + `get_wire_sample_level_score` API function | 2.0 | worker encodes bin on `wasm32`; frontend decodes on `wasm32`; both compile end-to-end |
| **1.4** ❌ | Define `EventMetaWire` + parallel `?fmt=bin` route on `GET /api/events` | 1.3 | **DEMOTED** — EventMeta has 8 variable `String` fields; fixed-array Pod is 26% larger than JSON. See Phase 1 conclusion. |
| **1.5** ❌ | Define `DepositStatusWire` + KV-cached bin variant on `GET /api/deposit/status/{id}` | 1.4 | **DEFERRED** — viable but needs base58 conversion + ID length policy (~1–2 days, not 1). Revisit when a per-check-in CPU hotspot is profiled. |
| **1.6** ❌ | `cached_get_bin` parallel cache in frontend `api/mod.rs` | 1.5 | **DEMOTED** with 1.5 — no production type needs the parallel cache. |
| **1.7** ✅ | `criterion` microbench: 50/200/500/1000-row `LevelScore` slice, JSON vs bin | 1.6 | **≥3× decode speedup AND ≥40% payload reduction** — both cleared at every row count (see results below) |
| **1.8** ⛔ | Rollout gate: promote `wire` to default-on only if 1.7 passes | 1.7 | **RESOLVED — stay opt-in.** 1.7 passed but 1.4–1.6 demoted; no production type justifies default-on. `wire` stays behind `--features wire`. |

**The non-obvious re-sequence:** Phase 2.0 (SSOT — frontend `domain` dep) must
land **before** Phase 1.3, not after. Otherwise wire types are unreachable from
the Leptos side without violating SSOT. The original plan had Phase 2 as a
separate track; in practice it is a hard dependency for Phase 1.

---

## What this audit deliberately refuses to claim

- **No claim that zero-copy will clear the GOAT-gate on production types.**
  The 1.7 benchmark cleared the gate on the smoke-test type (`LevelScore`).
  Every *production* type nominated (EventMeta, DepositStatus, AttendeeListItem)
  has variable-length strings that defeat pure Pod. The format infrastructure
  ships as opt-in; production application is a per-type cost/benefit decision.
- **No SIMD claim.** Plan 014 Phase 4 already concluded SIMD doesn't apply to
  this workload. This audit confirms: no dense `f32` math in any audited path.
- **No claim about `AttendeeListItemWire`.** Explicitly deferred. The original
  plan's "top 3 batch payload" framing led here; the audit says no.
- **No claim that the smoke endpoint is load-bearing.** `GET /api/wire-sample/level-score`
  is a probe, not a production endpoint. It can be removed in a future cleanup.
  It stays for now as a live confirmation that the worker→wasm path compiles
  and serves the format end-to-end.

---

## Task 1.7 — GOAT-gate result (CLEAR)

**Both thresholds cleared at every row count.** Run on host x86_64
(Apple M5 Pro); the bench is host-only because `criterion` is not wasm32-
compatible. The win on wasm32 is expected to be **larger**, not smaller, since
JSON parsers are 1.3–2× slower under wasm and `bytemuck::cast_slice` on wasm32
is a literal pointer reinterpret.

Payload: `Vec<LevelScore>` (16 B/row) under the slice wire envelope. JSON
fixture mirrors real adventure-score magnitudes (moves < 200, time < 600s,
stars 1-3, all 4 fields populated).

| Rows | JSON bytes | Wire bytes | Size shrink | Decode × | Encode × |
|-----:|----------:|-----------:|-----------:|---------:|---------:|
|   50 |   2,996   |     844    |   71.8 %   |   4.8×   |   3.2×   |
|  200 |  12,091   |   3,244    |   73.2 %   |   4.7×   |   3.2×   |
|  500 |  30,361   |   8,044    |   73.5 %   |   6.2×   |   4.2×   |
| 1000 |  60,721   |  16,044    |   73.6 %   |   7.6×   |   5.0×   |

**Gate thresholds:** ≥ 40 % payload reduction **and** ≥ 3× decode speedup.

- Payload reduction ≥ 40 %: cleared at every row count (smallest win 71.8 %).
- Decode speedup ≥ 3×: cleared at every row count (smallest win 4.7×).

**Two observations that change the rollout calculus:**

1. **The decode win scales with row count.** At 1000 rows the wire path is
   7.6× faster than JSON; at 50 rows only 4.8×. The wire format's value
   *grows* with payload size — so the TIER 1B admin endpoints (audience export,
   audit log) actually benefit more than the smaller TIER 1 endpoints.
2. **Encode wins too, not just decode.** The worker side benefits from skipping
   `serde_json::to_vec` — at 500 rows that's 20 µs → 4.7 µs (4.2×). On the
   worker this is CPU, and CPU is billable. The win is smaller than decode but
   still material on hot endpoints.

**Decision:** Proceed with Task 1.4 (`EventMetaWire`) and Task 1.5
(`DepositStatusWire`). The GOAT-gate experiment is concluded; the format earns
its maintenance surface.

**Caveats / what this bench does NOT prove:**

- These are `LevelScore` numbers (16 B/row, fixed-size, no `String`/`Option`).
  The production types (`EventMeta`, `DepositStatus`) will have smaller
  relative wins because their wire forms include inline byte arrays for what
  are currently `String` fields — the JSON-to-wire ratio is closer to 3× than
  20×. The GOAT-gate is still expected to clear, but the per-endpoint wins
  will be more modest than the headline numbers above suggest.
- This is host decode. Frontend (wasm32) decode should be checked before
  final rollout — the host numbers are a conservative lower bound.
- The benchmark does not exercise the HTTP boundary, KV cache hit, or the JS
  ↔ wasm `ArrayBuffer` copy. Those are integration-time concerns for Task 1.6
  / a future `wrangler dev` smoke.

Reproduce: `cargo bench -p event-checkin-domain --features wire --bench wire_bench`.
Source: `domain/benches/wire_bench.rs`. Results committed as text above;
  criterion's full report lives (gitignored) under `domain/target/criterion/`.

---

## References

- Original plan: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 1, L45-101)
- Sub-agent session IDs (full audit data): `2cc388a0`, `ad66442e`, `113baa38`
- katgpt-rs precedents cited: flat `&[f32]` over `Vec<Vec<f32>>` (1.8–5.1×),
  BLAKE3 commitment on every committed blob, GOAT FAILED → demote discipline
