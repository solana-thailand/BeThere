# 030 — Master Contacts Sheet Activation

## Problem

The Master Contacts Sheet code is fully implemented (#027, #029) but never activated because `CONTACTS_SHEET_ID` is not configured. Without it:

- Attendee contact upsert silently skips on every registration
- `POST /api/contacts/sync` returns a validation error
- `GET /api/contacts` and `GET /api/contacts/stats` return empty
- Events tab in the contacts sheet never gets populated
- Organizers have no cross-event deduplicated contact list

## Status: READY TO ACTIVATE — code complete, config missing

## Implementation (Code)

All code is done. No code changes needed.

| Module | Status | File |
|--------|--------|------|
| Contacts sheet CRUD | ✅ | `worker/src/sheets/contacts.rs` — upsert (dedupe by email), list, ContactUpsert struct |
| Events tab CRUD | ✅ | `worker/src/sheets/events_tab.rs` — upsert event row, list, delete |
| Contacts API handlers | ✅ | `worker/src/handlers/contacts.rs` — list, stats, sync, events-tab |
| Registration hook | ✅ | `worker/src/handlers/register.rs:317` — upsert_contact_after_registration() |
| Event create/update hook | ✅ | `worker/src/handlers/events.rs` — sync_event_to_tab() on create + update |
| Per-Org resolution | ✅ | `worker/src/org_store.rs` — resolve_contacts_sheet() with global fallback |
| Org CRUD API | ✅ | `worker/src/handlers/orgs.rs` — orgs can have their own contacts_sheet_id |
| Config loading | ✅ | `worker/src/state.rs:71` — reads CONTACTS_SHEET_ID from env, defaults empty |

## Activation Checklist

### Step 1 — Create the Google Sheet

- [ ] Create a new Google Sheet (e.g. "BeThere Master Contacts")
- [ ] Rename the default tab to `Contacts`
- [ ] Add header row in `Contacts` tab:

| Column | Header | Example | Purpose |
|--------|--------|---------|---------|
| A | `email` | `john@gmail.com` | Primary key (lowercased) |
| B | `name` | `John Doe` | Display name |
| C | `first_registered` | `2025-03-15` | First event date |
| D | `last_registered` | `2026-05-22` | Most recent event date |
| E | `events_joined` | `evt_abc,evt_xyz` | Comma-separated event IDs |
| F | `event_count` | `2` | Number of events |
| G | `contact_channel` | `Telegram` | Preferred contact channel |
| H | `contact_handle` | `@john` | Handle |
| I | `send_email_status` | `pending` | Last bulk email status |
| J | `last_emailed_at` | `2026-05-22` | Last email timestamp |

### Step 2 — Create Events tab

- [ ] Add a second tab named `Events`
- [ ] Add header row in `Events` tab:

| Column | Header | Example | Purpose |
|--------|--------|---------|---------|
| A | `event_id` | `solana-bangkok-2025` | Unique event ID |
| B | `name` | `Solana x AI Builders` | Display name |
| C | `slug` | `solana-bangkok-2025` | URL slug |
| D | `status` | `active` | Draft/Active/Completed/Archived |
| E | `event_format` | `in_person` | InPerson/Online/Hybrid |
| F | `event_start_ms` | `1777170600000` | Start timestamp (epoch ms) |
| G | `event_end_ms` | `1777183200000` | End timestamp (epoch ms) |
| H | `deposit_enabled` | `true` | Whether deposit is required |
| I | `deposit_amount_usdc` | `15000000` | USDC amount (6 decimals) |
| J | `deposit_amount_thb` | `500` | THB amount |
| K | `escrow_status` | `initialized` | None/Initialized/Deactivated/Closed |
| L | `location` | `Bangkok` | Venue |
| M | `tagline` | `Deep Dive...` | Subtitle |
| N | `organizer_emails` | `alice@x.com,bob@y.com` | Comma-separated |
| O | `total_attendees` | `42` | Attendee count |
| P | `created_at` | `2025-03-15T10:00:00Z` | ISO 8601 |
| Q | `organization_id` | `solana-thailand` | Org ID (empty = global) |

### Step 3 — Share with Service Account

- [ ] Click **Share** in the Google Sheet
- [ ] Add the service account email (same one used for event attendee sheets)
- [ ] Grant **Editor** access

### Step 4 — Configure Worker

- [ ] Set the contacts sheet ID as a Wrangler secret:
  ```bash
  cd worker
  npx wrangler secret put CONTACTS_SHEET_ID
  # Paste the sheet ID from the URL, e.g. 1AbCdEfGhIjKlMnOpQrStUvWxYz
  ```
- [ ] (Optional) Add tab name overrides to `wrangler.toml` `[vars]` if using non-default names:
  ```toml
  CONTACTS_SHEET_NAME = "Contacts"
  EVENTS_SHEET_NAME = "Events"
  ```

### Step 5 — Deploy & Verify

- [ ] Deploy: `cd worker && npx wrangler deploy`
- [ ] Verify health: `GET /api/health`
- [ ] Backfill existing contacts: `POST /api/contacts/sync` — scans all event sheets and populates both Contacts + Events tabs
- [ ] Verify contacts: `GET /api/contacts` — should return deduplicated attendee list
- [ ] Verify events tab: `GET /api/contacts/events-tab` — should return all events
- [ ] Verify stats: `GET /api/contacts/stats` — should show total contacts, repeat attendees, per-event breakdown
- [ ] Test auto-upsert: register a new attendee → check Contacts tab has a new row

### Step 6 — Per-Org (Optional, for multi-org)

- [ ] Create org(s): `POST /api/orgs` with `contacts_sheet_id` pointing to org-specific sheet
- [ ] Assign events to org: `PUT /api/events/{id}` with `organization_id`
- [ ] Future registrations for org events will upsert to org's contacts sheet

## API Endpoints (Already Implemented)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| `GET` | `/api/contacts` | SuperAdmin | List all deduplicated contacts |
| `GET` | `/api/contacts/stats` | SuperAdmin | Contact counts, repeat attendees, per-event stats |
| `POST` | `/api/contacts/sync` | SuperAdmin | Backfill from all event sheets |
| `GET` | `/api/contacts/events-tab` | SuperAdmin | List events from Events tab |
| `GET` | `/api/orgs` | SuperAdmin | List organizations |
| `POST` | `/api/orgs` | SuperAdmin | Create organization |
| `GET` | `/api/orgs/{id}` | SuperAdmin | Get organization |
| `PUT` | `/api/orgs/{id}` | SuperAdmin | Update organization |
| `DELETE` | `/api/orgs/{id}` | SuperAdmin | Delete organization |

## Refs

- `.issues/027_master_contacts_thb_guard.md` — original issue (THB guard + contacts sheet design)
- `.issues/029_per_org_contacts_sheet.md` — per-org contacts sheet (multi-org support)
- `worker/src/sheets/contacts.rs` — contacts sheet operations
- `worker/src/sheets/events_tab.rs` — events tab operations
- `worker/src/handlers/contacts.rs` — contacts API handlers
- `worker/src/org_store.rs` — org KV store + resolve_contacts_sheet()
- `worker/src/state.rs:65-77` — config loading (CONTACTS_SHEET_ID, CONTACTS_SHEET_NAME, EVENTS_SHEET_NAME)
