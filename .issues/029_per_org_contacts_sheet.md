# 029: Per-Org Contacts Sheet (Approach A — Multi-Org)

## Status: Done

## What

Implement Approach A for multi-org support: each organization gets its own Google Sheet for contacts + events tab. Events are assigned to an org via `organization_id`. When attendees register or contacts are synced, the system resolves the contacts sheet from the event's organization, falling back to the global config when no org is set.

## Implementation

### New Files
| File | Purpose |
|------|---------|
| `domain/src/models/org.rs` | `OrganizationConfig`, `OrgIndex`, `OrgMeta`, `CreateOrgRequest`, `UpdateOrgRequest`, `ResolvedContactsSheet` |
| `worker/src/org_store.rs` | KV CRUD for organizations + `resolve_contacts_sheet()` |
| `worker/src/handlers/orgs.rs` | API handlers: list, create, get, update, delete organizations |

### Modified Files
| File | Change |
|------|--------|
| `domain/src/models/mod.rs` | Added `pub mod org` |
| `domain/src/models/event.rs` | Added `organization_id` to `EventConfig`, `EventMeta`, `CreateEventRequest`, `UpdateEventRequest`, `from_global_config` |
| `worker/src/lib.rs` | Added `mod org_store` |
| `worker/src/handlers/mod.rs` | Added `pub mod orgs` + org CRUD routes |
| `worker/src/handlers/register.rs` | `upsert_contact_after_registration` now resolves contacts sheet per-org |
| `worker/src/handlers/contacts.rs` | `sync_contacts_handler` resolves contacts sheet per-org per event |
| `worker/src/handlers/events.rs` | `sync_event_to_tab` and `hard_delete_event` resolve per-org |
| `worker/src/event_store.rs` | `create_event`, `update_event`, `seed_from_config` include `organization_id` |
| `worker/src/handlers/deposit/escrow.rs` | Added `organization_id: None` to `UpdateEventRequest` construction |
| `worker/src/sheets/events_tab.rs` | Added column Q (`organization_id`) to Events tab schema |

### New API Endpoints
| Endpoint | Method | Purpose | Auth |
|----------|--------|---------|------|
| `/api/orgs` | GET | List all organizations | SuperAdmin |
| `/api/orgs` | POST | Create organization | SuperAdmin |
| `/api/orgs/{id}` | GET | Get organization details | SuperAdmin |
| `/api/orgs/{id}` | PUT | Update organization | SuperAdmin |
| `/api/orgs/{id}` | DELETE | Delete organization (no active events) | SuperAdmin |

### Events Tab Schema Update
Added column Q (`organization_id`) to the Events tab:
```
A–P: (unchanged from issue 028)
Q: organization_id | solana-thailand | Org ID (empty = global)
```

### KV Storage Layout
```
orgs           → OrgIndex (list of OrgMeta summaries)
org:{org_id}   → OrganizationConfig (full org config)
```

### Resolution Logic
```
event.organization_id → empty? → use global CONTACTS_SHEET_ID
                     → non-empty? → load org config → use org's contacts_sheet_id
                                    if org's sheet_id empty? → fall back to global
```

## Backward Compatibility
- Events without `organization_id` (empty string) → use global `CONTACTS_SHEET_ID` (unchanged behavior)
- Existing events continue to work without migration
- Global `CONTACTS_SHEET_ID` env var still serves as the default fallback
- No breaking API changes — `organization_id` is optional on `CreateEventRequest`/`UpdateEventRequest`

## Setup Required
- [ ] Run `POST /api/contacts/sync` to backfill organization_id into Events tab rows
- [ ] Create organization(s) via `POST /api/orgs`
- [ ] Update events to set `organization_id` via `PUT /api/events/{id}`
- [ ] Add "organization_id" header column (Q1) in existing Events tabs
