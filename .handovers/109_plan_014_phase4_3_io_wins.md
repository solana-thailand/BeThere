# Handover 109 — Plan 014 Phase 4.3 I/O Wins Audit (Mostly Negative Results)

> **Branch**: `feature/014_io_wins` (2 commits ahead of `develop`, pushed-status pending)
> **Status**: ✅ **Committed + ready to merge.** Builds clean, 285 tests pass, clippy zero warnings. Of the 4 Phase 4.3 candidates: **1 shipped (4.3.2), 2 already satisfied by existing code (4.3.1, 4.3.3), 1 demoted as unsafe (4.3.4).** NOT deployed.
> **Commits**: `c6f89d2` (perf — 4.3.2), `e1bb740` (docs — 4.3 audit conclusion + 4.3.4 demotion)
> **Predecessor**: handover #108 (Plan 014 Phase 1 wire format infrastructure)
> **Plan**: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 4.3 now CONCLUDED; Phases 2, 4.1, 4.4, 5 still open)
> **Created**: 2026-06-27

---

## 1. What Happened

Investigated all four Phase 4.3 "green software" I/O-win candidates against the **actual current codebase** before implementing any. The audit found that **Plan 014's premises were wrong for 3 of the 4 items, and unsafe for the 4th** — mirroring the pattern of audit misses already documented in `.plans/014_negative_results.md` (this is now the 5th consecutive miss; entries 4, 5, 6, 8, 9).

The honest outcome of Phase 4.3 is:

| Task | Plan's claim | Actual code reality | Outcome |
|---|---|---|---|
| 4.3.1 | "currently uncached" event-series endpoint | Already server-cached at 120s via `cache_public_120_layer` (Plan 013) | ✅ Already done |
| 4.3.2 | "3 sequential KV reads" in `get_public_ticket` | event→attendee is a dependency chain; only 2 reads are independent | ✅ Shipped (scope corrected) |
| 4.3.3 | "one PUT per answer" in quiz/adventure | `submit_quiz` grades all answers in-memory, writes once | ✅ Already done |
| 4.3.4 | "blockhash valid ~120s" → promote TTL to 90s | Confuses `MAX_HASH_AGE_IN_SECONDS=120` with `MAX_PROCESSING_AGE=150 blocks` (~60–90s). 90s cache would cause stale-blockhash failures | ❌ Demoted (unsafe) |

This is the katgpt-rs "GOAT FAILED → demote" discipline working as designed: every change was GOAT-gated against actual call-site semantics before implementation, and only changes that genuinely clear the bar shipped. The one that shipped (4.3.2) shipped with a **smaller scope than the plan framed** — the plan's "3 sequential reads" was audited down to "2 independent reads," because the event→attendee pair is a dependency chain.

---

## 2. Changes (2 commits, +150 lines)

### `c6f89d2 perf(ticket): parallelize USDC + THB deposit reads in get_public_ticket (Plan 014 Phase 4.3.2)`

**`worker/src/handlers/attendee.rs`** (+23 / −22 lines)

Collapsed the two independent deposit reads in `get_public_ticket` into a single concurrent step via `futures_util::join!`:

- `get_deposit_status_with_fallback` (USDC) and `get_thb_deposit_with_fallback` (THB) both depend only on `(event.id, attendee.api_id)` and are independent of each other.
- Previously they were fetched **sequentially across ~80 lines of intervening QR-backfill logic** — the THB read sat at line 361, far below the USDC read at line 285.
- Now they fan out concurrently, turning two sequential D1/KV round-trips into one. Follows the established pattern at `worker/src/handlers/deposit/escrow/status.rs:88`.
- The `deposit_status` result still flows into the `recover_and_verify_deposit` self-heal + QR-backfill logic unchanged; `thb_deposit` stays in scope for the `deposit_info` JSON construction downstream.
- Pure latency win, zero compute cost, zero behavior change.

### `e1bb740 docs(plan): conclude Plan 014 Phase 4.3 audit — 4.3.1/4.3.3 already done, 4.3.4 demoted`

**`.plans/014_katgpt_rs_paradigm_migration.md`** — Phase 4.3 marked `[x]` CONCLUDED with per-task notes explaining the audit findings and the scope correction on 4.3.2.

**`.plans/014_negative_results.md`** — Added entry #9 (Solana blockhash cache TTL promotion, demoted). Added the entry to the GOAT-gate outcomes reference table. Updated the "Pending evaluations" section to record that Phase 4.3 is concluded.

---

## 3. Per-task Audit Findings (the meat of this handover)

### 4.3.1 — Event-series KV cache → ALREADY SATISFIED ✅

**Plan's claim:** "KV cache the public event-series endpoint (Plan 013) at 120s — currently uncached, every ticket view hits D1."

**Reality:** The endpoint is **already server-cached at exactly 120s**. `worker/src/handlers/mod.rs:50-57` registers `/public/event-series/{event_id}` under the `public_events_detail` sub-router, which applies `cache_public_120_layer`:

```rust
// Public event detail: 120s cache (individual events rarely change)
let public_events_detail = Router::new()
    .route("/public/event/{slug}", get(public_event::get_public_event))
    // Event series (related events / prev-next). Shares the 120s cache —
    // series structure changes rarely and the payload is derived from
    // campaign_events + events, both already cached at this granularity.
    .route(
        "/public/event-series/{event_id}",
        get(event_series::get_event_series),
    )
    .layer(middleware::from_fn(crate::middleware::cache_public_120_layer));
```

The plan's "currently uncached" premise was written **before Plan 013 shipped** (issue #060, which created this endpoint with the cache layer attached). Nothing to do.

### 4.3.2 — Parallelize `get_public_ticket` reads → SHIPPED (scope corrected) ✅

**Plan's claim:** "Collapse the 3 sequential KV reads in `get_public_ticket` (event → attendee → lock) into `join!`."

**Reality:** The reads form a **dependency chain, not a fan-out**:

1. `resolve_event` → reads event config
2. `sheets::get_attendee_by_id` → **needs `event.sheet_id` from step 1**
3. `get_deposit_status_with_fallback` → needs `event.id` + `attendee.api_id` from steps 1+2
4. `recover_and_verify_deposit` → conditional self-heal, depends on deposit status
5. `get_thb_deposit_with_fallback` → needs `event.id` + `attendee.api_id` (independent of #3 and #4)

Steps 1 and 2 **cannot** be parallelized. But steps 3 and 5 are genuinely independent (both read different D1 tables keyed only by `event.id` + `attendee.api_id`). That's the real win — 2 reads instead of the plan's implied 3.

The fix (commit `c6f89d2`) `join!`s those two. The plan also mislabeled the third read as a "lock" — there is no lock read in `get_public_ticket`; that may have been a confusion with `get_claim` which does read a claim lock.

### 4.3.3 — Batch quiz/adventure writes → ALREADY SATISFIED ✅

**Plan's claim:** "Batch the quiz/adventure KV writes — currently one PUT per answer; batch into a single PUT per submit."

**Reality:** The per-answer-write anti-pattern **does not exist**. `worker/src/quiz.rs:356` (`submit_quiz`) grades all answers in-memory in a loop, then calls `save_quiz_progress` **once**:

```rust
// Grade each enabled question (in-memory, no writes)
for question in &enabled_questions { ... correct_count += 1; ... }

// ... update progress struct in-memory ...

save_quiz_progress(d1, kv, event_id, &progress).await?;  // ONE write
```

The adventure path is the same — `save_level_completion` does one write per `/adventure/{token}/save` request, and each request naturally corresponds to one level completion (the right granularity). The plan's "one PUT per answer" premise was simply wrong.

### 4.3.4 — Blockhash TTL 30s → 90s → DEMOTED (unsafe) ❌

**Plan's claim:** "Promote the Solana blockhash cache TTL from 30s to the ~90s effective lifetime (blockhash valid ~120s) — halves RPC calls."

**Reality:** The "blockhash valid ~120s" premise is **factually wrong** and acting on it would introduce real bugs. The technical analysis (full reasoning in `.plans/014_negative_results.md` entry #9):

1. **The "~120s" confuses two unrelated Solana constants.** The plan cites `MAX_HASH_AGE_IN_SECONDS = 120`, but that constant sizes the recent-blockhash ring buffer (`MAX_RECENT_BLOCKHASHES = 300`). The actual transaction validity limit is `MAX_PROCESSING_AGE = 150` **blocks** (`solana-sdk/clock/src/lib.rs`: "The maximum age of a blockhash that will be accepted by the leader"). Transactions referencing an older blockhash are rejected.

2. **150 blocks ≠ 120 seconds.** Block time is ~400ms/slot but ~5% of slots are skipped, so 150 blocks takes **~60–90s wall-clock** in practice. Ecosystem references agree: Flash Trade "~60s"; Chainstack "~80-90s"; Helius "~2 minutes" upper bound. None support 120s.

3. **BeThere uses `"commitment": "finalized"`, which makes it worse.** Solana's transaction-confirmation guide warns finalized commitment *"effectively reduces the expiration of your transactions by about 13 seconds"* (finalized is ~32 slots behind confirmed). The blockhash is **already ~13s stale at fetch time**, before the cache TTL starts counting.

4. **The concrete failure mode:** A 90s cache TTL would let the worker hand the frontend a blockhash up to ~90s (cache) + ~13s (finalized staleness) ≈ **103s old** — past the ~60–90s validity window. Result: intermittent "Blockhash not found" / "block height exceeded" transaction failures, exactly the symptom class handover #086 already documented during rollover testing.

The current 30s is correct and defensible: worst-case age at frontend submission ≈ 30s (cache) + ~13s (finalized) + ~15s (wallet signing) + ~2s (network) ≈ ~60s — comfortably within the 150-block window. The existing code comment ("Solana blockhashes expire after ~60s on mainnet; 30s gives a good trade-off") is accurate.

---

## 4. Validation Done

| Check | Method | Result |
|---|---|---|
| Workspace compiles | `cargo check --workspace --all-targets` | ✅ EXIT 0 (only pre-existing profile warning) |
| Workspace tests | `cargo test --workspace` | ✅ **285 tests, 0 failed** (84 + 9 + 144 + 15 + 33) |
| Workspace clippy | `cargo clippy --workspace --all-targets` | ✅ **Zero warnings** |
| Solana protocol constants | `Solana_Documentation_Search` for blockhash validity | ✅ Confirmed `MAX_PROCESSING_AGE = 150` blocks; ecosystem docs confirm ~60–90s wall-clock |
| `join!` pattern precedents | `rg "futures_util::join!"` in `worker/src/` | ✅ 5 existing call sites — followed `deposit/escrow/status.rs:88` (same semantic shape) |
| Dependency-graph audit | Manual read of `get_public_ticket` call sequence | ✅ Confirmed only USDC + THB deposit reads are independent |
| Quiz write-path audit | Read of `worker/src/quiz.rs:356` `submit_quiz` | ✅ Confirmed one `save_quiz_progress` call per submit (not per answer) |
| Event-series cache audit | Read of `worker/src/handlers/mod.rs:50-57` | ✅ Confirmed `cache_public_120_layer` already applied |

---

## 5. Reflections

### What went well

- **The "audit-before-implement" discipline prevented two no-op commits and one harmful commit.** Had I mechanically implemented the plan as written, 4.3.1 and 4.3.3 would have been redundant no-ops (the code already does what the plan asks), and 4.3.4 would have introduced intermittent stale-blockhash failures. Only 4.3.2 had a real win, and even that needed scope correction.
- **The `join!` change followed an existing precedent exactly.** `worker/src/handlers/deposit/escrow/status.rs:88` already does `(deposits_res, thb_res) = futures_util::join!(...)` for the same semantic shape — fetching USDC and THB deposit data concurrently. Copying that pattern made the 4.3.2 change a low-risk, stylistically-consistent diff.
- **The Solana protocol constant research was decisive.** Without checking `MAX_PROCESSING_AGE = 150` against the plan's "~120s" claim, I might have trusted the plan and shipped a harmful change. The `Solana_Documentation_Search` tool surfaced the authoritative source (`solana-sdk/clock/src/lib.rs`) plus three ecosystem references that triangulated the ~60–90s real-world window.

### What was struggled with

- **The plan's misframing of the third read as a "lock."** `get_public_ticket` has no lock read — claim locks live in the `/claim/{token}` path. The plan may have conflated the two endpoints. This made the "3 sequential reads" claim initially hard to map onto the actual code until I read the full handler.
- **`find_path` glob limitations.** `find_path` with `worker/src/**/*.rs` returned no results repeatedly, even though the files exist. Worked around with `fd` via the terminal tool (`fd -e rs . src | xargs rg -l ...`). Worth noting for future agents: prefer `fd`/`rg` via `terminal` over `find_path` for deep recursive searches.

### What was solved

- **The scope correction on 4.3.2 is documented in the plan itself**, not just the commit message. The plan's `[x]` entry for 4.3.2 now explains that the "3 sequential reads" was overstated and what actually shipped. Future agents reading the plan will see the corrected scope, not the original over-claim.
- **The blockhash TTL demotion has explicit re-open preconditions** in entry #9 (switch to `confirmed` commitment; protocol-level `MAX_PROCESSING_AGE` change; durable-nonce path). If any of those happen, the decision can be revisited — not cargo-culted either way.

---

## 6. Remaining Work

### Plan 014 — still open

- [ ] **Phase 2.1** — Cross-crate SSOT audit (participation-type normalization, deposit-status enum mapping). The deposit/refund subset (2.3) was investigated and demoted (negative-results entry #8); the remaining candidates need their own audit before any moves.
- [ ] **Phase 2.3** — Build the forward-looking CI check that flags *future* business-predicate duplication across `worker/` and `frontend-leptos/`. Value is forward-looking — the retroactive remediation list is near-empty.
- [ ] **Phase 4.1** — Profile a staged 200-attendee event end-to-end. Confirms (or refutes) the I/O-bound hypothesis with data. **Blocked on infrastructure coordination.**
- [ ] **Phase 4.4** — Document the "no SIMD" decision in `.plans/014_no_simd.md` with profile evidence from 4.1 (blocked on 4.1).
- [ ] **Phase 5.3 / 5.4** — Deterministic-not-stochastic lint; zero-allocation hot-path audit.

### Operational

- [ ] **Merge `feature/014_io_wins` to `develop`** (fast-forward; 2 commits sit directly on top of `develop`).
- [ ] **Push `develop` to `origin`** (will be 2 commits ahead after merge).
- [ ] **Deploy** when ready. The `join!` change is additive and behavior-preserving; rollback is `wrangler rollback`. No D1/R2/KV schema changes.

---

## 7. How to Dev/Test

### Build + verify the 4.3.2 change

```bash
git fetch origin
git checkout feature/014_io_wins   # or develop after merge
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets
cargo test --workspace
```

### Confirm the `join!` is wired correctly

The two deposit reads should now run concurrently. Grep for the join:

```bash
rg -n "join!" worker/src/handlers/attendee.rs
# Should show: let (usdc_status_res, thb_deposit_res) = futures_util::join!(...)
```

### Verify the existing 4.3.1 cache layer is in place

```bash
rg -n "cache_public_120_layer" worker/src/handlers/mod.rs
rg -n "event-series" worker/src/handlers/mod.rs
# The event-series route should sit inside the sub-router that applies the 120s layer.
```

### Verify the existing 4.3.3 single-write quiz path

```bash
rg -n "save_quiz_progress" worker/src/quiz.rs
# submit_quiz should call save_quiz_progress exactly once (not inside the grading loop).
```

### Re-read the blockhash TTL demotion reasoning

```bash
sed -n '/## 9\. Solana blockhash/,/^---/p' .plans/014_negative_results.md
```

---

## 8. Issues Ref

- Plan: `.plans/014_katgpt_rs_paradigm_migration.md` (Phase 4.3 CONCLUDED)
- Negative-results log: `.plans/014_negative_results.md` (entry #9 — blockhash TTL)
- Predecessor: handover #108 (Plan 014 Phase 1 wire format infrastructure)
- Branch: `feature/014_io_wins` (2 commits, ready to merge to `develop`)
- Solana protocol source: `solana-sdk/clock/src/lib.rs` — `MAX_PROCESSING_AGE = 150`, `MAX_HASH_AGE_IN_SECONDS = 120`
- Solana transaction-confirmation guide: finalized commitment reduces expiration by ~13s
- Existing `join!` precedent: `worker/src/handlers/deposit/escrow/status.rs:88`
- No issue tracker entry — this is plan-driven work, not an issue remediation

---

## 9. Commit Plan

Two commits on `feature/014_io_wins`, to be fast-forward merged to `develop`:

1. `c6f89d2 perf(ticket): parallelize USDC + THB deposit reads in get_public_ticket (Plan 014 Phase 4.3.2)`
2. `e1bb740 docs(plan): conclude Plan 014 Phase 4.3 audit — 4.3.1/4.3.3 already done, 4.3.4 demoted`

**Status:**
- ✅ Committed at `c6f89d2` and `e1bb740`
- ✅ Validated: `cargo check`/`test`/`clippy` all clean; 285 tests pass
- ⏳ Operator: merge to `develop` → push to `origin` → deploy when ready