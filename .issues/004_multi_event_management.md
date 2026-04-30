# Issue 004: Multi-Event / Organizer Management [PLAN]

## Problem

The platform currently assumes a **single global event**. All configuration is hardcoded in `wrangler.toml` environment variables, all attendees come from one Google Sheet, quiz config is stored under a single KV key (`"questions"`), and NFT metadata is hardcoded. This means:

- Each event deployment requires a full `wrangler` redeploy with new env vars
- Organizers cannot self-serve create/manage events
- Staff/roles are global — no per-event organizer assignment
- One quiz for all events (no per-event quiz)
- NFT name/description is hardcoded to "Road to Mainnet"

## Solution

Introduce an **event registry** stored in KV, with per-event configuration that overrides global defaults. Events become first-class entities with their own Google Sheet, quiz config, staff list, and NFT metadata.

### High-Level Architecture

```
KV Namespace: EVENTS (new)
  "events"                    → EventIndex { events: Vec<EventMeta> }
  "event:{id}"                → EventConfig (full per-event config)
  "event:{id}:quiz"           → QuizConfig (per-event quiz)
  "event:{id}:progress:{tok}" → QuizProgress (per-event quiz progress)

KV Namespace: QUIZ (existing)
  → migrate to "event:{id}:quiz" keys in EVENTS namespace
  → keep QUIZ namespace for backward compat during migration

Google Sheets (per-event)
  Each event stores its own sheet_id + sheet_name in EventConfig
  Staff tab column C gets event_id (or use global staff with per-event role)
```

### Data Model

#### EventMeta (list item in index)

```rust
pub struct EventMeta {
    pub id: String,              // e.g. "solana-bangkok-2025"
    pub name: String,            // display name
    pub slug: String,            // URL-friendly slug
    pub status: EventStatus,     // Draft | Active | Completed | Archived
    pub event_start_ms: i64,
    pub event_end_ms: i64,
    pub sheet_id: String,        // Google Sheets ID
    pub created_at: String,      // ISO 8601
    pub organizer_emails: Vec<String>,
}

pub enum EventStatus {
    Draft,
    Active,
    Completed,
    Archived,
}
```

#### EventConfig (full config, stored per event)

```rust
pub struct EventConfig {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub tagline: String,
    pub link: String,
    pub status: EventStatus,
    pub event_start_ms: i64,
    pub event_end_ms: i64,

    // Google Sheets
    pub sheet_id: String,
    pub sheet_name: String,        // attendee tab name
    pub staff_sheet_name: String,  // staff tab name (optional override)

    // Quiz
    pub quiz_enabled: bool,
    pub quiz_passing_score_percent: u8,
    pub quiz_max_attempts: u8,
    pub quiz_time_limit_seconds: Option<u16>,

    // NFT
    pub nft_collection_mint: String,
    pub nft_metadata_uri: String,
    pub nft_image_url: String,
    pub nft_name_template: String,   // e.g. "BeThere - {event_name}"
    pub nft_symbol: String,
    pub nft_description_template: String,

    // Auth
    pub organizer_emails: Vec<String>,
    pub staff_emails: Vec<String>,

    // Claim
    pub claim_base_url: String,

    // Timestamps
    pub created_at: String,
    pub updated_at: String,
}
```

### API Design

#### New Endpoints (Admin Only)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/events` | List all events (summary) |
| POST | `/api/events` | Create new event |
| GET | `/api/events/{id}` | Get event config |
| PUT | `/api/events/{id}` | Update event config |
| DELETE | `/api/events/{id}` | Archive event (soft delete) |

#### Modified Endpoints (Event-Scoped)

All existing endpoints gain an optional `event_id` parameter. For backward compatibility, if no `event_id` is specified, the **first active event** is used.

| Existing Route | Event-Aware Route | Notes |
|----------------|-------------------|-------|
| `GET /api/attendees` | `GET /api/events/{id}/attendees` | Legacy: uses active event |
| `GET /api/attendee/{id}` | `GET /api/events/{id}/attendees/{id}` | Legacy: searches active event |
| `POST /api/checkin/{id}` | `POST /api/events/{id}/checkin/{id}` | Legacy: uses active event |
| `POST /api/generate-qrs` | `POST /api/events/{id}/generate-qrs` | Legacy: uses active event |
| `GET /api/admin/quiz` | `GET /api/events/{id}/quiz/admin` | Legacy: uses active event |
| `POST /api/admin/quiz` | `POST /api/events/{id}/quiz/admin` | Legacy: uses active event |
| `GET /api/quiz` | `GET /api/events/{id}/quiz` | Public: uses active event |
| `POST /api/quiz/{token}/submit` | No change needed | Claim tokens are globally unique |
| `GET /api/claim/{token}` | No change needed | Claim tokens are globally unique |

**Migration strategy**: Keep existing routes working. Add new `/api/events/{id}/...` routes alongside. Frontend migrates incrementally.

### Frontend Changes

#### Admin Dashboard — Event Selector

- Top of sidebar: Event selector dropdown (or event list)
- Switching events reloads dashboard data for that event
- "Create Event" button opens event creation form

#### Event Management Page

- List all events with status badges
- Create/Edit event form (name, dates, sheet ID, NFT config, etc.)
- Event settings: quiz toggle, staff management
- "Go to Dashboard" link per event

#### Staff Scanner

- Event context set by URL or selector
- `/staff` → redirects to `/events/{id}/staff` if multiple events

#### Claim Page

- No change needed — claim tokens are globally unique UUIDs
- Event config fetched server-side from claim token → event lookup

### Auth Changes

#### Current Flow (Global)

```
Google OAuth → check STAFF_EMAILS env var → check staff sheet tab → assign role
```

#### New Flow (Per-Event)

```
Google OAuth → check global admin emails → check event organizer_emails → check event staff_emails
Roles become: super_admin | organizer | staff
```

- `super_admin`: Global admin, can create events, manage all events
- `organizer`: Per-event manager, can edit event config, manage quiz, view dashboard
- `staff`: Per-event scanner, can check in attendees

### KV Key Migration

#### Current KV Keys (QUIZ namespace)

```
"questions"                → QuizConfig
"progress:{claim_token}"   → QuizProgress
```

#### New KV Keys (EVENTS namespace)

```
"events"                          → EventIndex
"event:{id}"                      → EventConfig
"event:{id}:quiz:questions"       → QuizConfig
"event:{id}:quiz:progress:{tok}"  → QuizProgress
```

#### Migration Steps

1. Deploy new code with EVENTS KV binding added
2. Create first event from current `wrangler.toml` env vars via migration script
3. Copy existing `"questions"` key to `"event:{id}:quiz:questions"`
4. Rename/copy progress keys (or keep legacy QUIZ namespace for progress)
5. Frontend switches to event-scoped API calls
6. Remove QUIZ namespace binding after full migration

---

## Implementation Phases

### Phase 1: Backend Data Model & KV (Foundation) ✅ COMPLETE

**Goal**: Add event registry to KV, create event CRUD API.

- [x] Add `EVENTS` KV namespace binding to `wrangler.toml`
- [x] Add event-related types to `domain/src/models/`:
  - `EventMeta`, `EventStatus`, `EventConfig`, `EventIndex`
  - `CreateEventRequest`, `UpdateEventRequest`, `EventListResponse`
- [x] Add KV helper functions for event CRUD:
  - `get_event_index()`, `save_event_index()`
  - `get_event_config()`, `save_event_config()`
  - `list_events()`, `create_event()`, `update_event()`, `archive_event()`
- [x] Add event API handlers in `worker/src/handlers/events.rs`
- [x] Wire up routes: `GET/POST /api/events`, `GET/PUT/DELETE /api/events/{id}`
- [x] Add `require_admin` middleware (stricter than `require_auth`)
- [x] Implement `resolve_event_id()` helper: extract from path OR default to active event
- [x] **Test**: curl event CRUD endpoints

**Files changed**: `domain/src/models/event.rs`, `domain/src/models/mod.rs`, `worker/src/handlers/events.rs`, `worker/src/handlers/mod.rs`, `worker/src/state.rs`, `worker/wrangler.toml`

### Phase 2: Event-Scoped Attendee & Quiz APIs ✅ COMPLETE

**Goal**: Make existing handlers event-aware.

- [x] Modify `AppState` to hold `EVENTS` KV binding
- [x] Add `resolve_event()` helper that:
  1. Extracts `event_id` from path param
  2. Falls back to first active event
  3. Returns `EventConfig` with sheet_id, quiz config, etc.
- [x] Refactor `sheets.rs` to accept `sheet_id` + `sheet_name` parameters
  (instead of reading from global config)
- [x] Update attendee handlers to use `resolve_event()`:
  - `list_attendees()` → uses event's sheet_id
  - `get_attendee()` → uses event's sheet_id
  - `check_in()` → uses event's sheet_id
  - `generate_qrs()` → uses event's sheet_id
- [x] Update quiz handlers to use event-scoped KV keys:
  - `get_admin_quiz()` → reads `"event:{id}:quiz:questions"`
  - `put_quiz()` → writes `"event:{id}:quiz:questions"`
  - `get_quiz()` (public) → reads from event context
  - `submit_quiz()` → writes `"event:{id}:quiz:progress:{token}"`
  - `get_quiz_status()` → reads from event-scoped progress
- [x] Update claim handlers to look up event from attendee data:
  - `get_claim()` → returns event-specific `EventConfig` display data
  - `post_claim()` → uses event-specific NFT metadata
- [x] Add backward-compatible routes:
  - Legacy routes work by resolving to active event
  - New routes: `/api/events/{id}/attendees`, etc.
- [x] **Test**: verify legacy routes still work, test event-scoped routes

**Files changed**: `worker/src/state.rs`, `worker/src/sheets.rs`, `worker/src/handlers/attendee.rs`, `worker/src/handlers/checkin.rs`, `worker/src/handlers/qr.rs`, `worker/src/handlers/quiz.rs`, `worker/src/handlers/claim.rs`, `worker/src/solana.rs`

### Phase 3: Frontend Event Management UI ✅ COMPLETE

**Goal**: Add event selector and event management page to admin dashboard.

- [x] Add API functions to `api.rs`:
  - `get_events()`, `create_event()`, `update_event()`, `get_event()`
- [x] Add event selector to admin sidebar (dropdown or list)
  - Store selected event in `LocalStorage` for persistence
  - Pass `event_id` to all API calls
- [x] Create `EventManagement` page:
  - Event list with status, date, attendee count
  - Create event form (name, slug, dates, sheet_id, etc.)
  - Edit event form
  - Event settings (quiz toggle, NFT config)
- [x] Update `AdminSection` enum to include `Events` variant
- [x] Update all admin API calls to include `event_id` parameter
- [x] Update quiz editor to use event-scoped API
- [x] Update scanner page to accept event context
- [x] **Test**: Create event → see it in list → switch to it → manage attendees/quiz

**Files changed**: `frontend-leptos/src/api.rs`, `frontend-leptos/src/pages/admin.rs`, `frontend-leptos/src/pages/events.rs` (new), `frontend-leptos/src/pages/quiz_editor.rs`, `frontend-leptos/src/main.rs`

### Phase 4: Auth & Organizer Management ✅ COMPLETE

**Goal**: Per-event organizer/staff assignment.

- [x] Add `super_admin_emails` to config (global admins who can create events)
- [x] Modify `require_auth` to accept event context:
  - Check if user is super_admin → full access
  - Check if user is organizer for this event → event management
  - Check if user is staff for this event → scanner only
- [x] Add staff management UI in event settings:
  - Add/remove organizer emails
  - Add/remove staff emails
- [x] Update role resolution in `auth.rs`:
  - `is_admin_role()` checks global admin + per-event organizer
  - `is_staff_role()` checks global staff + per-event staff
- [x] **Test**: Test role-based access for different users across events

**Files changed**: `worker/src/auth.rs`, `domain/src/config/types.rs`, `frontend-leptos/src/pages/events.rs`

### Phase 5: Migration & Polish ✅ COMPLETE

**Goal**: Migrate existing data, clean up, deploy.

- [x] Create migration script/endpoint:
  - Read current `wrangler.toml` env vars
  - Create EventConfig from them
  - Copy KV `"questions"` → `"event:{id}:quiz:questions"`
  - Mark migration complete
- [x] Add `is_migrated()` check to gracefully handle pre/post migration
- [x] Update `wrangler.toml` to add EVENTS KV binding
- [x] Remove hardcoded event config from `wrangler.toml` vars (keep as defaults)
- [x] Update `EventConfig` served to claim page (dynamic per event)
- [ ] Update NFT minting to use event-specific metadata (post-deployment)
- [ ] Update `solana.rs` to use dynamic name/description (post-deployment)
- [ ] End-to-end test: create event → add quiz → check in → claim NFT (pre-deployment)
- [ ] Mobile QA: event selector, event list (pre-deployment)
- [ ] Deploy to dev → test → deploy to production (pending)

**Files changed**: `worker/src/solana.rs`, `worker/src/state.rs`, `worker/wrangler.toml`, `domain/src/config/types.rs`

---

## Key Design Decisions

### 1. KV for Event Registry (not D1)

**Rationale**: The codebase already uses KV. Adding D1 would require a new dependency and migration. KV is simpler for this use case — event configs are read-heavy, write-rarely. The event index is a single JSON blob that's easy to manage.

**Trade-off**: No complex queries. But we only need "list events" and "get event by ID", which KV handles fine.

### 2. Per-Event Google Sheets

**Rationale**: Each event already has its own attendee list in Google Sheets. Multi-event just means each EventConfig points to a different sheet_id. No schema change needed in Sheets.

**Trade-off**: Organizers must create their own Google Sheet and share it with the service account. This is documented in setup instructions.

### 3. Backward-Compatible Routes

**Rationale**: Existing routes (`/api/attendees`, `/api/checkin/{id}`, etc.) continue to work by resolving to the "active" event. This allows incremental migration — frontend can switch to event-scoped routes one page at a time.

### 4. Claim Tokens Remain Global

**Rationale**: Claim tokens are UUID v7 (globally unique). The claim page doesn't need to know which event it belongs to — the server resolves event context from the attendee data in Google Sheets.

### 5. Event Slug for URLs

**Rationale**: Using slugs (`solana-bangkok-2025`) instead of UUIDs in URLs is more user-friendly and SEO-friendly for public claim pages.

---

## Estimated Effort

| Phase | Description | Days |
|-------|-------------|------|
| 1 | Backend Data Model & KV | 1-2 |
| 2 | Event-Scoped APIs | 2-3 |
| 3 | Frontend Event Management UI | 2-3 |
| 4 | Auth & Organizer Management | 1-2 |
| 5 | Migration & Polish | 1-2 |
| **Total** | | **7-12 days** |

---

## Dependencies

- Feature B (Admin Quiz UI) — ✅ Complete
- New KV namespace binding (EVENTS) — requires `wrangler.toml` update
- Google Sheets API access — already configured per service account

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| KV key migration breaks existing quiz progress | Users lose quiz attempts | Keep QUIZ namespace active during migration; dual-read |
| Google Sheets rate limits with multiple events | Slower check-ins | Already ~500ms-2s; acceptable for now |
| Per-event auth complexity | Security regression | Comprehensive role tests; audit before deploy |
| Frontend event selector UX confusion | Organizers pick wrong event | Show event name prominently; confirm dialog |
| Large event index in KV | Slow list operations | KV reads are fast (<10ms); index stays small (<100 events) |

---

## Future Considerations (Post-004)

- Event analytics dashboard (per-event stats over time)
- Event templates (clone event config for recurring events)
- Event-specific theming (colors, logos, backgrounds)
- Attendee cross-event tracking (attendee attends multiple events)
- Event marketplace (public event listing page)
- QR code for event check-in (attendee scans event QR, not organizer scanning attendee)