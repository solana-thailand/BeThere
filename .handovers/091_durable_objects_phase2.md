# 091: Durable Objects Phase 2 — Check-in, Claim, Upsert Operations

## What Happened
Continued from Handover #090. Implemented Phase 2 of the DO migration: added 4 new `DoRequest` RPC variants (CheckIn, UndoCheckIn, ClaimAttendee, UpsertAttendee) with full handlers, an `attendees` SQLite table inside the DO, D1 sync helpers, and 11 new serde contract tests. **All 117 tests pass, 0 diagnostics.**

**Deployment remains blocked** — CF API still `degraded_performance`. DO binding still commented out in `wrangler.toml`. Phase 2 code is compiled in but dormant (same as Phase 1).

## Deploy Attempt (2026-06-06)
Tried `npx wrangler deploy` with DO binding uncommented:
- ✅ Build successful (4.2MB / 1.3MB gzip)
- ✅ DO binding recognized: `env.EVENT_DO → Durable Object EventDurableObject`
- ✅ All bindings listed correctly (KV, D1, R2, env vars)
- ❌ Upload step fails: CF API error 10013 "An unknown error has occurred"
- CF API status page: `degraded_performance` (Durable Objects service itself is `operational`)
- Root cause: Cloudflare infrastructure issue, not our config
- **Reverted** DO binding to commented state — deploy.sh fallback (PUT API) doesn't support DO bindings (error 10021)

Also fixed: `shim.mjs` was missing `export { EventDurableObject }` — added to wrangler.toml shim template.

## Uncommitted Changes
- `worker/src/durable_objects/event_do.rs` — +691 / -23 lines (Phase 2 implementation)
- `worker/wrangler.toml` — +1 line (EventDurableObject shim export fix)

## Commits Needed
- `feat: DO Phase 2 — check-in, undo, claim attendee, upsert attendee operations`
- `fix: shim exports EventDurableObject for DO binding`

## Architecture (Updated)

```
DoRequest RPC variants (7 total)
├── Phase 1 — Claim Lock
│   ├── AcquireClaimLock → handle_acquire_claim_lock
│   ├── FinalizeClaimLock → handle_finalize_claim_lock
│   └── ReleaseClaimLock → handle_release_claim_lock
└── Phase 2 — Attendee Writes (NEW)
    ├── CheckIn → handle_check_in (idempotent, same-token guard)
    ├── UndoCheckIn → handle_undo_check_in
    ├── ClaimAttendee → handle_claim_attendee (resolves via claim_token)
    └── UpsertAttendee → handle_upsert_attendee (ON CONFLICT upsert)

DO SQLite Tables
├── claim_locks (Phase 1)
└── attendees (Phase 2) — mirrors D1 schema write columns
    ├── Indexes: idx_attendees_event_id, idx_attendees_claim_token
    └── sync_attendee_to_d1() → fire-and-forget D1 upsert
```

## New Code Details

### DoRequest Variants Added
| Variant | Action Tag | Fields |
|---------|-----------|--------|
| `CheckIn` | `"check_in"` | attendee_id, event_id, checked_in_at, checked_in_by, claim_token |
| `UndoCheckIn` | `"undo_check_in"` | attendee_id, event_id |
| `ClaimAttendee` | `"claim_attendee"` | event_id, claim_token, claimed_at, claim_asset_id, claim_signature |
| `UpsertAttendee` | `"upsert_attendee"` | id, event_id, email, name, approval_status, participation_type, contact_channel, contact_handle |

### Handler Methods Added
- `handle_check_in` — idempotent: returns ok if already checked in with same claim_token, error if different token
- `handle_undo_check_in` — clears checked_in_at, checked_in_by, claim_token
- `handle_claim_attendee` — writes NFT claim result, resolves attendee ID via `resolve_attendee_id_by_claim_token`
- `handle_upsert_attendee` — takes `UpsertAttendeeParams<'a>` (avoids clippy too_many_arguments), ON CONFLICT upsert

### D1 Sync Helper
- `sync_attendee_to_d1()` — reads full attendee row from DO SQLite, upserts all 14 columns to D1, fire-and-forget

### Deserialization Structs
- `ExistingCheckIn` — for idempotency check in handle_check_in
- `AttendeeSyncRow` — for D1 sync row reading
- `UpsertAttendeeParams<'a>` — parameter struct for handle_upsert_attendee

## Test Results
| Suite | Count | Status |
|-------|-------|--------|
| Unit tests (lib) | 81 | ✅ All pass |
| DO claim lock contract | 15 | ✅ All pass |
| Serde contract | 21 | ✅ All pass |
| **Total** | **117** | **0 failures** |

11 new tests added (24 DO tests total).

## Deployment Status
- CF API: still `degraded_performance`
- Durable Objects service: `operational`
- Production commit: `5f3ed04` (Phase 1 + tests, no DO binding)
- DO binding: **commented out** in wrangler.toml (lines 140-146)
- All code handles `event_do=None` gracefully

## What's Next

### Immediate — Commit Phase 2 + Shim Fix
- [x] Commit Phase 2 code
- [x] Commit shim export fix
- [ ] Periodically retry `npx wrangler deploy` (uncomment DO binding first)

### DO Deployment (blocked by CF API 10013)
When CF API recovers:
1. Uncomment DO binding + migration in `wrangler.toml` (lines 140-146)
2. Run `npx wrangler deploy`
3. Verify `EVENT_DO` in `/api/health` response
4. Test claim lock through DO (existing Phase 1 routing in `claim/lock.rs`)

### Phase 2 Integration (requires DO deployment first)
Template: `claim/lock.rs` pattern (DO first, D1 fallback).
- [ ] Route `checkin.rs::check_in` through DO (`CheckIn` action) with D1 fallback
- [ ] Route `checkin.rs::undo_check_in` through DO (`UndoCheckIn` action) with D1 fallback
- [ ] Route `claim/mint.rs::execute_claim` through DO (`ClaimAttendee` action) with D1 fallback
- [ ] Route `register.rs` / `walkin.rs` through DO (`UpsertAttendee` action) with D1 fallback

### Non-DO Improvements (can do NOW)
- [x] Replace O(n) full-table-scan in `get_attendee_with_claim_counts` with targeted SQL (commit `ebf41bf`)
- [x] Batch `write_developer_data` 5 sequential inserts into 1 D1 batch (commit `feca436`, saves 4 round-trips)
- [ ] Parallelize the 3 D1 writes in `register_attendee` via `join!`
- [ ] Parallelize quiz+adventure checks with attendee lookup in `execute_claim`
- [ ] Replace `check_walkin_duplicate` + `upsert_walkin_attendee` with single INSERT ON CONFLICT
- [ ] Add `count_walkin_attendees` for `enforce_walkin_capacity` D1 fallback

## How to Dev/Test
```bash
# Build check
cd worker && cargo check --target wasm32-unknown-unknown

# Run all tests
cargo test

# Deploy (uncomment DO binding in wrangler.toml first)
cd worker && npx wrangler deploy

# Verify production
curl -s https://bethere.solana-thailand.workers.dev/api/health | python3 -m json.tool
```

## Key Reference Files
- `worker/src/durable_objects/event_do.rs` — DO implementation (1284 lines, 24 tests)
- `worker/src/claim/lock.rs` — claim lock routing pattern (DO first, D1 fallback) — template for Phase 2 integration
- `worker/src/handlers/checkin.rs` — check-in handler (Phase 2 routing target)
- `worker/src/handlers/claim.rs` — claim handler (Phase 2 routing target)
- `worker/src/db/attendees.rs` — D1 write functions (will become fallback)
- `.handovers/090_durable_objects_phase1.md` — Phase 1 context
