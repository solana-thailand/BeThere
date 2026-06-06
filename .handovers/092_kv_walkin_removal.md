# Handover #092: KV Walk-in Removal (#046 Phase 2e)

> **Date**: 2026-06-06
> **Issue**: #046 — D1 as Primary Data Store
> **Status**: ✅ Complete — build clean, 190 tests pass

## What Happened

Completed the remaining work to fully remove KV as a dependency for walk-in attendee data. D1 is now the sole primary store for all walk-in records. KV is kept as a best-effort mirror only.

## Changes Made (7 files)

| File | Change |
|------|--------|
| `worker/src/handlers/attendee.rs` | Removed KV walk-in merge in `list_attendees`, removed `walkin_to_attendee()`, D1-first walk-in delete |
| `worker/src/handlers/walkin.rs` | D1-first in all handlers (duplicate check, list, export, sync, capacity), removed `find_walkin_by_any()` |
| `worker/src/handlers/register.rs` | D1-first walk-in capacity count |
| `worker/src/handlers/public_event.rs` | D1-first walk-in capacity count for public stats |
| `worker/src/claim/mint.rs` | D1-only walk-in claim lookup (no KV fallback), `execute_walkin_claim` writes D1 (primary) + KV (mirror) |
| `worker/src/claim/lock.rs` | Removed `lookup_walkin_by_claim_token()`, renamed `mark_walkin_claimed` → `mark_walkin_claimed_kv` |
| `worker/src/db/attendees.rs` | Added `delete_attendee()` D1 function |

## Data Flow After Phase 2e

```
READ (list_attendees):
  D1 (all attendees including walkins) → Sheets fallback

READ (claim lookup):
  D1 (walkin: participation_type='walkin') → Sheets (pre-registered)

WRITE (walkin register):
  D1 (primary) → KV (best-effort mirror) → Sheets (async)

WRITE (walkin claim):
  D1 (primary) → KV (best-effort mirror)

DELETE (walkin):
  D1 (primary) → KV cleanup (best-effort)

CAPACITY COUNT:
  D1 (count_walkin_attendees) → KV fallback (legacy)
```

## What Remains in KV (Not Removed)

- `walkin:{event_id}:{email}` — best-effort mirror (written alongside D1)
- `claim_walkin:{token}` — reverse mapping (written alongside D1)
- `walkin_synced:{event_id}:{email}` — sync marker for sheet sync
- Column map cache, Google token cache, staff cache, QR image cache — unrelated

These KV keys are now **write-only mirrors** — no code path reads from them as primary source anymore. They can be removed in a future cleanup pass.

## Build & Test

| Check | Result |
|-------|--------|
| `cargo check --target wasm32-unknown-unknown` | ✅ 0 errors, 0 warnings |
| `cargo clippy --target wasm32-unknown-unknown` | ✅ 0 warnings |
| Tests (190 total) | ✅ 73 + 81 + 15 + 21 — all pass |

## Migration Note

Existing walk-ins that were stored **only in KV** (before Phase 2d) need a one-time migration to D1. The `walkin_sync_handler` now reads from D1-first, so any KV-only walk-ins not yet in D1 won't appear in sync listings. A migration script should scan `walkin:*` keys and call `try_insert_walkin()` for each.

## Next Steps

1. **Deploy** via `deploy.sh` and verify attendee list latency < 50ms
2. **#045 VULN-007** — FNV-1a → blake3 on-chain (last security vulnerability)
3. **#043 Phase D** — Data Deletion API (legal compliance)
4. **#050 DO Deployment** — Retry when CF API recovers
