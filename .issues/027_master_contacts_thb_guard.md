# 027 — Master Contacts Sheet + THB Deposit Amount Guard

## Problem

1. **THB deposit amount is mutable after deposits exist**: SEC-002 only guards `deposit_amount_usdc` when escrow is initialized. An organizer can change `deposit_amount_thb` after THB deposits have been uploaded, causing confusion for new depositors who see a different amount than existing ones.

2. **No cross-event contact list**: Organizers have no way to email all previous attendees across events. Each event has its own Google Sheet — there's no deduplicated master list of emails.

## Solution

### A. THB Deposit Amount Guard (SEC-002-THB)

Extend SEC-002 in `update_event` to also guard `deposit_amount_thb` when escrow is initialized (same condition as USDC). This prevents the organizer from changing either deposit amount after on-chain escrow creation.

### B. Master Contacts Sheet

A single "Master Contacts" Google Sheet that:

- **Upserts on every registration** — when an attendee registers (self or walk-in), upsert their contact info to the master sheet
- **Deduplicates by email** — if the same email registers for multiple events, append the new event_id to `events_joined` instead of creating a duplicate row
- **Supports email campaigns** — `send_email_status` column tracks email delivery status
- **Backfill endpoint** — `POST /api/contacts/sync` scans all event sheets and populates the master sheet

#### Master Contacts Sheet Schema

| Column | Field | Example | Purpose |
|--------|-------|---------|---------|
| A | `email` | `john@gmail.com` | Primary key (lowercased) |
| B | `name` | `John Doe` | Display name |
| C | `first_registered` | `2025-03-15` | First event date |
| D | `last_registered` | `2026-05-22` | Most recent event date |
| E | `events_joined` | `evt_abc,evt_xyz` | All event IDs (comma-separated) |
| F | `event_count` | `2` | Number of events |
| G | `contact_channel` | `Telegram` | Preferred contact |
| H | `contact_handle` | `@john` | Handle |
| I | `send_email_status` | `pending` | Last bulk email status |
| J | `last_emailed_at` | `2026-05-22` | Last email timestamp |

#### Implementation

1. **New module**: `worker/src/handlers/contacts.rs` — API handlers for sync/list
2. **New sheet function**: `worker/src/sheets/contacts.rs` — upsert/contact sheet operations
3. **Modify registration**: After appending to event sheet, also upsert to master contacts
4. **New config field**: `contacts_sheet_id` in `EventConfig` (or global config)
5. **New API routes**: `GET /api/contacts`, `POST /api/contacts/sync`

## Implementation Phases

### Phase 1 — THB Guard (quick fix)
- [x] Add `deposit_amount_thb` to SEC-002 check in `update_event`
- [ ] Test: verify organizer cannot change THB amount after escrow init

### Phase 2 — Contacts Sheet Core
- [ ] Add `contacts_sheet_id` to app config / environment
- [ ] Create `worker/src/sheets/contacts.rs` with upsert function
- [ ] Hook into `register_attendee` to upsert after event sheet append
- [ ] Hook into walk-in registration (if applicable)

### Phase 3 — Contacts API
- [ ] `POST /api/contacts/sync` — backfill from all event sheets
- [ ] `GET /api/contacts` — list deduplicated contacts
- [ ] `GET /api/contacts/stats` — count by event, total unique emails

### Phase 4 — Email Integration (future)
- [ ] Integrate Resend API for bulk email sending
- [ ] `POST /api/contacts/email` — send email to all or filtered contacts
- [ ] Update `send_email_status` column after sending
