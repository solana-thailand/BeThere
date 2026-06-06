# Handover #093: KV Walk-in Mirror Writes Removal (#046 Phase 2f)

## What Happened

Completed **Issue #046 Phase 2f** — the final cleanup pass removing all remaining KV writes for walk-in attendee data. This builds on Phase 2e (which removed KV reads) by eliminating the best-effort KV mirror writes, making **D1 the sole store** for walk-in attendees with zero KV dependency.

## Changes Made

### Files Modified (7 files)

| File | Changes |
|------|---------|
| `worker/src/handlers/walkin.rs` | Removed `list_walkin_attendees()` (48-line KV scan function), `walkin_key()`, `claim_walkin_key()`, `WALKIN_TTL_SECS`. Removed KV mirror writes in `register_walkin` (walkin:*, claim_walkin:* keys). Removed KV sync marker writes in `sync_walkin_to_sheet` (walkin_synced:*). Removed KV fallbacks in `list_walkin_handler`, `walkin_export_csv_handler`, `walkin_sync_handler`. Simplified `enforce_walkin_capacity` to D1-only (removed 30-line KV list+scan fallback). Changed `walkin_sync_handler` idempotency to skip claimed walk-ins instead of checking KV sync markers. |
| `worker/src/handlers/register.rs` | Removed 32-line KV legacy fallback in `enforce_capacity` — D1-only walk-in counting. |
| `worker/src/handlers/public_event.rs` | Removed 32-line KV legacy fallback in `count_attendees_by_track` — D1-only. |
| `worker/src/handlers/deposit/usdc/mod.rs` | Replaced KV list+scan walk-in count with D1 `count_walkin_attendees()` call. |
| `worker/src/claim/mint.rs` | Removed `mark_walkin_claimed_kv()` call and import. Walk-in claim now writes D1 only. |
| `worker/src/claim/lock.rs` | Removed `mark_walkin_claimed_kv()` function (38 lines), `WALKIN_PREFIX` constant, `WalkinAttendee` import. |
| `worker/src/handlers/attendee.rs` | Removed KV walk-in key cleanup in `delete_attendee` (walkin:*, claim_walkin:*, walkin_synced:* key deletions). Kept claim lock cleanup only. |

### New File

| File | Purpose |
|------|---------|
| `worker/scripts/migrate_kv_walkins_to_d1.sh` | One-time migration script. Scans KV `walkin:*` keys → generates idempotent INSERT SQL → executes against D1. Supports `--dry-run`. |

## Data Flow After Phase 2f

```
READ (all paths):        D1 only — zero KV reads for walk-in data
WRITE (walkin register): D1 only — zero KV writes
WRITE (walkin claim):    D1 only — zero KV writes
DELETE (walkin):         D1 only — claim lock cleanup via KV (not walkin-specific)
CAPACITY COUNT:          D1 only
SYNC TO SHEET:           D1 read → Sheet write (no KV sync markers)
```

## KV Keys No Longer Used (Can Expire Naturally)

- `walkin:{event_id}:{email}` — was best-effort mirror, now no longer written
- `claim_walkin:{token}` — was reverse mapping, now no longer written
- `walkin_synced:{event_id}:{email}` — was sync marker, now no longer written/read
- These keys have 90-day TTLs and will expire naturally

## Build & Test

| Check | Result |
|-------|--------|
| `cargo check --target wasm32-unknown-unknown` | ✅ 0 errors |
| `cargo clippy --target wasm32-unknown-unknown` | ✅ 0 warnings |
| Tests (190 total) | ✅ 73 domain + 81 worker + 15 DO contract + 21 serde — all pass |

## Remaining for Full #046 Closure

1. **Deploy** and verify attendee list response time < 50ms from D1
2. **Run migration script** (`bash worker/scripts/migrate_kv_walkins_to_d1.sh`) to migrate any pre-Phase-2d KV-only walk-ins to D1
3. Verify D1 walk-in count >= KV walk-in key count post-migration

## Recommended Next Steps

1. ✅ **#046 Phase 2d-2e-2f** — DONE (needs commit + deploy)
2. **#045 VULN-007** — FNV-1a → blake3 on-chain (last security vulnerability)
3. **#043 Phase D** — Data Deletion API (legal compliance)
4. **#050 DO Deployment** — Retry when CF API recovers from persistent 10013 error
5. **#049 Developer CRM Phase 1** — New tables, depends on #046 completion
