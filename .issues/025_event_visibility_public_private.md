# #025 — Event Visibility: Public / Private

## Status: ✅ IMPLEMENTED — Ready for deploy + E2E

## Problem
All Active events are currently visible to everyone on the landing page and `/e/{slug}`.
Organizers need the ability to create private/invite-only events that are:
- Hidden from the public landing page
- Only accessible via direct link with authentication + access check
- Visible to assigned organizers/staff in their admin dashboard

## Solution
Add `EventVisibility` enum (`Public` | `Private`) to control event discoverability,
separate from `EventStatus` lifecycle (Draft → Active → Completed → Archived).

### Visibility Matrix
| Status | Visibility | Landing | `/e/{slug}` (anon) | `/e/{slug}` (auth+access) | Admin list |
|--------|-----------|---------|---------------------|---------------------------|------------|
| Draft | any | hidden | 404 | 404 | super_admin + creator |
| Active | Public | shown | shown | shown | scoped by role |
| Active | Private | hidden | 404 | shown | scoped by role |
| Completed | Public | hidden (past) | shown | shown | scoped by role |
| Completed | Private | hidden | 404 | shown | scoped by role |

## Implementation
1. Domain: `EventVisibility` enum + field on `EventConfig`, `EventMeta`, requests/responses
2. Worker: Filter `list_public_events` by visibility, gate `get_public_event` with auth for private
3. Frontend: Mirror enum, add toggle in create/edit form, serde contract tests

## Files Changed
- `domain/src/models/event.rs` — EventVisibility enum, field on structs
- `worker/src/event_store.rs` — create/update logic
- `worker/src/handlers/public_event.rs` — visibility filtering + auth gate
- `frontend-leptos/src/api/event.rs` — mirror enum + fields
- `frontend-leptos/src/pages/events_page.rs` — form toggle
- `worker/tests/serde_contract.rs` — round-trip test
- `frontend-leptos/tests/serde_contract.rs` — round-trip test
