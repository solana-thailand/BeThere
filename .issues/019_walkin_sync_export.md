# 019 — Walk-in Phase 4: CSV Export + Google Sheet Sync

## Summary

Walk-in attendees are stored in KV. Phase 4 adds batch sync to Google Sheets + CSV download + auto-sync on register. Auto-sync has a known bug writing to the wrong sheet.

## Status: IN PROGRESS

### ✅ Done
- [x] `GET /api/walkin/list` — list walk-in attendees from KV (cursor pagination)
- [x] `GET /api/walkin/export` — CSV export with proper headers + UTF-8
- [x] `POST /api/walkin/sync` — batch sync to Google Sheet (idempotent)
- [x] Idempotency via `walkin_synced:{event_id}:{email}` KV markers
- [x] Admin UI: "Export Walk-in CSV" + "Sync Walk-ins to Sheet" buttons
- [x] Audit logging: `WalkinSynced`, `WalkinExported` actions
- [x] Auto-sync on register (best-effort, non-blocking fallback to batch sync)
- [x] Diagnostic logging for auto-sync: `event_id`, `sheet_id`, `sheet_name`

### ⚠️ Bug — Auto-sync writes to wrong event's Google Sheet
- [ ] Reproduce: `cd worker && npx wrangler tail` → register walk-in → check logs
- [ ] Find `"walk-in auto-sync: resolved sheet"` log line → note `sheet_id` + `sheet_name`
- [ ] Compare with expected Google Sheet
- [ ] Possible causes:
  - Event config in KV has wrong `sheet_id`
  - Multiple events share same `sheet_id` / `sheet_name`
  - Column mapping cache from different sheet
- [ ] Fix root cause
- [ ] Verify fix: register walk-in → check correct Google Sheet

### TODO — Remaining
- [ ] Unified attendee list: merge walk-ins into `GET /api/attendees` with `source` field
- [ ] Unified CSV export: `GET /api/attendees/export` includes both sheet + walk-in

## Endpoints

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| `POST` | `/api/walkin/register` | Staff | Register walk-in + auto-sync to sheet |
| `GET` | `/api/walkin/list` | Staff | List walk-in attendees from KV |
| `GET` | `/api/walkin/export` | Staff | Download walk-in CSV |
| `POST` | `/api/walkin/sync` | Staff | Batch sync all walk-ins to Google Sheet |

## Key Files

| File | Change |
|------|--------|
| `worker/src/handlers/walkin.rs` | Register + auto-sync, list, export, sync handlers |
| `worker/src/sheets.rs` | `append_walkin_row()` — maps walk-in fields to sheet columns |
| `frontend-leptos/src/api.rs` | Walkin API types |
| `frontend-leptos/src/pages/admin.rs` | Export/Sync buttons |

## How to Debug Auto-sync Wrong Sheet

```bash
# Terminal 1: Watch live logs
cd worker && npx wrangler tail --format json

# Terminal 2 (or browser): Register a walk-in attendee
# Check logs for:
#   "walk-in registered"          → event_id, email
#   "walk-in auto-sync: resolved sheet"  → event_id, sheet_id, sheet_name

# Check event config in KV
npx wrangler kv key get --namespace-id=c8a6a87f9ed34ce0a3c8e48b84039214 "event:<event_id>"
# Verify sheet_id field matches expected Google Sheet
```

## Refs
- `.issues/014_walkin_attendee_flow.md` — parent walk-in issue
- `.issues/024_registration_capacity_gating.md` — sheet row deletion fix (Phase 1)
- `worker/src/handlers/walkin.rs` — walk-in handlers
- `worker/src/sheets.rs` — Google Sheets integration
