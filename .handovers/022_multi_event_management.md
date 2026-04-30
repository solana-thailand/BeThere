# Handover 022: Multi-Event Management (Issue 004)

## What Happened

Implemented **Issue 004: Multi-Event / Organizer Management** — all 5 phases complete. The platform now supports multiple events with per-event configuration, per-event staff/organizer access control, and a KV migration path from the legacy single-event model.

**Flow change:**

```
Before:  Single hardcoded event → global staff → global config
After:   Event registry in KV → per-event config → per-event staff/organizer access guard
```

The session also discovered and fixed a critical **KV deserialization bug** in the `worker` crate where `.json::<T>()` fails for complex types at the WASM boundary.

## Changes Made

### Phase 1: Backend Data Model & KV

| Component | Details |
|-----------|---------|
| `domain/src/models/event.rs` | New file — `EventConfig`, `EventMeta`, `EventIndex`, `EventStatus` types |
| `worker/src/event_store.rs` | New file — KV event storage CRUD, seed, migration, key helpers |
| `worker/src/handlers/events.rs` | New file — event CRUD handlers + seed + migrate |
| `worker/src/state.rs` | Added `events_kv` binding, `super_admin_emails` parsing |
| `domain/src/config/types.rs` | Added `super_admin_emails` to `AppConfig` |
| `worker/wrangler.toml` | Added EVENTS KV namespace binding |

### Phase 2: Event-Scoped Attendee & Quiz APIs

- All existing handlers (attendees, checkin, QR, quiz, claim) now event-scoped via `resolve_event_or_fallback()`
- Backward-compatible: legacy routes work by resolving to first active event
- Quiz handlers use event-scoped KV keys: `"event:{id}:quiz:questions"`, `"event:{id}:quiz:progress:{tok}"`

### Phase 3: Frontend Event Management UI

| Component | Details |
|-----------|---------|
| `frontend-leptos/src/pages/events_page.rs` | New file — full event management page (list, create, edit) |
| `frontend-leptos/src/pages/quiz_editor.rs` | Event-scoped quiz editor |
| `frontend-leptos/src/api.rs` | Event CRUD API functions, `api_put_json` helper |
| `frontend-leptos/src/pages/admin.rs` | Event selector integration |
| `frontend-leptos/src/pages/scanner.rs` | Event context for scanner |

### Phase 4: Auth & Organizer Management

| Role | Access Level |
|------|-------------|
| `super_admin` | Full access — create/delete events, manage all events |
| `organizer` (assigned to event) | Edit event config, manage quiz, view dashboard |
| `staff` (assigned to event) | Scanner only — check in attendees |

New `check_event_access()` guard applied to all 4 staff-scoped handlers:
- `POST /api/checkin/{id}` (checkin.rs)
- `GET /api/attendees` (attendee.rs)
- `GET /api/attendee/{id}` (attendee.rs)
- `POST /api/generate-qrs` (qr.rs)

New `UserRole` enum and `resolve_user_role()` hierarchy in `worker/src/auth.rs`.

### Phase 5: Migration & Polish

| Component | Details |
|-----------|---------|
| `POST /api/events/migrate` | SuperAdmin only, copies quiz data from QUIZ namespace → EVENTS namespace |
| `POST /api/events/seed` | Seeds event from env-var config fallback |
| Idempotent migration | Re-runs return `{"migrated": false, "message": "quiz data already migrated..."}` |

### KV Deserialization Bug Fix

**Problem**: `kv.get("key").json::<T>().await` throws `invalid type: JsValue(Object({})), expected unit` for all complex types. The `serde_wasm_bindgen` deserialization path fails at the WASM boundary.

**Fix**: Changed all KV reads from `.json::<T>()` to `.text()` + manual `serde_json::from_str()` in:
- `worker/src/event_store.rs` — `get_event_index()`, `get_event_config()`, `migrate_quiz_to_event()`
- `worker/src/quiz.rs` — `get_quiz_config()`, `get_quiz_progress()`

**Important**: After the deserialization error, local KV state in `.wrangler/state/v3/local/KV/` became corrupted. Clearing it (`rm -rf`) was necessary before the fixed `.text()` approach could work.

### Files Modified (29 files changed, +5667 -174 lines)

| File | Change |
|------|--------|
| `domain/src/models/event.rs` | **NEW** — `EventConfig`, `EventMeta`, `EventIndex`, `EventStatus` |
| `domain/src/models/mod.rs` | Added `event` module |
| `domain/src/config/types.rs` | Added `super_admin_emails` to `AppConfig` |
| `worker/src/event_store.rs` | **NEW** — KV event storage, CRUD, seed, migration |
| `worker/src/handlers/events.rs` | **NEW** — event CRUD + seed + migrate handlers |
| `worker/src/handlers/mod.rs` | Added `/events`, `/events/seed`, `/events/migrate` routes |
| `worker/src/handlers/checkin.rs` | Added `check_event_access()` guard |
| `worker/src/handlers/attendee.rs` | Added `check_event_access()` guard (2 handlers) |
| `worker/src/handlers/qr.rs` | Added `check_event_access()` guard |
| `worker/src/handlers/claim.rs` | Event-scoped via `resolve_event_or_fallback` |
| `worker/src/handlers/quiz.rs` | Event-scoped, fixed KV reads to `.text()` |
| `worker/src/handlers/auth.rs` | `auth_me` returns role from hierarchy |
| `worker/src/auth.rs` | Added `check_event_access()`, `UserRole` enum, `resolve_user_role()` |
| `worker/src/quiz.rs` | Fixed KV reads to `.text()` + manual parse |
| `worker/src/state.rs` | Added `events_kv`, `super_admin_emails` parsing |
| `worker/src/lib.rs` | Module registration |
| `worker/wrangler.toml` | Added EVENTS KV namespace binding |
| `frontend-leptos/src/pages/events_page.rs` | **NEW** — event management UI |
| `frontend-leptos/src/pages/quiz_editor.rs` | Event-scoped quiz editor |
| `frontend-leptos/src/api.rs` | Event CRUD API functions |
| `frontend-leptos/src/components.rs` | `can_manage_events()`, updated `is_admin_role()` |
| `frontend-leptos/src/pages/admin.rs` | Event selector integration |
| `frontend-leptos/src/pages/scanner.rs` | Event context for scanner |

## Build Verification

| Check | Result |
|-------|--------|
| `cargo check` | ✅ 0 errors, 0 warnings |
| `cargo clippy` | ✅ 0 errors, 0 warnings |
| `cargo test` | ✅ 34/34 pass (14 domain + 20 worker) |
| `wrangler dev` local server | ✅ Health check OK |
| Event listing via API | ✅ Returns seeded event |
| Quiz migration (idempotent) | ✅ Works correctly |
| Auth flow (crafted JWT → auth/me) | ✅ Returns `super_admin` role |
| Per-event access guard | ✅ Rejected unauthorized staff with 403 |

## Architecture Decisions

### KV `.text()` over `.json::<T>()`
The `worker` crate's `.json::<T>()` uses `serde_wasm_bindgen` which fails for complex types at the WASM boundary. Using `.text()` + `serde_json::from_str()` is more reliable and avoids this class of bug entirely.

### Per-Event Access Guard Pattern
`check_event_access()` is called *after* `require_auth` (global staff check) and *after* event resolution. This gives a 3-tier access model: super_admin → organizer of this event → staff of this event. Any other authenticated user gets 403.

### Event Seeding from Config
`seed_from_config()` reads existing env vars (`EVENT_NAME`, `ORGANIZER_EMAILS`, etc.) to create the initial event. This is idempotent — re-running updates the existing event if the ID matches.

## JWT Testing Note

When crafting JWTs for local testing with Node.js `crypto.createHmac`:
- Use `secret` directly as UTF-8 bytes (`secret.as_bytes()` in Rust)
- Do NOT base64-decode the secret — Rust uses raw bytes, not base64

## Pre-Deployment Checklist

- [ ] Deploy to dev — `npx wrangler deploy --env dev`
- [ ] Seed production event — `POST /api/events/seed` with super_admin JWT
- [ ] Migrate production quiz — `POST /api/events/migrate` against production
- [ ] End-to-end test — Create event → add quiz → check in → claim NFT
- [ ] Mobile QA — Event selector, event list, scanner on mobile viewport
- [ ] Add backup super_admin email for account recovery
- [ ] Clean up env vars — Remove hardcoded event config from `wrangler.toml`
- [ ] Rebase/merge to main after production verification

## Post-Deployment — Cleanup

- [ ] Update NFT minting to use event-specific metadata (`solana.rs`)
- [ ] Clean up `wrangler.toml` vars (keep as defaults only)
- [ ] Remove QUIZ KV namespace binding after full migration verified

## Future — Post Issue 004

- **Issue 005**: SOL + USDC Refund on Claim (on-chain refund flow)
- Event analytics dashboard
- Event templates (clone config for recurring events)
- Per-event theming (colors, logos)

## Refs

- Issue: `.issues/004_multi_event_management.md`
- Commit: `b902e5a` on `develop`
- Related: `.handovers/021_quiz_gated_claim_flow.md` (Issue 002)
