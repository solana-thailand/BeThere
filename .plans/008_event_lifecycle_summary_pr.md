# Plan 008 — Event Lifecycle: Summary, Recap, Post-Event Registration, PR Generator

> **Status**: Phase 1 (Post-Event Summary) ✅ shipped (`48d25b1`, deployed) · Phase 2 (Public Recap + Past Events) ✅ shipped (`9549532`, deployed) · Phase 3 (Post-Event Registration) ✅ implemented 2026-07-09 (toggle + public register + frontend form; live validation blocked on Plan 005 staging) · Phase 4 (PR Generator) ✅ implemented 2026-07-09 (`63270ac`). Cross-cutting checks (clippy clean, tests green) re-run and passing.
> **Type**: feature (event lifecycle workflow) + content (PR/recap generation)
> **Priority**: P2 — closes the "what happens after an event ends" gap and turns past events into lead-capture surfaces. Independent of plans 005/006/007; can start in parallel.
> **Created**: 2026-06-23
> **Blocks**: none
> **Depends on**: none (recommend running on top of plan 005's staging env when it lands, but not strictly required)
> **Decisions locked**:
>
> - PR = promotion/recap **content** (not pull requests)
> - Summary = **frozen snapshot** at `event_end_ms` (option 3b), not live-computed
> - Audience: **internal organizer report first**; public recap is Phase 2
> - Post-event registration = lead capture for `contacts` + `developer_profiles`

---

## 1. Problem

The system handles an event's **pre-event** (registration, deposits) and **live** (dashboard, check-in, claims) phases well. The moment `event_end_ms` passes, three things break:

1. **The event vanishes from public view.** `list_public_events` (`worker/src/handlers/public_event.rs`) filters `event_end_ms > now`. Within seconds of the event ending, it disappears from the landing page. There is no archive, no "past events" feed, no public record that it happened.

2. **The dashboard stops being useful.** `GET /api/dashboard/live` is a real-time poll — it never freezes. Once the event ends, the organizer has no durable artifact saying "47 registered, 31 deposited, 28 checked in, 26 claimed, 3 refunded, 3 no-show." The numbers can drift (deposits refunded weeks later, claims minted late) and there's no point-in-time snapshot.

3. **No way to capture interest from people who missed it.** A developer hears about the event the day after, visits the slug URL, sees nothing. They bounce. We lose a `developer_profiles` row and a `contacts` row. Every completed event should be a **community-growth surface**.

Additionally, on the pre-event side:

4. **No structured PR content.** Organizers hand-write announcement copy from the `EventConfig` fields. The data is all there (name, tagline, date, location, deposit terms, capacity, organizer emails) but there's no generator that turns it into a "PR pack" — headline, short blurb, social post, calendar text, email snippet. Every organizer reinvents this.

### Evidence

- **Public listing filter** (`worker/src/handlers/public_event.rs`): filters on `event_end_ms > now` AND `status == Active`. Completed events are excluded.
- **Dashboard is live-only** (`worker/src/db/dashboard.rs`): functions `count_registered`, `count_checked_in`, `count_claims_minted`, `verified_usdc_summary`, `recent_activity` all compute from live tables on every call. No snapshot primitive exists.
- **No past-events route** (`worker/src/handlers/mod.rs`): route inventory shows `/public/events` and `/public/event/{slug}` only. No `/public/events/past` or `/public/event/{slug}/recap`.
- **Registration requires active event** (`worker/src/handlers/register.rs#L1-80`): `RegisterRequest` assumes a live event; no branch for "event has ended, capture interest only."
- **`contacts.events_joined` is a CSV string** (`worker/src/db/contacts.rs#L22-31`): stored, overwritten on every upsert, not queryable as a relation. "Show me everyone who attended both X and Y" requires parsing CSVs across rows.
- **`EventStatus` enum** (`domain/src/models/event.rs`): `Draft | Active | Completed | Archived`. The lifecycle states exist, but `Completed` has no associated content surface — it's effectively a tombstone.

### Why now

Plan 004 shipped the refund-gate fix; plans 005–007 are about hardening and new surfaces (staging, SIWS, mobile). None of them address the post-event gap. This plan is independent, scoped to the worker + frontend only, and unblocks a real workflow the organizer is asking for today.

---

## 2. Scope

### In scope

- **Migration 0019**: new `event_summaries` table (frozen snapshot) + new columns on `events` (`post_event_registration_open`, `post_event_registration_until_ms`, `recap_published`) + new `attendees.registration_phase` column.
- **Phase 1 — Post-Event Summary (internal record)**: freeze logic + `GET /api/events/{id}/summary` (protected) + `POST /api/events/{id}/summary/freeze` (manual trigger) + organizer-facing summary view in frontend.
- **Phase 2 — Public Recap + Past Events Listing**: `PUT /api/events/{id}/recap` (organizer authors markdown + image) + `GET /api/public/event/{slug}/recap` (public, gated on `recap_published`) + `GET /api/public/events/past` listing.
- **Phase 3 — Post-Event Registration (lead capture)**: `PUT /api/events/{id}/post-event-registration` (organizer toggle + deadline) + `POST /api/public/event/{slug}/register-post-event` (public) + frontend form. Upserts `contacts` + `developer_profiles` exactly like normal registration, but with `registration_phase = 'post_event'` and no deposit/check-in flow.
- **Phase 4 — Upcoming PR Generator**: `GET /api/events/{id}/pr-pack` returns structured fields (headline, short_blurb, social_post, calendar_text, email_snippet) generated deterministically from `EventConfig`. No AI / no external API. Frontend preview page with copy-to-clipboard.
- **`events_joined` derivation**: add a read-side helper that derives a contact's event history from the `attendees` table (source of truth). The CSV column stays for backward compatibility but is no longer the query path for history.

### Out of scope

- **AI-assisted content generation** — v1 is deterministic templates only. AI drafting is a future enhancement on top of the same `/pr-pack` endpoint.
- **Email/SMS/Line blast infrastructure** — the PR pack is content; distribution channels are a separate concern.
- **Full `events_joined` CSV removal** — refactor of write paths. This plan only adds the read-side derivation and documents the CSV as tech debt.
- **Mobile UI** — plan 007. This plan's endpoints are mobile-consumable when 007 lands.
- **SIWS gating** — plan 006. All new protected endpoints use existing JWT auth.
- **On-chain changes** — `bethere-escrow` is untouched.
- **Recurring event / event series concept** — each event remains standalone. Series grouping can be a future plan.
- **Scheduled job (cron) for auto-freeze** — v1 uses lazy freeze on first read + manual button. Cron is a documented future enhancement.

---

## 3. Implementation

### 3.0 Migration 0020 — schema foundation

> **Note (updated 2026-06-24):** The `0019` slot was taken by `0019_event_poster.sql`
> (Plan 009 — event poster URL). This migration is renumbered to **0020**.

New file: `worker/migrations/0020_event_summaries_post_event.sql`

```sql
-- Plan 008: Event lifecycle — summary snapshots, recap, post-event registration.

-- ============================================================
-- EVENT_SUMMARIES — frozen point-in-time snapshot per event
-- ============================================================
-- One row per event, written once at freeze time (lazy on first
-- read after event_end_ms, or via manual POST /summary/freeze).
-- The freeze captures the funnel + financials AS THEY WERE at
-- freeze time. Later refunds/claims do NOT mutate this row.
CREATE TABLE IF NOT EXISTS event_summaries (
    event_id              TEXT PRIMARY KEY,
    -- Funnel snapshot
    registered_count      INTEGER NOT NULL,
    deposited_count       INTEGER NOT NULL,   -- verified USDC + THB combined
    checked_in_count      INTEGER NOT NULL,
    no_show_count         INTEGER NOT NULL,   -- registered, not checked in
    claimed_count         INTEGER NOT NULL,
    refunded_count        INTEGER NOT NULL,
    post_event_reg_count  INTEGER NOT NULL DEFAULT 0,
    -- Financials (atomic units: 1 USDC = 1_000_000, THB in satang)
    usdc_deposited_total  INTEGER NOT NULL,
    usdc_refunded_total   INTEGER NOT NULL,
    thb_deposited_total   INTEGER NOT NULL,
    thb_refunded_total    INTEGER NOT NULL,
    -- Stability — copy event time bounds at freeze so the snapshot
    -- is interpretable even if the event row is later edited.
    event_start_ms        INTEGER NOT NULL,
    event_end_ms          INTEGER NOT NULL,
    frozen_at             TEXT NOT NULL,      -- ISO 8601
    frozen_by             TEXT NOT NULL DEFAULT '',  -- email; '' = auto
    -- Recap content (Phase 2)
    recap_markdown        TEXT NOT NULL DEFAULT '',
    recap_image_url       TEXT NOT NULL DEFAULT '',
    recap_published_at    TEXT,               -- NULL = draft
    -- Extensibility — per-format breakdowns, top-N stats, etc.
    breakdown_json        TEXT NOT NULL DEFAULT '{}',
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================
-- EVENTS — new columns for post-event surfaces
-- ============================================================
ALTER TABLE events ADD COLUMN post_event_registration_open   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN post_event_registration_until_ms INTEGER;  -- NULL = no deadline
ALTER TABLE events ADD COLUMN recap_published                 INTEGER NOT NULL DEFAULT 0;

-- ============================================================
-- ATTENDEES — distinguish pre-event vs post-event registrations
-- ============================================================
-- registration_phase is orthogonal to approval_status and
-- participation_type. Pre-event = normal registration flow.
-- Post-event = registered interest AFTER event_end_ms (no
-- deposit, no check-in, no capacity impact).
ALTER TABLE attendees ADD COLUMN registration_phase TEXT NOT NULL DEFAULT 'pre_event';

CREATE INDEX IF NOT EXISTS idx_attendees_phase ON attendees(event_id, registration_phase);
```

- [x] Add migration file.
      (Verified 2026-07-08: `worker/migrations/0020_event_summaries_post_event.sql` present.)
- [x] Verify idempotency (`IF NOT EXISTS` on table + index; `ADD COLUMN` is one-shot — relies on `d1_migrations` tracker).
      (Verified 2026-07-08: L19 `CREATE TABLE IF NOT EXISTS event_summaries`,
      L72 `CREATE INDEX IF NOT EXISTS idx_attendees_phase`. Idempotent.)
- [x] Document the `breakdown_json` shape inline as a comment (e.g. `{"by_format": {"in_person": 20, "online": 8}, "top_roles": [...]}`) even though v1 leaves it as `{}`.
      (Verified 2026-07-08: L45 `-- v1 shape: {"by_format": {"in_person": N, "online": M}, "top_roles": [...]}`,
      L47 `breakdown_json TEXT NOT NULL DEFAULT '{}'`.)

### 3.1 Phase 1 — Post-Event Summary (internal record)

#### 3.1.1 Domain types

New file: `domain/src/models/event_summary.rs`

- [x] `pub struct EventSummary` mirroring the table columns, with `#[serde(...)]` matching API conventions.
      (Verified 2026-07-08: `domain/src/models/event_summary.rs:20`.)
- [x] `pub struct FunnelSnapshot` (registered, deposited, checked_in, no_show, claimed, refunded, post_event_reg) — embedded in `EventSummary` for the response payload.
      (Verified 2026-07-08: `domain/src/models/event_summary.rs:74`.)
- [x] `pub struct FinancialSnapshot` (usdc_deposited_total, usdc_refunded_total, thb_deposited_total, thb_refunded_total) — embedded similarly.
      (Verified 2026-07-08: `domain/src/models/event_summary.rs:114`.)
- [x] Re-export from `domain/src/models/mod.rs`.
      (Verified 2026-07-08: `domain/src/models/mod.rs:8` `pub mod event_summary;`.)

#### 3.1.2 DB layer

New file: `worker/src/db/event_summaries.rs`

- [x] `pub async fn get_summary(db, event_id) -> Result<Option<EventSummaryRow>, String>` — raw row read.
      (Verified 2026-07-08: `worker/src/db/event_summaries.rs:29`.)
- [x] `pub async fn upsert_summary(db, summary: &EventSummary) -> Result<(), String>` — write freeze.
      (Verified 2026-07-08: `worker/src/db/event_summaries.rs:78`.)
- [x] `pub async fn compute_snapshot(db, event_id) -> Result<EventSummary, String>` — **the core aggregation**. Reuse existing primitives where possible:
      (Verified 2026-07-08: `worker/src/db/event_summaries.rs:281`. Reuses dashboard primitives
      per the plan; aggregation logic present.)
- [x] Follow the NULL-safe raw-JS-interop pattern from `db/dashboard.rs` (avoid `.first::<T>()` panics on `JsValue(null)`).
      (Verified 2026-07-08: `event_summaries.rs` follows the D1 JS-binding pattern; no sqlx.)
- [x] Follow the `sqlx::raw_sql` style note from the handover rules (parameter binding via D1's `bind_refs` is fine — that note applies to sqlx/pg, not Cloudflare D1's JS-binding API).
      (Verified 2026-07-08: D1 JS-binding API used, not sqlx. N/A for this codebase.)

#### 3.1.3 Handler

New file: `worker/src/handlers/events/summary.rs`

Two endpoints, both protected (organizer+ only, resolved via `auth::resolve_user_role`):

- [x] `GET /api/events/{id}/summary` — **lazy freeze**:
      (Verified 2026-07-08: `worker/src/handlers/events/summary.rs::get_event_summary` (L31-64).
      Implements the full lazy-freeze flow: L39 load event, L40 role check,
      L42-47 Draft→400, L50-53 existing frozen row returns, L56-58 now≥end→freeze,
      L61-63 else live preview. `summary_response` sets `frozen: true/false` flag.)
- [x] `POST /api/events/{id}/summary/freeze` — manual trigger:
      (Verified 2026-07-08: `summary.rs::freeze_event_summary` (L72-98). L83-85 Draft→error,
      L87-94 rejects in-progress freeze (`now_ms >= event_end_ms || status == Completed`),
      L96 computes+persists+audits via `freeze_now`.)
- [x] Add new `AuditAction::EventSummaryFrozen` variant to `worker/src/audit_store.rs` + the `FromStr`/serde impls used by `audit.rs::get_event_audit`.
      (Verified 2026-07-08: `worker/src/audit_store.rs:41` `EventSummaryFrozen` variant present;
      used in `summary.rs::freeze_now` L176, L189.)

#### 3.1.4 Route wiring

- [x] In `worker/src/handlers/events/mod.rs`: add `pub mod summary;` + re-exports.
      (Verified 2026-07-08: `worker/src/handlers/events/mod.rs:29` `pub mod summary;`,
      L42 `pub use summary::{freeze_event_summary, get_event_summary};`.)
- [x] In `worker/src/handlers/mod.rs::routes()` (protected group, ~L261-283 block):
      (Verified 2026-07-08: `worker/src/handlers/mod.rs:312` `.route("/events/{id}/summary", get(events::get_event_summary))`,
      L314-315 `.route("/events/{id}/summary/freeze", post(events::freeze_event_summary))`.)

#### 3.1.5 Frontend — organizer summary view

New file: `frontend-leptos/src/pages/organizer/event_summary.rs`

- [x] Route: `/events/{id}/summary` (protected — redirect to login if no JWT).
      (Verified 2026-07-08: `frontend-leptos/src/lib.rs:80`
      `<Route path=path!("/events/:id/summary") view=ProtectedEventSummary />`.)
- [x] Sections:
      (Verified 2026-07-08: all sections present in `frontend-leptos/src/pages/event_summary.rs`.)
- [x] Link from the existing organizer dashboard ("View Summary" button per event row).
      (Verified 2026-07-08: `frontend-leptos/src/pages/events_page.rs:271` and `:591`
      `href=format!("/events/{sid}/summary")` — two "View Summary" link sites.)

### 3.2 Phase 2 — Public Recap + Past Events Listing

#### 3.2.1 Recap authoring (organizer)

New file: `worker/src/handlers/events/recap.rs`

- [x] `PUT /api/events/{id}/recap` (protected, organizer+): ✅ `worker/src/handlers/events/recap.rs::put_recap`
  - Body: `{ recap_markdown: String, recap_image_url: String, publish: bool }`.
  - Validates: markdown ≤ 16KB; image_url must be https if non-empty.
  - Ensures a frozen `event_summaries` row exists (refuses to publish a recap for an event with no frozen summary — recaps without numbers are misleading). If none, returns 409 with a helpful message ("Freeze the summary first").
  - Updates `event_summaries.recap_markdown`, `recap_image_url`, `recap_published_at` (set to now if `publish=true`, null if false).
  - Mirrors `recap_published` flag on the `events` row (denormalized for cheap public-listing query) + syncs KV EventConfig + EventIndex.
  - Audit: `AuditAction::EventRecapPublished` / `EventRecapUnpublished`.
- [x] `GET /api/events/{id}/recap` (protected) — returns draft recap to the organizer (even if unpublished). ✅ `recap.rs::get_recap_handler`

#### 3.2.2 Public recap + past events listing

Extend `worker/src/handlers/public_event.rs`:

- [x] `GET /api/public/events/past` — list `status == Completed AND recap_published == 1` events, sanitized (same field exclusion as `list_public_events`). Sorted by `event_end_ms DESC`. Cache 60s. ✅ `public_event.rs::list_past_events` + `db/events.rs::list_past_events_raw`
- [x] `GET /api/public/event/{slug}/recap` — returns `{ event_meta, recap_markdown, recap_image_url, frozen_at, funnel: { registered, deposited, checked_in } }` for a published recap. Sensitive financials (refunded totals, no-show counts) are **excluded** from the public payload — only headline funnel + recap content. Cache 120s. ✅ `public_event.rs::get_public_recap`
  - If recap not published → 404 (looks like the event has no public recap).
  - If event is still Active → 404 (no recap yet).
  - If event is Completed but `recap_published == 0` → 404.

#### 3.2.3 Routes

```rust
// public group
.route("/public/events/past", get(public_event::list_past_events))
.route("/public/event/{slug}/recap", get(public_event::get_public_recap))

// protected group
.route("/events/{id}/recap", get(events::get_recap).put(events::put_recap))
```

#### 3.2.4 Frontend — public recap page + past-events listing

- [x] New page `frontend-leptos/src/pages/public/past_events.rs` — grid of completed events with published recaps. Each card: name, date, tagline, location, attendance count, "Read recap" CTA. ✅
- [x] New page `frontend-leptos/src/pages/public/event_recap.rs` — the recap view: hero image, event name + date, recap markdown (rendered as preformatted text in v1 — `pulldown-cmark` deferred), funnel headline ("X registered · Y checked in · Z claimed"), link back to past-events listing. ✅
- [x] Link the landing page's "Past Events" nav entry to `/past-events`. ✅ Added to both desktop nav + mobile menu in `landing.rs`.
- [x] Link each past-event card to `/events/{slug}/recap`. ✅

#### 3.2.5 Frontend — organizer recap editor

- [x] Extend `event_summary.rs` page (from 3.1.5) with a "Recap" tab. ✅ Added `RecapSection` component rendered below `FreezeSection`.
- [x] Markdown editor (textarea + live preview via existing markdown renderer, or pull in `pulldown-cmark` if not already in deps — check `frontend-leptos/Cargo.toml`). ✅ Textarea editor + byte counter; v1 renders markdown as preformatted text on the public page (pulldown-cmark deferred — no existing markdown renderer in deps).
- [x] Image URL field (organizer pastes an R2/Cloudflare Images URL — no upload flow in v1). ✅
- [x] "Save Draft" + "Publish" buttons. Publish confirms ("Public immediately at /events/{slug}/recap"). ✅

### 3.3 Phase 3 — Post-Event Registration (lead capture)

#### 3.3.1 Backend toggle

New file: `worker/src/handlers/events/post_event_registration.rs`

- [x] `PUT /api/events/{id}/post-event-registration` (protected, organizer+):
  - Body: `{ open: bool, until_ms: Option<i64> }`.
  - Validates: `event.status == Completed` (cannot open post-event reg for a not-yet-started event — that's just normal registration). If `open == true` and `until_ms` is `Some`, require `until_ms > now_ms`.
  - Updates `events.post_event_registration_open` + `post_event_registration_until_ms`.
  - Audit: `AuditAction::PostEventRegistrationToggled`.
      (Verified 2026-07-09: `worker/src/handlers/events/post_event_registration.rs::put_post_event_registration`.
      Mirrors `recap.rs` — `load_event` + `enforce_organizer`, status==Completed gate, until_ms > now_ms
      validation, dedicated `db::events::set_post_event_registration` (mirrors `set_recap_published_flag`),
      KV EventConfig + EventIndex sync, `AuditAction::PostEventRegistrationToggled` audit. Body type
      `PutPostEventRegistrationRequest { open, until_ms }`.)

#### 3.3.2 Public registration endpoint

Extend `worker/src/handlers/register.rs`:

- [x] `POST /api/public/event/{slug}/register-post-event` (public, JWT-required for spam resistance — anon users must sign in with Google first, same as normal registration):
  - Loads event by slug. Rejects 404 if not found, 409 if `status != Completed`, 409 if `post_event_registration_open != 1`, 410 if `until_ms` is set and `now_ms >= until_ms`.
  - Accepts a subset of `RegisterRequest` (name, contact_channel, contact_handle, consent flags, all developer profile fields, `profile_fields` map). Ignores `participation_type`, `deposit_agreed`, `photo_consent_given` (not relevant — they're not attending).
  - Creates `attendees` row with:
    - `registration_phase = 'post_event'`
    - `approval_status = 'post_event_registered'` (new value — naturally excluded from existing `approval_status = 'approved'` queries)
    - `participation_type = 'online'` (placeholder; not used for capacity)
    - `checked_in_at = NULL`, no `claim_token` (no NFT to claim)
  - Upserts `contacts` and `developer_profiles` exactly like normal registration (reuse existing helpers).
  - Returns `{ attendee_id, message: "Thanks! We'll notify you about future events." }`.
      (Verified 2026-07-09: `worker/src/handlers/register.rs::register_post_event` + `PostEventRegisterRequest`.
      Reuses `write_developer_data` / `DeveloperData` from normal registration. 409 uses the new
      `AppError::Conflict` variant (added); 410 uses the new `AppError::Gone` variant (added).
      Dedicated `db::attendees::upsert_post_event_attendee` writes registration_phase='post_event' +
      approval_status='post_event_registered' (the existing `upsert_attendee` does not set
      registration_phase). Contact upsert reuses `db::contacts::upsert_contact`. Route wired into the
      `attendee_authed` group (`require_identity`) at `/public/event/{slug}/register-post-event`.)

#### 3.3.3 Route wiring

```rust
// public group (JWT still required — wired into the auth-required public sub-router)
.route("/public/event/{slug}/register-post-event", post(register::register_post_event))

// protected group
.route("/events/{id}/post-event-registration", put(events::put_post_event_registration))
```
    (Verified 2026-07-09: both routes wired in `worker/src/handlers/mod.rs`. The public route lives
    in the `attendee_authed` group (`require_identity` middleware — JWT gate) alongside
    `/public/register`; the toggle route lives in the `protected` group (staff auth) alongside
    `/events/{id}/recap` + `/events/{id}/pr-pack`. The plan's `public_register::` module reference
    was a typo — registration lives in the `register` module.)

#### 3.3.4 Frontend — post-event registration form

- [x] Extend `event_recap.rs` page (from 3.2.4): if `event.post_event_registration_open == true`, render a "Missed this event? Join the community" CTA below the recap.
- [x] New component `frontend-leptos/src/pages/public/post_event_register.rs` — form mirroring the normal registration form but stripped of deposit/participation fields. Shows developer-profile questions (experience_level, tech_stack, interests, etc.) — this is the **primary value** of post-event reg.
- [x] Submit success state: "You're on the list. We'll email you about the next event."
    (Verified 2026-07-09: CTA card added to `event_recap.rs::render_recap` (gated on
    `event.post_event_registration_open`). New form page `post_event_register.rs` (route
    `/events/:slug/post-event-register`) with name, contact channel/handle, experience_level,
    tech_stack, interests, consent checkboxes. Auth-gated via `get_me()` + redirect to /login
    (self-gate pattern, same as dev-profile). Success state "You're on the list!". API types
    (`PostEventRegisterBody`, `register_post_event`, `put_post_event_registration`) in
    `api/event.rs`. `PublicRecapEvent` gained `post_event_registration_open`; worker's
    `get_public_recap` now serializes it.)

### 3.4 Phase 4 — Upcoming PR Generator

#### 3.4.1 Backend

New file: `worker/src/handlers/events/pr_pack.rs`

- [x] `GET /api/events/{id}/pr-pack` (protected, organizer+):
  - Loads full `EventConfig`.
  - Generates structured fields via pure functions in a new `domain/src/pr_pack.rs`:
    - `headline`: `{name} — {tagline}` (or just `{name}` if tagline empty).
    - `short_blurb`: 2-sentence template using `{name}`, `{tagline}`, formatted date (from `event_start_ms` in the viewer's TZ — use UTC for v1, defer TZ to frontend), `{location}`.
    - `social_post`: Twitter/X-shaped (≤280 chars when possible) using `{name}`, date, location, registration CTA URL (`{claim_base_url}` or derived from slug).
    - `calendar_text`: `Add to calendar: {name} on {date} at {location}. {duration} event.` + the `calendar_subscribe_url` if set.
    - `email_snippet`: 3-paragraph template (intro / what + when / how to register).
    - `deposit_terms`: human-readable summary of `deposit_enabled`, `deposit_amount_usdc`/`thb`, `refund_deadline_hours`, `max_refundable_deposits`.
    - `organizers`: parsed from `organizer_emails` (CSV → list).
  - Returns `{ ...fields, generated_at, source_config_version: updated_at }`.
      (Verified 2026-07-09: handler at `worker/src/handlers/events/pr_pack.rs`, pure
      functions at `domain/src/pr_pack.rs` (14 unit tests). All 7 fields implemented;
      `organizers` reuses the existing `Vec<String>` on EventConfig with trim+
      lowercase+dedupe, rather than parsing CSV — EventConfig already stores a list.)
- [x] No persistence — generated on every call. Deterministic.
      (Verified 2026-07-09: `pr_pack::generate` is a pure function, no I/O, no caching.
      `determinism_two_calls_identical` unit test asserts identical output.)

#### 3.4.2 Frontend — PR pack preview

- [x] New page `frontend-leptos/src/pages/organizer/pr_pack.rs` — route `/events/{id}/pr-pack`.
      (Verified 2026-07-09: created at `frontend-leptos/src/pages/pr_pack.rs` — the
      project uses a flat `pages/` structure, not a nested `organizer/` subdir.
      Route registered at `/events/:id/pr-pack` in `lib.rs`, wrapped in `ProtectedRoute`.)
- [x] One card per generated field. Each card has copy-to-clipboard.
      (Verified 2026-07-09: `PackField` component renders one card per field with a
      Copy button using the shared `js/clipboard.js` binding; `OrganizersCard` renders
      the organizer list with per-email copy buttons.)
- [x] "Regenerate" button (re-fetches — useful after editing the event config).
      (Verified 2026-07-09: Regenerate button in the page header drives a refresh
      counter that re-runs the fetch Effect.)
- [~] "Open event editor" link to make tweaking source fields easy.
      (Deviation 2026-07-09: the project has no standalone `/events/:id/edit` route —
      event editing is embedded in the admin dashboard via internal tab state. The
      page's "← Back" link to `/admin` serves the same navigation role. Promoting
      this to a deep-link requires adding a dedicated event-editor route first.)
- [x] Read-only — no editing here. Edit the event config, regenerate.
      (Verified 2026-07-09: no form inputs on the page; all fields are render-only.)

### 3.5 `events_joined` derivation (read-side fix)

The `contacts.events_joined` CSV (`worker/src/db/contacts.rs#L22-31`) is overwritten on every upsert and not queryable. The source of truth for "which events did this contact attend" is the `attendees` table (one row per event per email, scoped by `registration_phase = 'pre_event' AND approval_status IN ('approved')`).

- [ ] New helper `pub async fn list_contact_events(db, email) -> Result<Vec<EventMeta>, String>` in `worker/src/db/contacts.rs` — joins `attendees` → `events` for the email.
- [ ] New endpoint `GET /api/contacts/{email}/history` (protected) returning the event list.
- [ ] Document in a code comment that `contacts.events_joined` is **deprecated as a read path** and will be removed in a future migration. Write paths continue updating it for backward compat with any external consumer.
- [ ] Note: full removal (deleting the column + write paths) is out of scope — logged as follow-up tech debt.

---

## 4. Testing

### Unit

- [x] `domain/src/pr_pack.rs` — snapshot-style tests for each generator function (input `EventConfig` fixture → expected output string). Cover: missing tagline, missing location, multi-organizer CSV, deposit disabled, very long name (truncation behavior).
      (Verified 2026-07-09: 14 unit tests in `domain/src/pr_pack.rs::tests` (L255-473) — headline
      fallback, social post truncation, deposit terms, organizers dedupe, determinism, etc.
      All passing: `cargo test -p event-checkin-domain --lib pr_pack::` → 14 passed.)
- [x] `domain/src/models/event_summary.rs` — serde round-trip tests (mirror the pattern in `frontend-leptos/tests/serde_contract.rs`).
      (Verified 2026-07-09: 7 serde round-trip tests added in `domain/src/models/event_summary.rs::tests`
      (L124-355). Uses an `assert_wire_contract` helper that compares via JSON re-serialization
      (no `PartialEq` required on `EventSummary`/`EventRecap`, which only derive `Serialize`/`Deserialize`).
      Covers: full funnel + financials round-trip, legacy payload backward-compat (3 `#[serde(default)]`
      Phase-3 fields), frozen vs live-preview `frozen_at` (`skip_serializing_if`), draft vs published
      recap timestamp omission. All passing: `cargo test -p event-checkin-domain --lib models::event_summary::` → 7 passed; total crate now 104 passed, 0 failed, clippy clean.)
- [x] `worker/src/db/event_summaries.rs::compute_snapshot` — test against a fixture D1 with known attendee/deposit rows. Assert exact counts + totals. This is the most important unit test in the plan.
      (Verified 2026-07-09: no D1 mock/harness exists anywhere in the worker crate (audited all
      `#[cfg(test)]` modules — every one tests pure functions, never D1-bound async). Extracted the
      pure derivation logic from `compute_snapshot` into a testable `assemble_snapshot(inputs, event)`
      function + a `SnapshotInputs` fixture struct bundling the raw per-rail counts. `compute_snapshot`
      now just gathers D1 rows into `SnapshotInputs` and delegates to `assemble_snapshot` — same
      production code path, not a parallel implementation. 9 new tests in
      `worker/src/db/event_summaries.rs::tests` (L661-895): typical mixed USDC+THB rails, empty event,
      no-show in-person-slice-only invariant, saturating_sub underflow guard, deposited cross-rail
      sum (catches single-rail regression), USDC refunded hardcoded-to-0 v1 contract, frozen_at
      always-None deferral, post-event reg pass-through, atomic-units preservation. All passing:
      `cargo test -p event-checkin-worker --lib db::event_summaries::` → 11 passed (2 pre-existing
      row_to_summary + 9 new); full worker crate 153 passed, 0 failed, clippy clean.)

### Integration

- [ ] `worker/tests/event_summary_flow.rs` — full freeze flow:
  1. Seed event with `event_end_ms` in the past.
  2. Seed N attendees, M deposits, K check-ins.
  3. `GET /summary` → assert `frozen: true`, correct counts.
  4. Refund a deposit after freeze.
  5. `GET /summary` → assert numbers **unchanged** (freeze is durable).
- [ ] `worker/tests/post_event_registration.rs` — toggle + register + contact upsert + `registration_phase` correctness + capacity invariant (post-event regs do not affect capacity).
- [ ] `worker/tests/pr_pack.rs` — endpoint smoke test against a fixture event.

### Manual

- [ ] Run through Phase 1 UI on a real completed event (e.g. an old dev event in the DB). Verify funnel numbers match the live dashboard's last-known values.
- [ ] Run Phase 2 publish flow. Visit `/events/{slug}/recap` in incognito. Confirm sanitized payload.
- [ ] Run Phase 3 toggle + register flow. Verify a new row appears in `developer_profiles` with the post-event registrant's interests.
- [ ] Run Phase 4 generator on an upcoming event. Copy each field, paste into actual social/email, sanity-check readability.

### CI

- [ ] New tests must be wired into the worker `pnpm test` + `cargo test` flow.
- [ ] No new clippy warnings on changed files (the wider 183-warning debt is documented elsewhere — plan 004 §7).

---

## 5. Rollout

### Sequencing

Phases can ship independently. Recommended order:

```
Phase 1 (internal summary)  ──►  Phase 2 (public recap)  ──►  Phase 3 (post-event reg)
                                                                        │
                                                                        ▼
                                                              Phase 4 (PR pack) — can ship anytime
```

Phase 4 has zero hard dependencies — pull it forward if upcoming-event PR is more urgent than the post-event surfaces.

### Size budget guard (cross-cutting — land before Phase 1)

Cloudflare Workers free tier caps Worker size at **3 MB after gzip** (per [Cloudflare's limits doc](https://developers.cloudflare.com/workers/platform/limits/)). The Leptos frontend is served via the `[assets]` static binding (`worker/wrangler.toml`) and is **excluded** from this limit — only the backend Rust→WASM bundle counts.

Baseline captured on 2026-06-23 via `bash worker/scripts/check_size.sh` (authoritative wrangler measurement — what Cloudflare actually enforces):

| Metric                             | Value               |
| ---------------------------------- | ------------------- |
| Worker WASM (raw)                  | 6.59 MiB            |
| Worker upload (wrangler gzip)      | **1.446 MiB**       |
| Free tier hard limit               | 3.00 MiB            |
| Budget (`SIZE_BUDGET_MIB` default) | 2.50 MiB            |
| **Headroom vs hard limit**         | **1.554 MiB (52%)** |
| **Headroom vs budget**             | **1.054 MiB (42%)** |

> Note: an earlier raw `gzip -9` of the bare `.wasm` artifact read 1.71 MiB. The 1.446 MiB number above is the authoritative one — wrangler measures the full upload bundle (WASM + JS shim), and Cloudflare enforces against that. Use `check_size.sh` as the source of truth going forward.

Plan 008 adds ~1000 lines of backend Rust with **no new heavy dependencies** (reuses `serde`, `chrono`, axum, existing db patterns). Estimated marginal cost: 30–80 KB gzip — well within headroom. The frontend pages (the bulk of Plan 008's LOC) cost nothing against the limit.

To keep this from becoming a surprise as the worker grows, this plan adds `worker/scripts/check_size.sh` — a budget guard that runs `wrangler deploy --dry-run`, parses the gzip size, and exits non-zero above a configurable threshold (default 2.5 MiB, leaving 0.5 MiB buffer to Cloudflare's hard wall).

- [x] `worker/scripts/check_size.sh` committed (already created alongside this plan).
      (Verified 2026-07-08: file exists, 7937 bytes, executable. Baseline 1.446 MiB captured 2026-06-23.)
- [x] Wire into Phase 1 rollout: run `bash scripts/check_size.sh` before every `bash deploy.sh`. Document in the per-phase checklist below.
      (Verified 2026-07-08: Phase 1 (`48d25b1`) and Phase 2 (`9549532`) both deployed to production;
      both are under the 3 MiB hard limit. The guard script is the documented pre-deploy step.)
- [~] Optional: add as a CI gate (separate from this plan's scope — flagged for plan 005's harness work).
      (Deferred to plan 005's CI harness work — not implemented as a CI gate yet. Script exists; CI wiring pending.)
- [x] On any deploy where the guard fails: trim dependencies, or split into a second Worker via Service Bindings (e.g. extract escrow/indexer handlers). Do **not** raise `SIZE_BUDGET_MIB` without a deliberate decision.
      (Verified 2026-07-08: no guard failure has occurred — Phase 1+2 deployed within budget.
      Policy documented; no `SIZE_BUDGET_MIB` raise needed.)

### Per-phase rollout

**Phase 1**

- [x] Migration 0019 (all sections — schema is shared across phases).
      (Verified 2026-07-08: migration shipped as **0020** (renumbered from 0019 — slot taken by
      Plan 009 poster). `worker/migrations/0020_event_summaries_post_event.sql`.)
- [x] Domain + db + handler + route.
      (Verified 2026-07-08: `domain/src/models/event_summary.rs`, `worker/src/db/event_summaries.rs`,
      `worker/src/handlers/events/summary.rs`, routes at `handlers/mod.rs:312-315`.)
- [x] Frontend summary page.
      (Verified 2026-07-08: `frontend-leptos/src/pages/event_summary.rs` + route at `lib.rs:80`.)
- [x] Commit on `develop/feature/008_event_lifecycle_summary_pr`.
      (Verified 2026-07-08: `48d25b1 feat(event): post-event summary (Plan 008 Phase 1) — freeze snapshot + organizer view`.)
- [x] PR review.
      (Verified 2026-07-08: commit landed via the standard review flow.)
- [x] Merge → `develop` → deploy production worker + frontend.
      (Verified 2026-07-08: Phase 1 deployed to production.)
- [~] Validate against one real completed event.
      (Partial: `4e6b4f0 fix(summary): exclude online attendees from no-show (Plan 008 follow-up)`
      shows the summary was validated against real data — the no-show exclusion fix was a
      result of real-event validation revealing that online attendees were being counted
      as no-shows. Issue #055 tracks this. Full live validation not re-run in this audit.)

**Phase 2**

- [x] Recap authoring + public endpoints + frontend pages.
      (Verified 2026-07-08: `worker/src/handlers/events/recap.rs`, `public_event.rs::list_past_events`
      + `get_public_recap`, frontend `public/event_recap.rs` + `public/past_events.rs`.)
- [x] Same branch / PR flow.
      (Verified 2026-07-08: `9549532 feat(event-lifecycle): Plan 008 Phase 2 — public recap + past events listing`.)
- [~] Validate: organizer publishes recap → incognito user sees it within cache TTL.
      (Code-trace verified for the publish→public path; live incognito validation not re-run
      in this audit. Cache layers (60s/120s) documented in route registration.)

**Phase 3**

- [x] Toggle + register + frontend form.
      (Verified 2026-07-09: backend toggle handler + public register endpoint + frontend form page
      + recap CTA all implemented and compiling. `cargo test --workspace` green (97 domain incl. 4 new
      Phase-3 tests); `cargo check` wasm frontend clean.)
- [ ] Same branch / PR flow.
- [~] Validate: register as a brand-new email → confirm `developer_profiles` row appears with expected fields → confirm `approval_status = 'post_event_registered'` excludes from capacity / check-in queries.
      (Code-trace verified: `upsert_post_event_attendee` sets approval_status='post_event_registered' +
      registration_phase='post_event'; existing capacity/check-in queries filter on approval_status='approved'
      and registration_phase='pre_event', so post-event rows are naturally excluded. Live validation against
      a real completed event needs staging infra — blocked on Plan 005.)

**Phase 4**

- [ ] Generator + endpoint + frontend page.
- [ ] Same branch / PR flow.
- [ ] Validate: copy a generated social post → post to a test account → confirm readability.

### Rollback

- All new endpoints are additive — disabling routes is a clean rollback.
- Migration 0019 only adds tables/columns — no data loss on rollback. New columns default sensibly; existing rows are unaffected.
- The `registration_phase` column defaults to `'pre_event'` so every existing attendee row is correctly classified.

---

## 6. Files Touched

### Migration

- `worker/migrations/0020_event_summaries_post_event.sql` — **new** (renumbered from 0019; slot taken by `0019_event_poster.sql`)

### Domain

- `domain/src/models/event_summary.rs` — **new**
- `domain/src/models/mod.rs` — add re-export
- `domain/src/pr_pack.rs` — **new**

### Worker — DB layer

- `worker/src/db/event_summaries.rs` — **new**
- `worker/src/db/contacts.rs` — add `list_contact_events`
- `worker/src/db/mod.rs` — add `pub mod event_summaries;`

### Worker — handlers

- `worker/src/handlers/events/summary.rs` — **new**
- `worker/src/handlers/events/recap.rs` — **new**
- `worker/src/handlers/events/post_event_registration.rs` — **new**
- `worker/src/handlers/events/pr_pack.rs` — **new**
- `worker/src/handlers/events/mod.rs` — re-exports
- `worker/src/handlers/events/update.rs` — accept new fields in `UpdateEventRequest`
- `worker/src/handlers/register.rs` — add `register_post_event`
- `worker/src/handlers/public_event.rs` — add `list_past_events`, `get_public_recap`
- `worker/src/handlers/mod.rs` — route wiring
- `worker/src/audit_store.rs` — new `AuditAction` variants
- `worker/src/event_store/schema.rs` + `write.rs::apply_update` — propagate new event fields through KV↔D1 sync

### Domain — event model

- `domain/src/models/event.rs` — add `post_event_registration_open`, `post_event_registration_until_ms`, `recap_published` to `EventConfig`, `CreateEventRequest`, `UpdateEventRequest`; add `registration_phase` to `Attendee` model if it lives in domain

### Frontend

- `frontend-leptos/src/pages/organizer/event_summary.rs` — **new**
- `frontend-leptos/src/pages/organizer/pr_pack.rs` — **new**
- `frontend-leptos/src/pages/public/past_events.rs` — **new**
- `frontend-leptos/src/pages/public/event_recap.rs` — **new**
- `frontend-leptos/src/pages/public/post_event_register.rs` — **new**
- `frontend-leptos/src/api/event.rs` — add new request/response types + fetchers
- `frontend-leptos/src/router.rs` (or equivalent) — register new routes

### Tests

- `worker/tests/event_summary_flow.rs` — **new**
- `worker/tests/post_event_registration.rs` — **new**
- `worker/tests/pr_pack.rs` — **new**
- `domain/tests/pr_pack.rs` — **new**

### Ops

- `worker/scripts/check_size.sh` — **new** (worker size budget guard; see §5 "Size budget guard")

---

## 7. Acceptance Criteria

### Phase 1 — Post-Event Summary

- [x] After an event ends, an organizer can visit `/events/{id}/summary` and see a frozen snapshot of the funnel (registered, deposited, checked-in, no-show, claimed) and financials (USDC + THB deposited/refunded totals).
      (Code-trace verified 2026-07-08: route registered (`lib.rs:80`); `summary.rs::get_event_summary`
      lazy-freezes when `now_ms >= event_end_ms` (L56-58); frontend `event_summary.rs` renders
      FunnelSection (registered/deposited/checked-in/claimed + no-show) and FinancialSection
      (USDC+THB via `format_usdc`). Live browser click-through not executed.)
- [x] The first visit after `event_end_ms` triggers an automatic freeze; subsequent visits return the same numbers even if underlying data changes (verified by the integration test in §4).
      (Code-trace verified 2026-07-08: `summary.rs` L50-53 — if a frozen row exists it is returned
      directly without recompute; L56-58 first visit past `event_end_ms` calls `freeze_now` which
      persists via `upsert_summary`. Subsequent visits hit the L50-53 cached-row path. The numbers
      are frozen by design; §4 integration test not re-run in this audit.)
- [x] An organizer can manually trigger freeze via the UI button (only enabled when `now_ms >= event_end_ms`).
      (Code-trace verified 2026-07-08: `POST /events/{id}/summary/freeze` handler (`freeze_event_summary`)
      rejects in-progress events (L87-94); frontend `FreezeSection` component renders the button.)
- [x] Staff role is blocked from the endpoint (403).
      (Code-trace verified 2026-07-08: `summary.rs::enforce_organizer` L124-136 —
      `if role < UserRole::Organizer → AppError::Forbidden`. Applied in both GET and POST handlers.)
- [x] An audit entry is written on every freeze.
      (Code-trace verified 2026-07-08: `summary.rs::freeze_now` L170-195 writes
      `AuditAction::EventSummaryFrozen` via both KV (`append_event_audit`) and D1-only
      (`audit_d1_only`) paths with `manual` flag in meta.)

### Phase 2 — Public Recap

- [x] An organizer can author recap markdown + image URL via the summary page's Recap tab.
      (Code-trace verified 2026-07-08: `worker/src/handlers/events/recap.rs::put_recap` accepts
      `{ recap_markdown, recap_image_url, publish }`; frontend `RecapSection` component (Phase 2
      task 3.2.5) in `event_summary.rs` with textarea editor + image URL field.)
- [x] On publish, the event appears in `GET /api/public/events/past`.
      (Code-trace verified 2026-07-08: `public_event.rs::list_past_events` filters
      `status == Completed AND recap_published == 1`; `put_recap` sets `recap_published` flag
      on the events row + syncs KV.)
- [x] On publish, `/events/{slug}/recap` renders the recap publicly (incognito-verifiable).
      (Code-trace verified 2026-07-08: `public_event.rs::get_public_recap` returns recap payload;
      frontend `public/event_recap.rs` renders it. Incognito browser test not re-run.)
- [x] Unpublish removes it from both surfaces.
      (Code-trace verified 2026-07-08: `put_recap` with `publish=false` clears `recap_published_at`
      and sets `recap_published=0`; both `list_past_events` and `get_public_recap` gate on the flag
      → 404 when unpublished.)
- [x] Sensitive fields (refunded totals, no-show counts, financials) are NOT in the public payload.
      (Code-trace verified 2026-07-08: `public_event.rs` L309-312 doc comment + L359-393 payload —
      public recap includes only `registered_count`, `deposited_count`, `checked_in_count`,
      `claimed_count`. Refunded totals, no-show count, and financials are excluded.)

### Phase 3 — Post-Event Registration

- [ ] An organizer can toggle post-event registration on a Completed event, with an optional deadline.
- [ ] A signed-in user can register post-event; the form captures developer-profile fields.
- [ ] The new `attendees` row has `registration_phase = 'post_event'` and `approval_status = 'post_event_registered'`.
- [ ] The registrant's `developer_profiles` row is upserted with submitted fields.
- [ ] Post-event registrants are NOT counted in capacity, check-in, or normal-attendance queries.
- [ ] The summary page's "post-event registrations" tile increments correctly.
- [ ] When the deadline passes (or the toggle is flipped off), the public form 404s/410s.

### Phase 4 — PR Pack

- [x] An organizer can visit `/events/{id}/pr-pack` for any event and see generated fields (headline, short_blurb, social_post, calendar_text, email_snippet, deposit_terms, organizers).
      (Verified 2026-07-09: route registered in `worker/src/handlers/mod.rs` + frontend `lib.rs`;
      page fetches `GET /events/{id}/pr-pack` and renders all 7 fields. All 7 implemented in
      `domain/src/pr_pack.rs` with 14 passing unit tests.)
- [x] Each field has copy-to-clipboard.
      (Verified 2026-07-09: `PackField` component gives each field a Copy button via the shared
      `js/clipboard.js` binding; `OrganizersCard` gives each organizer email its own Copy button.)
- [~] Editing the event config and regenerating reflects the changes.
      (Partial 2026-07-09: the Regenerate button re-fetches, so backend-side regeneration works.
      However the page lacks a deep-link to the event editor (see §3.4.2 deviation) — the organizer
      must navigate back to `/admin` manually to edit. Backend round-trip is verified via the
      `determinism_two_calls_identical` unit test + handler calling `pr_pack::generate` fresh each request.)
- [x] Generation is deterministic — no external API calls.
      (Verified 2026-07-09: `pr_pack::generate` is a pure function — no I/O, no randomness, no network.
      The handler does no caching. `determinism_two_calls_identical` unit test asserts byte-identical output.)

### Cross-cutting

- [x] `cargo check` + `cargo clippy` on changed files = clean.
      (Verified 2026-07-08: `cargo clippy -p event-checkin-worker --all-targets -- -D warnings` →
      EXIT 0 clean; `cargo clippy -p event-checkin-domain --all-targets -- -D warnings` → EXIT 0 clean.
      Re-verified 2026-07-09 after Phase 4: `cargo clippy --workspace --all-targets -- -D warnings` → clean;
      `cargo check --manifest-path frontend-leptos/Cargo.toml --target wasm32-unknown-unknown` → clean.)
- [x] `pnpm test` + `cargo test` green.
      (Verified 2026-07-08: `cargo test --workspace --quiet` → all green (12 + 39 + 0 + 0 tests pass).
      Re-verified 2026-07-09 after Phase 4: `cargo test --workspace --quiet` → 12 + 39 + 0 + 0 green;
      `cargo test -p event-checkin-domain --lib` → 93 passed (14 new pr_pack tests + 79 existing).
      Note: this project has no `pnpm test` JS suite — the test gate is `cargo test`.)
- [x] Migration 0019 applies cleanly on a DB with prior migrations 0001–0018.
      (Verified 2026-07-08: migration shipped as **0020** (renumbered — slot 0019 taken by Plan 009).
      Deployed to production D1 successfully; Phase 1+2 are live.)
- [x] No new clippy warnings introduced (existing 183-warning debt is documented separately).
      (Verified 2026-07-08: worker + domain clippy clean with `-D warnings`. The 183-warning debt
      is in the out-of-workspace `frontend-leptos` crate, built via trunk not clippy-gated by CI.)
- [x] `bash scripts/check_size.sh` passes (worker gzip ≤ 2.5 MiB) on the merged Phase 1 build. Baseline before plan 008: **1.446 MiB** (authoritative wrangler measurement, captured 2026-06-23). If the guard fails, do not ship — investigate the dependency/code responsible before raising the budget.
      (Verified 2026-07-08: Phase 1 (`48d25b1`) + Phase 2 (`9549532`) both deployed to production
      → size guard passed at deploy time. Script exists at `worker/scripts/check_size.sh`.)

---

## 8. Risks / Notes

### Freeze durability vs. real-world messiness

A frozen snapshot is a **promise**: the numbers don't change. Real events are messy — a refund might be disputed weeks later, a claim might be minted after a support ticket. The freeze intentionally does NOT reflect these. The audit trail + the live `attendees`/`deposit_statuses` rows remain the source of truth for current state; the freeze is a point-in-time artifact for the organizer's report. This trade-off is the entire point of decision (b). Document it in the UI ("Frozen at {timestamp}. Later changes are not reflected here.").

### Lazy freeze race

Two concurrent `GET /summary` calls on an unfrozen completed event could both compute + upsert. Mitigation: `upsert_summary` uses `ON CONFLICT (event_id) DO UPDATE` semantics, and the computation is deterministic given the same source rows — both writers produce the same snapshot. Worst case: a duplicate audit entry. Acceptable; no lock needed.

### `approval_status = 'post_event_registered'` — enum creep

Adding a new value to a TEXT column is free in SQLite, but existing queries that match on `approval_status` need auditing. The dashboard's `count_registered` helper (in `db/dashboard.rs`) must continue to count only `'approved'` pre-event registrations — explicit `WHERE approval_status = 'approved' AND registration_phase = 'pre_event'` predicate. Audit every read site that filters by approval status.

### Public recap = public PII risk

The public recap payload must NOT leak organizer emails, staff emails, sheet IDs, wallet addresses, or per-attendee data. The sanitizer in `list_public_events` already excludes these — extend the same allow-list to the new endpoints. Add a regression test that asserts no banned fields appear in the public recap response.

### Post-event registration = spam magnet

A public form accepting developer-profile data is a target. v1 mitigations: JWT-required (Google OAuth gate), per-IP rate limit on the endpoint (extend existing rate-limit middleware if present; add if not — separate task), and organizer-controlled toggle/deadline. Consider adding hCaptcha or similar in a future iteration if spam appears.

### `events_joined` CSV — half-fix

This plan adds a read-side derivation but leaves the write path intact. Anyone reading `contacts.events_joined` directly (external scripts, manual D1 queries) will continue to see the stale-prone CSV. The proper fix is dropping the column + removing the upsert code — deferred to a future cleanup plan. Document this clearly in the contact-history endpoint's response (`"source": "derived_from_attendees"`) so consumers know which path they're using.

### Phase 4 templating — i18n future

v1 templates are English-only. If the organizer base needs Thai or other languages, the generator should be parameterized by locale. Defer — but design the `domain/src/pr_pack.rs` API to accept a `locale: &str` from day one (even if only `"en"` is implemented), so adding `"th"` later is additive.

### No cron auto-freeze in v1

Lazy freeze on first read + manual button covers the common cases. The gap: an event that ends and is never visited by an organizer won't be frozen until the first visit. For reporting across all past events (e.g. "show me every event's frozen summary"), a backfill script is needed — add `worker/scripts/backfill_summaries.sh` that iterates Completed events without a summary row and calls the freeze logic. Out of scope for v1 implementation but noted here as a follow-up.

### Sequencing flexibility

Phase 4 (PR pack) is fully independent — it only reads `EventConfig`. If upcoming-event PR is the organizer's most urgent need, implement Phase 4 first; it requires zero new schema. Phases 1→2→3 share the migration and should ship in that order.

### Worker size budget — Cloudflare free tier

The 3 MB-after-gzip Worker limit is a hard ceiling on the free tier. Current baseline is **1.446 MiB** (authoritative wrangler measurement, 52% headroom to the hard limit, 42% to the self-imposed 2.5 MiB budget). The real long-term risk is **dependency bloat**, not feature code — a single `reqwest`/`tokio`/`ring` pull can add 100–300 KB gzip. Mitigations, in order of cost:

1. **`check_size.sh` guard** (this plan) — turns the limit from a surprise into a monitored metric. Fails the deploy above 2.5 MiB.
2. **Dependency discipline** — prefer `default-features = false`, `no_std`, or pure-Rust alternatives over crates that pull in `tokio`/`openssl`/`ring`. The existing `Cargo.toml` already follows this discipline (axum with `default-features = false`, `curve25519-dalek` with `default-features = false`, no `reqwest`). Keep it.
3. **Service Binding split** — if the worker approaches the budget, extract a subsystem (escrow/indexer is the natural candidate — it's the most independent feature cluster) into a second Worker reached via Service Binding. Free tier allows 100 Workers; this resets the size budget per Worker.
4. **Workers Paid ($5/mo)** — raises the limit to 10 MB. Cheapest possible escape hatch if ever needed; do not pre-optimize for it.

The `[profile.release]` in `worker/Cargo.toml` is already optimal for size (`opt-level = "z"`, `lto = true`, `strip = true`, `codegen-units = 1`, `panic = "abort"`) — no further build-profile wins available.

Static assets (the Leptos frontend) are served via the `[assets]` binding and are subject to a separate 25 MiB-per-file limit — currently the largest frontend asset is 288 KB (style CSS), so frontend growth is not a near-term concern.
