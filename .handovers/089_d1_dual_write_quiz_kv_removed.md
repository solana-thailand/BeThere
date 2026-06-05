# 089: D1 Dual-Write Complete + QUIZ KV Removed

## What Happened
Continued from `088_d1_quiz_escrow_deploy_prod.md`. Implemented Option A (D1 gaps) + Option C (QUIZ KV removal), committed, deployed to production, verified.

## Commit
- `9a63d34` — `feat: D1 dual-write for update_event, remove QUIZ KV binding`

## Changes Made

### Option A: D1 Dual-Write Gaps Fixed
| File | Change |
|------|--------|
| `worker/src/event_store/write.rs` | `update_event` signature: `(kv: &KvStore)` → `(kv: Option<&KvStore>, d1: Option<&D1Database>)`. D1-first reads via `get_event_config_with_fallback`. Full dual-write for escrow index. KV ops guarded by `if let Some(kv_ref)`. |
| `worker/src/handlers/deposit/escrow/status.rs` | `confirm_escrow_init_handler`: KV optional (no hard `ok_or_else`). Passes `d1` to `update_event`. Audit log guarded by `if let Some(kv_ref)`. |
| `worker/src/handlers/events.rs` | Inline `PUT /api/events/{id}` handler: Added `save_escrow_index(d1, kv, ...)` + KV index entry update (both missing). |

### Option C: QUIZ KV Binding Removed
| File | Change |
|------|--------|
| `worker/wrangler.toml` | QUIZ KV binding commented out with rollback instructions. |
| `worker/deploy.sh` | QUIZ KV binding removed from PUT API metadata JSON. |
| `worker/migrations/0003_events_full_schema.sql` | Documented non-idempotent nature with manual fix instructions. |

## D1 Dual-Write Coverage (All Paths)
| Code Path | D1 | KV |
|-----------|-----|-----|
| `create_event` | ✅ | ✅ |
| `update_event` (write.rs) | ✅ **was KV-only** | ✅ |
| `update_event` (events.rs inline) | ✅ **was missing** | ✅ **was missing** |
| `confirm_escrow_init_handler` | ✅ **was KV-only** | ✅ |
| `archive_event` | ✅ | ✅ |
| `hard_delete_event` | ✅ | ✅ |

## Production Verification
| Endpoint | Result |
|----------|--------|
| `/api/health` | ✅ `d1.connected: true`, 3 events, 3 attendees |
| `/api/public/events` | ✅ `{success: true, events: []}` (no public events) |
| `/api/quiz` | ✅ `{configured: false}` — D1-only, QUIZ KV removed |

## KV Bindings Status
| Binding | Status |
|---------|--------|
| `EVENTS` | Active in wrangler.toml (code ignores it; kept for Cloudflare API compat) |
| `QUIZ` | **Removed** from wrangler.toml + deploy.sh |
| `DB` (D1) | Active — primary data store |
| `ASSETS_BUCKET` (R2) | Active — blob storage |

## Deploy Notes
- Used `bash deploy.sh` — Cloudflare `/versions` API returned 10013, fallback PUT API succeeded
- QUIZ KV binding removed from production — `quiz_kv` is `None` at runtime
- `migrate_quiz` endpoint returns error ("quiz KV namespace not configured") — expected and correct

## What's Next: Option B — #050 Durable Objects
- Route writes through DO + SQLite for ACID guarantees on check-in, claim, registration
- See `.issues/050_durable_objects_acid_migration.md`
- Production has 0 real write traffic — ideal time to migrate
- D1 stays as read store for analytics/dashboards

## How to Dev/Test
```bash
# Local dev
cd worker && npx wrangler dev
curl -s http://localhost:8787/api/health | python3 -m json.tool
curl -s http://localhost:8787/api/public/events | python3 -m json.tool
curl -s http://localhost:8787/api/quiz | python3 -m json.tool

# Deploy
cd worker && bash deploy.sh

# Prod verification
curl -s https://bethere.solana-thailand.workers.dev/api/health | python3 -m json.tool
curl -s https://bethere.solana-thailand.workers.dev/api/public/events | python3 -m json.tool
curl -s https://bethere.solana-thailand.workers.dev/api/quiz | python3 -m json.tool
```
