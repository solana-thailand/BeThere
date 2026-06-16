# Handover #100: KV Write Elimination — Fit Free-Tier Write Quota Without Upgrading

## What Happened

Triggered by a Cloudflare email warning that the account had hit **50% of the daily Workers KV free-tier limit**. Rather than upgrade to the Workers Paid plan ($5/mo, 1M writes/day), the user asked whether the free tier (1,000 writes+deletes+lists/day) could be made safe for Demo Day without paying.

This session answered that with hard evidence and a code fix. Built a read-only KV usage diagnostic, proved the constraint was **writes** (not reads), traced the writes to redundant KV caching on top of D1, and eliminated the hot-path KV writes. The fix keeps the account on the free tier safely through Demo Day.

## The Evidence (real measurements, not estimates)

Built `scripts/diag_kv_usage.py` — queries the Cloudflare GraphQL Analytics API (`kvOperationsAdaptiveGroups` dataset) with schema introspection so it's resilient to Cloudflare schema drift. **Consumes zero KV operations** (analytics endpoint only), safe to run repeatedly.

Daily breakdown vs free-tier ceilings:

| Date (UTC) | Reads | Writes | Deletes | Lists | W+D+L | % W+D+L | % Reads |
|---|---|---|---|---|---|---|---|
| 2026-06-15 (warned) | 11,960 | **510** | 0 | 0 | **510** | **51.0%** ⚠️ | 12.0% |
| 2026-06-14 | 6,860 | 360 | 0 | 10 | 370 | 37.0% | 6.9% |
| 2026-06-16 (today, since reset) | 3,940 | 240 | 0 | 10 | 250 | 25.0% | 3.9% |

Two decisive findings from the hourly breakdown of 2026-06-15:

1. **The 50% warning was about WRITES, not reads.** The W+D+L bucket (1,000/day) hit 51%; reads (100,000/day) were only 12%. Reads were never the constraint.
2. **Writes are STEADY per-request traffic, not the cron.** 510 writes spread across 18 separate hours, peak hour only 20% of total. The cleanup cron window (03:00 UTC) showed 0 writes / 0 lists / 0 deletes — the cron was NOT the driver. 100% of ops were on the EVENTS namespace.

Verdict: the writes came from **per-request application dual-writes and a KV write-back** in `event_store`.

## Root Cause

The `EVENTS` KV namespace is bound in production, but D1 is already the source of truth for all the data it caches. Four hot-path code paths were redundantly writing to KV on every relevant request:

1. `resolve_event_or_fallback` — on a KV miss that succeeded in D1, it wrote the config **back to KV** to "rebuild the cache". Fired on every event resolution that missed KV — the biggest source.
2. `save_deposit_status_with_fallback` — wrote D1, then best-effort wrote KV.
3. `save_deposit_status` (raw) — wrote D1, then **unconditionally** wrote KV + maintained a KV attendee list. Called directly by check-in handlers.
4. `save_escrow_index` — dual-wrote D1 + KV.

All read paths are already **D1-first with KV fallback** (`get_deposit_status_with_fallback`, `list_deposit_statuses`, `get_event_id_by_escrow`, etc.), so the KV cache was strictly redundant in production (where D1 is bound). Issue #053 Phases 3b/3e acceptance criteria literally already stated *"KV keys no longer read or written"* for deposit status and the escrow index — the migration was marked complete but the write path was never actually removed. This session closes that gap.

## Changes Made

Branch: `feature/kv_write_elimination` (2 commits, both on branch; NOT merged to `develop` yet).

### Commit 1 — `c27bbcc` — `feat(scripts): add KV usage diagnostic`

| File | Purpose |
|------|---------|
| `scripts/diag_kv_usage.py` | New (~600 lines). Read-only KV quota diagnostic. Daily report + `--detail YYYY-MM-DD` hourly/per-action/per-namespace breakdown. Schema-introspecting so it survives Cloudflare GraphQL drift. |

### Commit 2 — `ebd0e97` — `fix(event_store): eliminate redundant KV writes`

| File | Function | Change |
|------|----------|--------|
| `worker/src/event_store/read.rs` | `resolve_event_or_fallback` | Removed the `save_event_config(kv, &config)` write-back on D1-fallback hit. |
| `worker/src/event_store/read.rs` | `save_deposit_status_with_fallback` | Removed the KV best-effort write block. Signature keeps `_kv` for API stability. |
| `worker/src/event_store/write.rs` | `save_escrow_index` | Removed the KV dual-write block. Signature keeps `_kv` for API stability. |
| `worker/src/event_store/write.rs` | `save_deposit_status` | Made D1-first with early `return Ok(())` when D1 present (mirrors `save_thb_deposit`). KV path retained as legacy fallback for D1-absent deployments. |

### Intentionally NOT changed

- **Admin/CRUD writes** (`create_event`, `update_event`, `hard_delete_event`, `save_event_config`, `save_event_index`) — rare operations; left alone so the KV cache stays fresh for admin reads.
- **`delete_escrow_index`** — rare (event deletion); keeping the KV delete helps clean up legacy entries.
- **`increment_deposit_counter_with_fallback`** — already D1-first, only writes KV if D1 unavailable.
- **`save_thb_deposit`** — was ALREADY D1-first with early return; no change needed.
- **The `EVENTS` KV binding in `wrangler.toml`** — kept. The versions API requires binding parity with production, and `None`-KV handling already exists throughout. The binding just stops getting written to on the hot path.

## Build & Test

| Check | Result |
|-------|--------|
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ clean |
| `cargo clippy -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ 0 warnings (only a benign workspace profile-location note) |
| `cargo test -p event-checkin-worker` | ✅ **123/123 pass** (87 unit + 15 do_claim_lock + 21 serde_contract) |
| `diagnostics` on both edited files | ✅ no errors, no warnings |

No call sites changed. All read paths verified D1-first with KV fallback — removing the writes is safe in production.

## ⚠️ Critical: Fix is NOT live until deployed

The code change is **local only** — production still runs the old worker and will keep burning ~510 writes/day until deploy. To take effect before Demo Day:

```bash
cd worker && ./deploy.sh
```

**Proof the fix works** = re-run the diagnostic after deploy and the next day:
```bash
python3 scripts/diag_kv_usage.py --days 3
```
Expected: writes drop from ~510/day to near-zero (only rare admin CRUD). The `--detail` mode confirms the per-request write pattern is gone.

**Known deploy hazard**: `deploy.sh` works around the wrangler `/versions` API bug (CF error 10013) by falling back to the PUT API + manual asset upload. The fallback path is battle-tested but worth watching for the "✅ Deployed successfully" line.

## Demo Day Quota Math (free tier, post-deploy)

With the fix live, projected daily writes:
- Hot-path writes (event resolution, check-ins, escrow): **~0** (eliminated)
- Admin CRUD writes: a handful (event config edits)
- Cleanup cron (03:00 UTC): a few list/delete ops if any events have expired (none yet — event is in the future)

**Headroom: ~99% of the 1,000/day write bucket**, even with a Demo Day check-in spike. Each check-in previously cost 2–3 KV writes; now it costs 0. Free tier is safe.

## Merge / Deploy Plan

1. **Review**: `git log feature/kv_write_elimination` (2 commits ahead of `develop`)
2. **Merge to develop**: `git checkout develop && git merge --ff-only feature/kv_write_elimination` (clean FF — `develop` hasn't moved since branch creation)
3. **Deploy**: `cd worker && ./deploy.sh`
4. **Verify**: next day, run `python3 scripts/diag_kv_usage.py --days 3` — writes should be near-zero

`develop` is currently 22 commits ahead of `origin/develop` (local-only per standing instruction). This branch adds 2 more.

## Reflection

**What struggled**: Cloudflare's GraphQL Analytics schema. First attempt hardcoded a dataset name that didn't exist; rewrote the script to introspect `__schema` and discover the dataset (`kvOperationsAdaptiveGroups`) and its fields dynamically. Also the dataset uses a single `requests` counter + an `actionType` dimension (not separate read/write/delete sum fields), which required bucketing by dimension rather than by field name. The `rg`/`grep` IDE tool also returned false "No matches" several times this session — worked around by reading files directly and using `rg` in the terminal.

**What solved it**: the hourly breakdown was the killer evidence. Daily totals would have left the cron as a plausible suspect; the hourly view showed 03:00 UTC had zero ops, definitively pointing at per-request traffic. And reading issue #053's acceptance criteria — which already declared these writes removed — reframed the fix from "risky pre-demo change" to "making code match the documented intent."

**Key honesty note**: the prior session's handover 098/099 flagged pitch-deck overclaims; this session found a different kind of gap — issue #053 marked Phases 3b/3e "✅ COMPLETE" when the write path wasn't actually removed. Same theme (truth vs. claim) in a different artifact.

## Remaining Work

| Priority | Item | Notes |
|----------|------|-------|
| 1 | **Deploy** | `cd worker && ./deploy.sh` — fix is local only until then |
| 2 | **Verify next day** | `python3 scripts/diag_kv_usage.py --days 3` — writes should be near-zero |
| 3 | **Merge to develop** | `git merge --ff-only feature/kv_write_elimination` |
| 4 | **(Optional) Finish issue #053** | Remaining KV write sites are admin-only (create/update/delete event). Could be removed for full D1-only purity, but low priority — they're rare and don't threaten any quota. |
| 5 | **(Optional) Remove EVENTS KV binding** | Once all writes confirmed gone, the binding itself could be removed. Blocked by the versions-API binding-parity constraint noted in `wrangler.toml`. |

## How to Dev/Test

```bash
# Re-run the diagnostic anytime (safe — no KV ops consumed)
python3 scripts/diag_kv_usage.py --days 3
python3 scripts/diag_kv_usage.py --detail 2026-06-15

# Verify the worker still builds for wasm
cargo check -p event-checkin-worker --target wasm32-unknown-unknown
cargo clippy -p event-checkin-worker --target wasm32-unknown-unknown

# Run the worker test suite
cargo test -p event-checkin-worker

# Deploy (when ready to take effect)
cd worker && ./deploy.sh
```

## Issues Ref

- **#053** (KV→D1 migration) — Phases 3b (on-chain index) and 3e (deposit status) acceptance criteria now actually match the code. Phases 3a/3c/3d remain as documented.
- **#011** (sustainability) — daily cleanup cron unaffected; confirmed not the write source.

## Submission Timeline Reminder

- Demo Day: **Jun 23** (IslandDAO V4 Hackathon, Koh Samui)
- Submission deadline: **Jun 22 midnight UTC**
- The fix must be deployed before Demo Day for the free-tier quota to hold under check-in load.