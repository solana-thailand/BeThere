# 014 — Walk-in Attendee Flow (Hybrid KV-based)

## Summary
Walk-in attendees (who show up without pre-registering) are registered by staff via the scanner UI → backend creates KV record + auto-syncs to Google Sheet → same deposit/NFT/refund loop as pre-registered attendees.

## Status: IN PROGRESS

### Phase 1 — Backend Walk-in Registration API ✅ Done
- [x] Add `WalkinAttendee` struct in `domain/src/models/attendee.rs`
- [x] Add `POST /api/walkin/register` endpoint (staff-only, requires auth)
- [x] Validate input: name (required), email (required), phone (optional)
- [x] Create KV attendee record with `walkin:{event_id}:{email}` key (90-day TTL)
- [x] Generate claim token (UUID v7) + reverse mapping `claim_walkin:{token}`
- [x] Return claim token + claim URL to staff UI

### Phase 2 — Deposit/Refund/NFT Flow Compatibility ✅ Done
- [x] Walk-in claim lookup: `lookup_walkin_by_claim_token()` checks KV first
- [x] Walk-in claim execution: `execute_walkin_claim()` mints NFT + updates KV (no sheet)
- [x] Deposit flow: wallet-based, works independently of attendee records
- [x] Refund flow: wallet-based, works independently of attendee records

### Phase 3 — Scanner UI ✅ Done
- [x] `WalkinRegisterRequest` / `WalkinRegisterResponse` in frontend API
- [x] "Register Walk-in Attendee" button in scanner Idle state
- [x] Walk-in registration form: name, email, phone
- [x] Walk-in success: QR code of claim URL for attendee to scan
- [x] "Scan Another" button returns to Idle

### Phase 4 — Google Sheet Sync & CSV Export ✅ Done
- [x] `POST /api/walkin/sync` — batch sync walk-ins to Google Sheet (idempotent)
- [x] `GET /api/walkin/export` — CSV export of walk-in attendees
- [x] `GET /api/walkin/list` — list walk-in attendees from KV
- [x] Admin UI: "Export CSV" and "Sync to Sheet" buttons
- [x] Idempotency via `walkin_synced:{event_id}:{email}` KV markers

### Phase 5 — Auto-sync on Register ⚠️ In Progress
- [x] Auto-sync walk-in to Google Sheet immediately after KV write (best-effort)
- [x] Mark as synced in KV on success (prevents duplicate by `/walkin/sync`)
- [x] Graceful fallback — logs warning on failure, `/walkin/sync` retries later
- [x] Diagnostic logging: `sheet_id`, `sheet_name`, `event_id` on auto-sync
- [ ] **BUG: auto-sync writes to wrong event's Google Sheet** — needs investigation
  - Check `wrangler tail` logs for `"walk-in auto-sync: resolved sheet"` line
  - Compare `sheet_id` with expected Google Sheet
  - Possible cause: event config in KV has wrong `sheet_id`

### TODO — Remaining
- [ ] Fix auto-sync wrong sheet bug (Phase 5)
- [ ] Unified attendee list: merge walk-ins into `GET /api/attendees` with `source` field
- [ ] Walk-in count shown separately in event dashboard
- [ ] Walk-in cap per event (configurable, prevent escrow abuse) — see `.issues/024_registration_capacity_gating.md` Phase 4
- [ ] Rate limiting on walk-in register (max N per minute per staff)

## Architecture

```
Register Flow:
  Staff UI → POST /api/walkin/register
    → 1. Validate input
    → 2. Check duplicate in KV
    → 3. Generate claim token (UUID v7)
    → 4. Write walk-in record to KV (90-day TTL)
    → 5. Write reverse mapping (claim token → event + email)
    → 6. Auto-sync to Google Sheet (best-effort, non-blocking)
    → 7. Return claim URL + attendee data

Sync Flow (fallback):
  Admin UI → POST /api/walkin/sync
    → List all walk-in KV records for event
    → Skip already-synced (idempotency check)
    → Append each to Google Sheet via append_walkin_row()
    → Mark synced in KV
```

## Key Files

| File | Purpose |
|------|---------|
| `worker/src/handlers/walkin.rs` | Register, list, export, sync handlers |
| `worker/src/sheets.rs` | `append_walkin_row()` — maps walk-in fields to sheet columns |
| `frontend-leptos/src/pages/scanner.rs` | Walk-in form, QR display |
| `frontend-leptos/src/api.rs` | Frontend API types for walk-in |
| `domain/src/models/attendee.rs` | `WalkinAttendee` struct |

## Refs
- `.issues/019_walkin_sync_export.md` — Phase 4 sync/export issue
- `docs/escrow_protocol.md` — Protocol design with deposit/NFT/refund loop
- Issue 010 — Deposit/refund escrow architecture
- Issue 013 — Escrow rug pull prevention (security context)
