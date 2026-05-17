# 019 — Walk-in Phase 4: CSV Export + Google Sheet Sync

## Summary

Walk-in attendees are stored in KV but invisible to organizers post-event. They're not in Google Sheets, not in the attendee list API, and have no CSV export. Phase 4 adds batch sync to Google Sheets + CSV download for walk-in data.

## Problem

### Walk-ins are invisible to organizers

- `GET /api/attendees` only reads from Google Sheets — walk-in attendees don't appear
- Walk-ins are KV-only (`walkin:{event_id}:{email}` with 90-day TTL)
- After an event, organizers can't see walk-in attendance in their Sheets analytics
- CSV exports miss all walk-in data

### Current state of walk-in data

| Field | Walk-in (`WalkinAttendee`) | Pre-registered (`Attendee`) |
|-------|---------------------------|---------------------------|
| Source | KV only | Google Sheet |
| Primary key | email (per event) | api_id (UUID v7) |
| Fields | 8: name, email, phone, claim_token, timestamps, wallet | 28 columns |
| Check-in | Automatic (checked_in_at = registration time) | Written to sheet columns |
| NFT claim | KV only | Sheet + KV |

## Proposed Solution

### A. Unified attendee list endpoint

Merge walk-ins into the existing `GET /api/attendees` response so organizers see everyone in one view.

- Scan KV prefix `walkin:{event_id}:*` using cursor pagination (same pattern as `cleanup.rs` `delete_keys_by_prefix`)
- Add a `source` field: `"sheet"` or `"walkin"` to each attendee
- Walk-ins get synthetic values for missing fields (e.g., `participation_type: "In-Person"`, `approval_status: "CheckedIn"`)

### B. CSV export endpoint

```
GET /api/walkin/export?event_id=xxx
```

- Staff-only endpoint
- Scans all `walkin:{event_id}:*` KV keys
- Returns `text/csv` with headers: `Name, Email, Phone, Check-in Time, Registered By, Wallet Address, NFT Claimed`
- Also add a unified export: `GET /api/attendees/export?event_id=xxx` that includes both sheet + walk-in attendees

### C. Google Sheet sync endpoint

```
POST /api/walkin/sync?event_id=xxx
```

- Staff-only endpoint
- Scans all walk-in KV records for the event
- Maps `WalkinAttendee` fields to `ColumnMapping` columns with sensible defaults:
  - `ParticipationType` = "In-Person"
  - `ApprovalStatus` = "CheckedIn" (they were checked in on arrival)
  - `TicketName` = "Walk-in"
  - `DepositMethod` = empty (walk-ins don't deposit)
- Uses existing `append_attendee_row()` for each record, OR batch append
- Idempotency: check KV key `walkin_synced:{event_id}:{email}` before appending
- Returns sync summary: `{ synced: N, skipped: M, errors: [...] }`

### D. Admin UI button

Add "Export CSV" and "Sync to Sheet" buttons to the admin dashboard for each event.

## Files to Create/Modify

| File | Change |
|------|--------|
| `worker/src/handlers/walkin.rs` | Add `list_walkin_attendees()`, `export_walkin_csv()`, `sync_walkin_to_sheet()` |
| `worker/src/handlers/mod.rs` | Register `/walkin/export` and `/walkin/sync` routes on staff router |
| `worker/src/handlers/admin.rs` | Add unified attendee export endpoint |
| `frontend-leptos/src/pages/admin.rs` | Add "Export CSV" and "Sync to Sheet" buttons per event |

## Acceptance Criteria

- [ ] `GET /api/walkin/export?event_id=xxx` returns CSV with all walk-in attendees
- [ ] `POST /api/walkin/sync?event_id=xxx` appends walk-in rows to Google Sheet
- [ ] Sync is idempotent — running twice doesn't create duplicate rows
- [ ] `GET /api/attendees?event_id=xxx` includes walk-in attendees (with `source` field)
- [ ] Admin dashboard has "Export CSV" button for walk-in data
- [ ] Admin dashboard has "Sync to Sheet" button with sync result feedback
- [ ] CSV has proper headers and UTF-8 encoding

## Dependencies

- Issue 014 Phases 1-3 (walk-in registration) — ✅ already deployed
- Google Sheets service account access — already configured

## Refs

- `.issues/014_walkin_attendee_flow.md` — original walk-in issue
- `worker/src/handlers/walkin.rs` — current walk-in handler
- `worker/src/sheets.rs` — Google Sheets integration patterns
- `worker/src/cleanup.rs` L209-240 — KV list+cursor pagination pattern
