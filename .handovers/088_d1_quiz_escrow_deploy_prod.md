# 088: D1 Quiz/Escrow Index Tables + Production Deploy

## What Happened
Continued from commit `7e1c7f1` (P3 — EVENTS KV optional). Completed P4, P5, fixed public_event.rs, deployed to production.

## Commits
- `75fab75` — P4: D1 quiz/escrow index tables, remove EVENTS KV binding
- `44f808f` — Fix: keep EVENTS KV binding in wrangler.toml for Cloudflare API compat

## Changes Made
- **New files**: `db/quiz.rs`, `db/escrow_index.rs`, migration `0004_quiz_escrow_index_tables.sql`
- **Updated**: `public_event.rs` — D1-first listing and slug resolution (was KV-only, caused `/api/public/events` to fail)
- **Updated**: ~30 call sites across quiz, escrow index, handlers, claim, event_store — all D1-first with KV fallback
- **wrangler.toml**: EVENTS KV binding kept (not commented out) — Cloudflare `/versions` API returns 500 when bindings differ from production

## Production State
- D1 connected, 3 events, 3 attendees, 0 quiz/escrow index rows
- All endpoints verified: `/api/health`, `/api/public/events`, `/api/quiz`
- Deployed via `deploy.sh` (Cloudflare `/versions` API has 500 error — `deploy.sh` fallback to PUT API works)
- Migration `0004` applied to remote D1; `0003` manually marked as applied (columns already existed)

## Remaining Gaps
1. `update_event` passes `None` for D1 to escrow index functions — dual-write gap
2. Walk-in attendee counting in `public_event.rs` still requires KV for capacity display
3. `0003_events_full_schema.sql` is not idempotent (uses `ALTER TABLE ADD COLUMN` without checks) — only matters if applied to fresh DB

## Suggested Next Thread

### Option A: Fix Remaining D1 Gaps (Small, ~1 hour)
Thread these D1 references through the remaining code paths:
1. Thread D1 through `update_event` → `save_escrow_index`/`delete_escrow_index` for full dual-write
2. Add D1-based walk-in counting or accept 0 capacity without KV
3. Make `0003` migration idempotent (wrap ALTER TABLE in column-existence checks)

### Option B: #050 Durable Objects for ACID (Large, next major milestone)
Route writes through DO + SQLite for check-in, claim, registration.
- See `.issues/050_durable_objects_acid_migration.md`
- Production has 0 real write traffic — ideal time to switch
- D1 stays as read store for analytics/dashboards

### Option C: QUIZ KV Binding Removal (Small, ~30 min)
Comment out `QUIZ` KV binding in wrangler.toml, verify quiz endpoints work D1-only.
After validation, update `deploy.sh` metadata to remove QUIZ binding.

## How to Dev/Test
```bash
# Local dev (wrangler dev must be running)
curl -s http://localhost:8787/api/health
curl -s http://localhost:8787/api/public/events
curl -s http://localhost:8787/api/quiz

# Deploy
cd worker && bash deploy.sh

# Check prod
curl -s https://bethere.solana-thailand.workers.dev/api/health | python3 -m json.tool

# D1 queries
cd worker && npx wrangler d1 execute bethere-db --remote --command="SELECT id, status, visibility, event_end_ms FROM events;"
```
