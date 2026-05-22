# 028: Events Tab + Multi-Org Design Discussion

## Status: Done (Events Tab), Design Pending (Multi-Org)

## What

1. **Events Tab** — Stores event metadata in a dedicated "Events" tab in the same Google Sheet as Contacts, giving organizers a single-sheet view of events + contacts.
2. **Multi-Org Design** — Discussion about how to handle multiple organizations each seeing their own data.

## Events Tab Implementation

### Sheet Schema (columns A–P)
```
A: event_id            | solana-bangkok-2025
B: name                | Solana x AI Builders
C: slug                | solana-bangkok-2025
D: status              | active
E: event_format        | in_person
F: event_start_ms      | 1777170600000
G: event_end_ms        | 1777183200000
H: deposit_enabled     | true
I: deposit_amount_usdc | 15000000
J: deposit_amount_thb  | 500
K: escrow_status       | initialized
L: location            | Bangkok
M: tagline             | Deep Dive...
N: organizer_emails    | alice@x.com,bob@y.com
O: total_attendees     | 42
P: created_at          | 2025-03-15T10:00:00Z
```

### Sync Triggers
- **Event create** (`POST /api/events`) → appends new row
- **Event update** (`PUT /api/events/{id}`) → updates existing row
- **Contacts sync** (`POST /api/contacts/sync`) → syncs all events with attendee counts
- **Event hard-delete** (`DELETE /api/events/{id}?delete`) → clears row

### Files Created
- `worker/src/sheets/events_tab.rs` — Sheet operations: upsert, delete, list

### Files Modified
- `worker/src/sheets/mod.rs` — Added `pub mod events_tab`
- `worker/src/handlers/mod.rs` — Added `GET /api/contacts/events` route
- `worker/src/handlers/contacts.rs` — Added `list_events_tab_handler`, Events tab sync in `sync_contacts_handler`
- `worker/src/handlers/events.rs` — Added `sync_event_to_tab` helper, called on create/update/hard-delete
- `domain/src/config/types.rs` — Added `events_sheet_name` to `SheetsConfig`
- `worker/src/state.rs` — Read `EVENTS_SHEET_NAME` env var
- `worker/src/auth.rs` — Added `events_sheet_name` to test initializer
- `frontend-leptos/src/pages/admin_escrow.rs` — Fixed `_step2_done` warning
- `worker/src/handlers/contacts.rs` — Fixed clippy warnings (redundant closures, sort_by_key)

### New API Endpoint
| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/contacts/events` | GET | List events from Events tab (staff-protected) |

### New Environment Variable
| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `EVENTS_SHEET_NAME` | Optional | `"Events"` | Tab name for events registry in contacts sheet |

## Multi-Org Design (Pending User Decision)

Three approaches discussed:
- **A. Per-Org Contacts Sheet** (recommended) — Add `organization_id`, each org has its own contacts sheet
- **B. Organization Entity with Roles** — Full org model with Owner/Admin/Member roles
- **C. Full RBAC/IAM** — Enterprise ACL system

## Setup Required
- [ ] Create "Events" tab in the BeThere Contacts Google Sheet with header row
- [ ] Set `EVENTS_SHEET_NAME` env var (or use default "Events")
- [ ] Run `POST /api/contacts/sync` to backfill events
