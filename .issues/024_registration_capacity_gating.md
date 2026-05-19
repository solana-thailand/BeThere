# 024 — Registration Capacity, Track Gating & Claim Timing

## Summary

Implement capacity limits, track-based registration gating, and claim timing enforcement for events. This adds:
- In-person capacity (configurable by organizer)
- Online capacity (default unlimited, configurable — prevents NFT exhaustion)
- Online registration gating (auto on full, manual toggle, or always open)
- Deposit deadline with auto-switch to online track
- Claim gating: online attendees can only claim after event ends
- Sheet row deletion fix (delete row instead of clearing cells)

## Status: IN PROGRESS — Phase 2 Done

## Design Decisions

### Capacity Model

| Track | Default | Configurable | Behavior |
|-------|---------|-------------|----------|
| In-person | Required field | Organizer sets limit | Registration counts toward capacity |
| Online | Unlimited (None) | Organizer can set limit | Prevents NFT exhaustion |

**Capacity counting**: Both states count toward in-person capacity:
- **Pending deposit** (registered but not yet deposited) — holds the spot
- **Deposited** (deposit verified) — confirmed spot

### Deposit Deadline + Auto-Switch

When an in-person registrant doesn't deposit within the deadline:
- Their `participation_type` is **auto-switched to "Online"**
- Their in-person spot is released
- They receive a notification (if configured)
- Default deadline: 24 hours (organizer configurable)

### Online Registration Gating (`OnlineOpenMode`)

| Mode | Behavior |
|------|----------|
| `Always` | Both tracks open from registration start |
| `AutoOnFull` | Online opens automatically when in-person capacity is reached |
| `Manual` | Organizer flips toggle manually via staff UI |

### Claim Timing

| Track | When can they claim? |
|-------|---------------------|
| In-Person | After **check-in** at event (proves physical presence) |
| Online | After **event ends** (`now > event_end_ms`) |

This prevents online attendees from completing everything before the event occurs.

### Walk-in Handling (Phase 4)

- Walk-ins are always in-person by definition (physical presence required)
- They count against in-person capacity
- If capacity is reached, staff gets a **warning** but can still override (they see the person physically)
- Online-only events: walk-in blocked

### Registration UX

- In-person option shows remaining spots: "In-Person — 3 spots left"
- In-person option **disappears** when full (not disabled — removed entirely)
- Online option auto-selected when in-person is full
- Registration form adapts based on available tracks

## New Domain Types

### `EventConfig` New Fields

```rust
// Capacity
pub in_person_capacity: Option<u32>,    // None = unlimited, Some(150) = 150 spots
pub online_capacity: Option<u32>,       // None = unlimited (default)

// Online registration gating
pub online_open_mode: OnlineOpenMode,   // default: Always
pub online_registration_open: bool,     // manual toggle (for Manual mode)

// Deposit deadline
pub deposit_deadline_hours: Option<u32>, // None = no deadline, Some(24) = 24h
```

### `OnlineOpenMode` Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnlineOpenMode {
    #[default]
    Always,      // Both tracks open from registration start
    AutoOnFull,  // Online opens when in-person capacity is reached
    Manual,      // Organizer flips toggle manually
}
```

## Registration Endpoint Logic

```
POST /api/public/register
  1. Resolve event → config
  2. Determine available tracks based on event_format:
     - InPerson: only in-person track
     - Online: only online track
     - Hybrid: both tracks (gated by capacity + online_open_mode)
  3. Count in-person attendees (pending deposit + deposited)
  4. If participation_type == "In-Person":
     - if in_person_capacity is Some && count >= capacity → reject "In-person spots are full"
  5. If participation_type == "Online":
     - if online_capacity is Some && online_count >= capacity → reject "Online spots are full"
     - check online_open_mode:
       - Manual: check online_registration_open toggle
       - AutoOnFull: check if in-person is full
       - Always: always allowed
  6. Register → append to sheet → return next_step
```

## `build_next_step` Changes

```
In-Person + no deposit  → deposit page (/deposit/{id}?event_id={eid})
In-Person + deposited   → ticket page (/ticket/{id}?event_id={eid})
Online                  → waiting page (event page with "Claims open after event" message)
```

## Implementation Phases

### Phase 1 — Sheet Fix + Claim Gating (Critical) ✅ Done
- [x] Fix `delete_sheet_row` to delete rows instead of clearing cells
- [x] Gate online claims behind `event_end_ms`
- [x] Change online `next_step` to "waiting" instead of direct claim URL
- [x] Invalidate cache after row deletion
- **Files**: `worker/src/sheets/write.rs`, `worker/src/handlers/register.rs`, `worker/src/claim.rs`

### Phase 2 — Capacity Fields + Registration Enforcement ✅ Done
- [x] Add `in_person_capacity`, `online_capacity` to `EventConfig`
- [x] Add `OnlineOpenMode` enum to domain
- [x] Add capacity fields to `EventMeta`, `CreateEventRequest`, `UpdateEventRequest`
- [x] Registration endpoint enforces capacity limits (sheet + walk-in KV counts)
- [x] Count in-person attendees (sheet + walk-in KV)
- [x] Count online attendees (sheet)
- [x] Public event endpoint returns capacity info (counts + remaining + availability flags)
- [x] Frontend shows remaining spots on public event page (capacity indicator card)
- [x] Hybrid dropdown shows remaining spots per track, hides full tracks
- [x] Frontend API types updated (`EventMeta`, `EventDetail`, `CreateEventBody`, `UpdateEventBody`)
- [x] Event form state includes capacity fields (create/edit wired through)
- **Files**: `domain/src/models/event.rs`, `worker/src/handlers/register.rs`, `worker/src/handlers/public_event.rs`, `worker/src/event_store.rs`, `frontend-leptos/src/pages/public_event.rs`, `frontend-leptos/src/api/event.rs`, `frontend-leptos/src/pages/events_page.rs`

### Phase 3 — Organizer Controls + Deposit Deadline
- [ ] Manual toggle for online registration in staff/admin UI
- [ ] Capacity input fields on event create/edit form
- [ ] `OnlineOpenMode` selector on event form
- [ ] Deposit deadline field + countdown on deposit page
- [ ] Cron/edge trigger for deposit deadline enforcement (auto-switch to online)
- **Files**: `frontend-leptos/src/pages/events_page.rs`, `worker/src/handlers/attendee.rs`

### Phase 4 — Walk-in Capacity Handling
- [ ] Walk-in counts against in-person capacity
- [ ] Warning dialog when walk-in exceeds capacity (staff can override)
- [ ] Walk-in blocked for online-only events
- **Files**: `worker/src/handlers/walkin.rs`, `frontend-leptos/src/pages/scanner.rs`

## Related Issues
- `.issues/014_walkin_attendee_flow.md` — Walk-in flow
- `.issues/015_event_format_model.md` — EventFormat model (completed)
- `.issues/019_walkin_sync_export.md` — Walk-in sync (sheet row bug)

## Related Docs
- `docs/business_flows_event_page.md` — Section 16 (Event Format Model), Section 17 (Attendee Journey)
- `docs/ux_roadmap.md` — UX improvements

## Refs
- `domain/src/models/event.rs` — EventConfig, EventFormat
- `worker/src/handlers/register.rs` — Registration endpoint, build_next_step
- `worker/src/sheets/write.rs` — delete_sheet_row (clear vs delete)
- `frontend-leptos/src/pages/public_event.rs` — Registration form
