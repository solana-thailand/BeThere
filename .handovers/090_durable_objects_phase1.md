# 090: Durable Objects Phase 1 — DO Skeleton + Claim Lock ACID

## What Happened
Implemented Phase 1 of Issue #050: `EventDurableObject` with SQLite storage for ACID claim lock operations. Code compiles clean. **Deployment blocked by Cloudflare API 10013 error** — transient infrastructure issue, needs retry.

## Commits
- `e23ec2b` — `feat: Durable Objects Phase 1 — claim lock ACID operations via EventDurableObject`
- `359d28a` — `fix: shim exports EventDurableObject for DO binding, update deploy.sh`

## Files Created
| File | Purpose |
|------|---------|
| `worker/src/durable_objects/mod.rs` | Module index, re-exports `EventDurableObject` + `DoRequest` |
| `worker/src/durable_objects/event_do.rs` | DO class with SQLite claim lock CRUD + D1 sync |

## Files Modified
| File | Change |
|------|--------|
| `worker/src/lib.rs` | Added `durable_objects` module + `pub use EventDurableObject` |
| `worker/src/state.rs` | Added `event_do: Option<ObjectNamespace>` to `AppState` + `CachedBindings` |
| `worker/src/claim/lock.rs` | Added `do_rpc()` helper. All 3 lock functions now try DO first, fall back to D1+KV. Added `event_do` param. |
| `worker/src/claim/mint.rs` | All 6 call sites updated to pass `state.event_do.as_ref()` |
| `worker/wrangler.toml` | Added `[[durable_objects.bindings]]` EVENT_DO + `[[migrations]]` v1 with `new_sqlite_classes` |
| `worker/deploy.sh` | Added DO binding to metadata (commented out for PUT API — not supported) |

## Architecture

```
Client Request
     │
     ▼
Worker (Rust/Axum)
     │
     ├─ claim lock ops → EventDurableObject (sharded by event_id)
     │                     │
     │                     └─ SQLite (ACID, single-threaded per event)
     │                          │
     │                          └─ async sync → D1 (fire-and-forget)
     │
     └─ all other reads/writes → D1 (unchanged)
```

## DO RPC Protocol
Worker sends `DoRequest` as JSON `POST http://internal/do` to the DO stub:
- `{"action":"acquire_claim_lock", ...}` → check existing lock, insert if none
- `{"action":"finalize_claim_lock", ...}` → update with asset_id, signature, claimed_at
- `{"action":"release_claim_lock", ...}` → delete lock row

DO returns `{"success": bool, "error": string|null}`.

## D1 Sync Pattern
After every DO SQLite write, reads the changed row and upserts to D1 via `wasm_bindgen_futures::spawn_local()` (fire-and-forget). If sync fails, D1 is stale but DO is source of truth.

## Build Status
- `cargo check --target wasm32-unknown-unknown` ✅ Clean
- `cargo clippy` — 4 warnings (pre-existing style, `too_many_arguments` on finalize_claim_lock, `same_postfix` on DoRequest variants)

## Deployment Blocker
**Cloudflare API error 10013** — both `npx wrangler deploy` (non-versioned) and `npx wrangler versions upload` fail:
- Non-versioned deploy: 10013 "unknown error" (infrastructure issue)
- Versions upload: 10211 "migrations must be applied via non-versioned deployment" (correct — DO migrations can't go through versions API)

**Fix**: Retry `npx wrangler deploy` when Cloudflare API is healthy. The 10013 error is transient.

## Shim.mjs Fix
The wrangler build command in `wrangler.toml` now appends:
```js
export { EventDurableObject } from "./event_checkin_worker_bg.js";
```
This is required because `wasm-bindgen` generates the DO class in `_bg.js`, but wrangler needs it exported from the entry point (`shim.mjs`).

## What's Next
### Phase 2: Check-in + Claim Writes Through DO
- Add `CheckIn`, `ClaimAttendee`, `UndoCheckIn` variants to `DoRequest`
- Add attendee mutation methods to `EventDurableObject`
- Route check-in and claim handlers through DO

### Phase 3: Registration + Deposit Writes Through DO
- Add `UpsertAttendee`, `VerifyDeposit`, `MarkRefund` variants
- Route registration and deposit handlers through DO

### Phase 4: Remove D1 Write Paths
- All writes go through DO, D1 is read-only
- Add retry logic for DO→D1 sync

## How to Dev/Test
```bash
# Build check
cd worker && cargo check --target wasm32-unknown-unknown

# Deploy (retry if 10013)
cd worker && npx wrangler deploy

# Verify production
curl -s https://bethere.solana-thailand.workers.dev/api/health | python3 -m json.tool

# Check DO binding in health
# Should show EVENT_DO in bindings when deployed
```
