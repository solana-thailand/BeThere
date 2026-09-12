# Plan 008 — Event Lifecycle: Summary, Recap, Post-Event Registration, PR Generator

> **Status**: Phase 1 (Post-Event Summary) ✅ shipped (`48d25b1`, deployed) · Phase 2 (Public Recap + Past Events) ✅ merged to `develop` 2026-07-16 via PR #20 (rebased from `feature/event_recap` `9549532` + visibility-fix `74bc43d`); deploy pending · Phase 3 (Post-Event Registration) ✅ merged to `develop` 2026-07-16 via PR #20; its completed-event gateway is merged at `8e8673b`, and its retrospective-isolation hardening is merged at `9954731` (2026-09-12), deployed to staging as Worker version `2cf965f1-84d1-4864-b899-10d0011d3554`. The isolated staging API journey passed: a completed event with a Genesis archive link persisted one `retrospective` lead, returned `online_count = 0`, created no money/NFT state, and rejected a later submission with `409`. Browser validation through organizer UI remains pending (never raw D1) · Phase 4 (PR Generator) ✅ merged to `develop` 2026-07-16 via PR #20 (originally `63270ac`). The hardening passed domain tests (131), Worker tests (252), formatting, strict Worker clippy, frontend WASM build, and staging health/content-type checks.
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
    - `participation_type = 'retrospective'` (a dedicated post-event-learning
      value; never reuse `online`, which is a published live-registration metric)
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

- [x] Completed events use `/e/{slug}` as the canonical gateway: it exposes a
  trusted Genesis archive link and, only when the server says enrollment accepts
  a submission now, a "Join the community" CTA. A recap remains optional and
  is never required for the gateway.
- [x] New component `frontend-leptos/src/pages/public/post_event_register.rs` — form mirroring the normal registration form but stripped of deposit/participation fields. Shows developer-profile questions (experience_level, tech_stack, interests, etc.) — this is the **primary value** of post-event reg.
- [x] Submit success state: "You're on the list. We'll email you about the next event."
    (Verified 2026-07-09: CTA card added to `event_recap.rs::render_recap` (gated on
    `event.post_event_registration_open`). New form page `post_event_register.rs` (route
    `/events/:slug/post-event-register`) with name, contact channel/handle, experience_level,
    tech_stack, interests, consent checkboxes. Auth-gated via `get_me()` + redirect to /login
    (self-gate pattern, same as dev-profile). Success state "You're on the list!". API types
    (`PostEventRegisterBody`, `register_post_event`, `put_post_event_registration`) in
      `api/event.rs`. `PublicRecapEvent` gained `post_event_registration_open`; worker's
      `get_public_recap` now serializes it. The completed-event gateway was added in
      `8e8673b`: the public-event payload supplies a server-calculated accepting flag
      and a Genesis URL only when `events.link` is a trusted Genesis event path.)

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

- [x] New helper `pub async fn list_contact_events(db, email) -> Result<Vec<ContactEventRow>, String>` in `worker/src/db/contacts.rs` — joins `attendees` → `events` for the email.
      (Verified 2026-07-09: `list_contact_events` at `worker/src/db/contacts.rs` JOINs attendees→events,
      scoped to `registration_phase = 'pre_event' AND LOWER(approval_status) = 'approved'`, ordered by
      `event_start_ms DESC`. Email is bound as positional `?1` (not interpolated) — guards against SQL
      injection, mirroring `audience_aggregate` rather than the `exec`-with-format `clear_contact_pii`.
      Return type renamed from the plan's `EventMeta` to a new `ContactEventRow` that carries
      per-registration detail (`checked_in_at`, `participation_type`, `registered_at`) essential for a
      history view — the domain `EventMeta` is a listing summary without registration context. Uses
      `safe_all_rows` for NULL-safety. 4 new deserialization tests cover full-row mapping, NULL
      `checked_in_at` → `None` (the no-show signal), missing-column defaults, and
      `skip_serializing_if` wire contract. All passing.)
- [x] New endpoint `GET /api/contacts/{email}/history` (protected) returning the event list.
      (Verified 2026-07-09: `contact_history_handler` in `worker/src/handlers/contacts.rs` — protected
      via `Extension(claims): Extension<Claims>`, takes `Path(email)`, normalizes email
      (trim+lowercase), returns `ContactHistoryResponse { email, events, total }`. Route registered
      at `worker/src/handlers/mod.rs` as `/contacts/{email}/history` (3 segments — no ambiguity with
      the existing 2-segment `/contacts/events`, `/contacts/stats`, etc.). D1-missing check follows
      the `audience_handler` idiom. Compiles clean.)
- [x] Document in a code comment that `contacts.events_joined` is **deprecated as a read path** and will be removed in a future migration. Write paths continue updating it for backward compat with any external consumer.
      (Verified 2026-07-09: crate-level doc comment at the top of `worker/src/db/contacts.rs` adds a
      "Source of truth: `attendees` table (not `contacts.events_joined`)" section explaining the
      deprecation. The `upsert_contact` doc comment expands on why the CSV column drifts (denormalized,
      overwrite-on-every-upsert, not queryable) and explicitly states the read path is deprecated while
      writes continue for backward compat. `list_contact_events` doc reinforces "This is the source of
      truth" with a back-reference to the crate doc.)
- [x] Note: full removal (deleting the column + write paths) is out of scope — logged as follow-up tech debt.
      (Verified 2026-07-09: documented in both the crate-level doc ("Full column removal is tracked as
      follow-up tech debt — out of scope for Plan 008") and the `upsert_contact` doc ("Full removal
      (dropping the column + these write paths) is out of scope for Plan 008 and is tracked as
      follow-up tech debt: it requires a migration to drop the column plus an audit of every upsert
      caller"). No column drop migration shipped; write paths untouched.)

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

- [x] `worker/tests/event_summary_flow.rs` — **superseded: exercised against
      local D1 instead (2026-09-04).** Same reasoning as the post-event box
      below — the handler is `#[worker::send]` axum over a D1 binding, so a
      host-target `tests/*.rs` could only re-implement it. Ran all five steps
      under `wrangler dev --local` against a seeded `completed` event
      (3 approved in-person attendees, 2 checked in, 1 claimed, 2 verified USDC
      deposits). Verified there are no `sheets::` calls anywhere in
      `handlers/events/summary.rs`, `db/event_summaries.rs` or `db/dashboard.rs`
      first, so nothing left the machine.

      **What held.** Step 3 froze correctly: registered 3, checked-in 2,
      claimed 1, deposited 2, no-show 1, 2 USDC deposited. `registered_count`
      counts `approval_status = 'approved'` only, so a seeded
      `post_event_registered` row was correctly excluded while
      `post_event_reg_count` picked it up. Step 4 un-verified a deposit and
      added a late check-in + late claim; step 5 returned **byte-identical**
      numbers. Freeze is durable. Also confirmed: an event still running returns
      a preview with `frozen: false` and persists **no** row; `POST
      /summary/freeze` on it is rejected 400; a manual re-freeze after the event
      ended does update the numbers (intended — it is an explicit organizer
      action) and leaves `recap_markdown` / `recap_published_at` intact, because
      `upsert_summary`'s `DO UPDATE` set omits the recap columns.

      **Defect — a read failure destroyed the snapshot (fixed, `893bd01`).**
      Step 4 of the handler matched the existing-freeze lookup on `Ok(Some(_))`
      and let `Err` fall through to step 5, which recomputes and
      `upsert_summary`s over the durable row. A transient D1 read failure would
      therefore replace the official record with post-drift numbers — silently,
      and unrecoverably, since the whole point of the freeze is that the source
      rows have since moved. Now returns 500 and leaves the row alone. **Not
      reproduced locally**: `get_summary` only errs when D1 itself fails, and a
      `SELECT` cannot be made to fail from SQLite (triggers do not fire on
      reads) while the write path still succeeds. Reasoned from the code, fixed
      because refusing is cheap and a destroyed snapshot is not recoverable.

      The mirror of it in the **public** recap (`handlers/public_event.rs`) was
      fixed in the same commit: a failed funnel read fell into a `_ =>` arm that
      published `registered_count: 0, checked_in_count: 0, claimed_count: 0` —
      indistinguishable from a real "nobody came", on a public page. It now
      propagates, agreeing with the `get_recap` call two lines above it.

      **Also fixed in `893bd01`:** `summary` / `recap` / `pr_pack` /
      `post_event_registration` each carried a byte-identical private
      `load_event` + `enforce_organizer`, with a comment in `pr_pack.rs`
      deferring extraction "only if a fourth consumer appears". It had. They now
      share `handlers/events/common.rs`, which also fixes the defect all four
      copies shared: `if let Ok(Some(_))` on both the KV and D1 reads, so a
      backend outage was reported to the organizer as *"event not found"* —
      the same masking `resolve_event_by_slug` carried before `734aa4b`.
- [x] `worker/tests/post_event_registration.rs` — **superseded: exercised against
      local D1 instead (2026-09-04), and it found two live defects.** The
      endpoint is `#[worker::send]` axum over a D1 binding, so a host-target
      `tests/*.rs` can only re-implement it, not run it. Ran the real handler
      under `wrangler dev --local` (see `.plans/020_sql_parameter_binding.md`
      for the harness) against a seeded `completed` event with
      `post_event_registration_open = 1`.

      **What held.** `registration_phase = 'post_event'` /
      `approval_status = 'post_event_registered'` are written correctly, and the
      capacity invariant holds *by construction* rather than by an explicit
      filter: `count_registered` selects `approval_status = 'approved'`
      (`db/dashboard.rs:57`), and post-event rows are never written to Sheets, so
      neither the dashboard nor the Sheets capacity check can see them.
      `post_event_registered` appears in exactly two files repo-wide — nothing
      else needs to exclude it.

      **Defect 1 — a repeat submission was silently discarded (fixed, `2f25910`).**
      `upsert_post_event_attendee` carried `ON CONFLICT (id) DO UPDATE`, but the
      caller mints a fresh `Uuid::now_v7()` per request, so that clause could
      never fire. The real conflict is on `idx_attendees_unique_event_email`
      (migration 0026, partial on `participation_type <> 'walkin'` — post-event
      rows use `retrospective`, so they are covered). Reproduced: second submission →
      `UNIQUE constraint failed: index 'idx_attendees_unique_event_email'`,
      swallowed by the handler's "non-fatal" warn → **HTTP 200 "Thanks!"** with a
      brand-new `attendee_id` matching no row, and the attendee row unchanged.
      A visitor who resubmitted **withdrawing marketing consent** was told
      "Thanks!" while `attendees.consent_marketing` stayed `1` — and
      `write_developer_data` did succeed, so `developer_profiles.consent_outreach`
      went to `0`. The two stores then disagreed on consent, with the permissive
      value surviving in `attendees`. Fixed by targeting the real index and
      `RETURNING id`. The `DO UPDATE` set deliberately omits `approval_status` /
      `participation_type` / `registration_phase`: the conflicting row may be a
      genuine pre-event in-person attendee, and refreshing their contact details
      is right while demoting them to a `retrospective` lead is not. Verified: repeat
      submission now updates the one row and returns its real id; a seeded
      `approved` / `in_person` / `pre_event` row keeps all three fields.

      **Defect 2 — a failed write reported success (fixed, `2f25910`).** Every D1
      write in this handler was warn-and-continue. That is defensible in
      `signup.rs`, where Sheets is the primary store and D1 a mirror — but this
      endpoint has **no** Sheets write, so a failed attendee insert loses the lead
      outright and still answers `200 {"message": "Thanks!..."}`. The attendee
      write is now fatal (contact/profile enrichment stays non-fatal). Verified
      with a `BEFORE UPDATE … RAISE(ABORT)` trigger on `attendees`: **500** with
      `"could not save your registration — please try again"` — no SQL text or JS
      stack in the response, detail stays in the log.

      A host-target regression test still cannot cover either defect; both are
      properties of the SQL against a live SQLite. Re-run the local harness when
      touching this path.
- [x] `worker/tests/pr_pack.rs` — **superseded: smoke-tested against local D1
      (2026-09-04).** `GET /api/events/{id}/pr-pack` returned all seven fields
      for a seeded fixture event; generation is a pure function over
      `EventConfig` already covered by 17 unit tests in `domain/src/pr_pack.rs`,
      so the only thing a host-target test could add is the handler's
      load-event + role check, which `events::common` now shares with three
      other endpoints.

      **Defect — the "Register here" link was wrong in both branches (fixed,
      `1c156d0`).** `registration_url` returned `claim_base_url` verbatim when
      set. That is a *claim* endpoint — `handlers/walkin.rs:346` builds
      `{claim_base_url}/{claim_token}` — so every generated tweet, blurb and
      email invited readers to a token-gated claim page rather than the
      registration page. When it was unset the function returned a root-relative
      `/e/{slug}`, which the doc comment described as "clickable"; it is not,
      the moment the copy is pasted into an email or a tweet, which is the only
      thing this feature produces. Now resolves in order: the organizer's
      external `link`, else `origin(claim_base_url) + /e/{slug}`, else the
      relative path. Verified live: the blurb went from
      `Register: https://bethere.app/claim` to
      `Register: https://bethere.app/e/freeze-test`.

### Manual

**Status 2026-09-04 (second pass):** all four remain open and all four are
genuinely blocked on a browser + a real event — the API side of each is now verified against local D1
(see the Integration notes and the Phase 3 acceptance list), so what is left is
specifically *rendering and copy*, not behaviour. Nothing here is owner-gated;
it just cannot be done from a shell.

- [ ] Run through Phase 1 UI on a real completed event (e.g. an old dev event in the DB). Verify funnel numbers match the live dashboard's last-known values.
- [ ] Run Phase 2 publish flow. Visit `/events/{slug}/recap` in incognito. Confirm sanitized payload.
- [ ] Run Phase 3 toggle + register flow. Verify a new row appears in `developer_profiles` with the post-event registrant's interests.
- [ ] Run Phase 4 generator on an upcoming event. Copy each field, paste into actual social/email, sanity-check readability.
      (The *generated text* was sanity-checked from a shell on 2026-09-04 via
      `domain/examples/pr_pack_preview.rs`, which found and fixed two defects —
      see Phase 4 above. What is left here is the paste-into-a-real-surface
      check: line wrapping, link unfurls, emoji rendering in a real client.)

### CI

- [x] New tests must be wired into the worker `pnpm test` + `cargo test` flow.
      **Reworded on completion (2026-09-04): there is no `pnpm test`, and this
      workstream added no new test *files*.** Both integration boxes above were
      closed by exercising the real handlers against local D1 rather than by
      adding host-target tests, so the only new automated coverage is unit
      tests inside existing crates (`domain/src/pr_pack.rs` is now 17 tests).
      Those run under `cargo test --workspace --locked`, which
      `.github/workflows/ci.yml` (job `build-test`) already executes on every
      push to `develop`/`main` and every PR. Verified locally at `cd49a11`:
      **477 tests, 0 failed** across `domain` (121 lib + 5 integration files),
      `worker` (210 lib + 5 integration files) and both doc-test targets.

      `pnpm test` does not exist and never did — `worker/package.json` defines
      only `test:e2e*` (Playwright). See the new box below for that gap.
- [x] No new clippy warnings on changed files (the wider 183-warning debt is
      documented elsewhere — plan 004 §7). Verified at `cd49a11`:
      `cargo clippy --workspace --locked --all-targets -- -D warnings` is
      **clean**, as is the frontend's separate gate
      (`cd frontend-leptos && cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings`).

      **Defect found by running the real CI invocation (fixed, `cd49a11`).**
      The gate this workstream had been using day to day —
      `cargo clippy -p event-checkin-worker --target wasm32-unknown-unknown -- -D warnings`
      — omits `--all-targets`, so it never lints `#[cfg(test)]` code. CI's
      invocation does. A `useless_vec` in `worker/src/handlers/contacts.rs:693`
      (introduced 2026-08-21 by `9388ddf`, i.e. **pre-existing**, not from this
      plan) therefore sat red on the branch: `cargo clippy --workspace
      --all-targets` failed with `could not compile event-checkin-worker (lib
      test)`. Any PR from this branch would have gone red on the `build-test`
      job. Use `--all-targets` when checking clippy locally.
- [x] **Closed 2026-09-04 — the Playwright e2e suite is now in CI.** `e2e/` holds
      5 smoke specs (`auth-guards`, `claim`, `landing`, `login`, `routes`; 95
      lines total) driven by `worker/playwright.config.ts`, whose `testDir` is
      `../e2e`. The config has no `webServer` block and nothing in
      `.github/workflows/ci.yml` ran them, so a frontend regression that still
      compiles — a renamed CSS hook, a dead route, a broken auth guard — only
      surfaced in a browser. `frontend-clippy` compiles the SPA but never runs it.

      New `e2e` job: trunk-build the SPA, then serve **everything from the
      worker** on one port rather than `trunk serve` + a proxy. The worker's
      `[assets]` block already points at `frontend-leptos/dist` with SPA
      fallback, so 8788 answers both the app and `/api` — the production
      topology. That matters for `claim.spec.ts`, which asserts an error state
      for a bad token: against the real handler it gets a genuine 404, not a
      proxy failure that happens to render the same way.

      `BASE_URL` overrides the config's `localhost:3001` default; the config's
      own `CI` branches (`retries: 2`, `workers: 1`, `forbidOnly`) apply
      unchanged. `trunk` and `wasm-bindgen-cli@0.2.118` come from
      `taiki-e/install-action` (`wasm-bindgen-cli` is a documented alias of
      `wasm-bindgen`); the exact wasm-bindgen version matters because the
      worker's `[build]` command in `wrangler.toml` shells out to it directly
      and a mismatch fails at bindgen, not at compile.

      **Verified locally before wiring**, since a green YAML file proves
      nothing: built the SPA with trunk, ran `wrangler dev --local` with
      `worker/.dev.vars` moved aside to simulate a secretless runner, and ran
      `BASE_URL=http://localhost:8788 npx playwright test` → **13 passed in
      3.6s**. Also grepped the dev log for `sheets::` afterwards → **0 hits**,
      confirming the job cannot reach a live Google Sheet (see plan 020 §4.4 —
      `--local` does not sandbox Sheets, so this is not free by default; it
      holds here only because every spec is unauthenticated and no role
      resolution fires).

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
      (Corrected 2026-07-09, updated 2026-07-16: only Phase 1 (`48d25b1`) is deployed to production; Phase 2/3/4 are now merged to `develop` via PR #20 (originally on `feature/event_recap` `9549532`, rebased). Deploy to production pending. All builds were checked locally and are under the 3 MiB hard limit. The guard script is the documented pre-deploy step.)
- [~] Optional: add as a CI gate (separate from this plan's scope — flagged for plan 005's harness work).
      (Deferred to plan 005's CI harness work — not implemented as a CI gate yet. Script exists; CI wiring pending.)
- [x] On any deploy where the guard fails: trim dependencies, or split into a second Worker via Service Bindings (e.g. extract escrow/indexer handlers). Do **not** raise `SIZE_BUDGET_MIB` without a deliberate decision.
      (Corrected 2026-07-09: no guard failure has occurred — Phase 1 deployed within budget;
      Phase 2 build checked locally pending deploy. Policy documented; no `SIZE_BUDGET_MIB` raise needed.)

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
- [x] Same branch / PR flow.
      (Verified 2026-07-09: committed at `6b11137 feat(event-lifecycle): Plan 008 Phase 3 — post-event registration (lead capture)` (2026-07-09 13:08 +0700) on `feature/event_recap`. Phase 3 code is live on the branch alongside Phase 1+2.)
- [~] Validate: register as a brand-new email → confirm `developer_profiles` row appears with expected fields → confirm `approval_status = 'post_event_registered'` excludes from capacity / check-in queries.
      (Code-trace verified: `upsert_post_event_attendee` sets approval_status='post_event_registered' +
      registration_phase='post_event'; existing capacity/check-in queries filter on approval_status='approved'
      and registration_phase='pre_event', so post-event rows are naturally excluded. The staging
      environment is available; the remaining validation is the browser journey after the gateway
      deploy, using an isolated completed event and no raw D1 writes.)

**Phase 4**

- [x] Generator + endpoint + frontend page.
      (Verified 2026-07-09: delivered at `63270ac feat(event-lifecycle): Plan 008 Phase 4 — PR pack generator` (2026-07-09 08:53 +0700). Generator pure-fns in `domain/src/pr_pack.rs` (471 lines, 14 unit tests) + `domain/src/lib.rs` re-export; endpoint `worker/src/handlers/events/pr_pack.rs` (99 lines) + route `GET /api/events/{id}/pr-pack` in `handlers/mod.rs`; frontend page `frontend-leptos/src/pages/pr_pack.rs` (286 lines) + `api/event.rs` client + `lib.rs` route + `pages/mod.rs`. §3.4 sub-item checkboxes already `[x]` for all 4 details.)
- [x] Same branch / PR flow.
      (Verified 2026-07-09: committed at `63270ac` on `feature/event_recap`, same feature-branch flow as Phases 1–3. No separate PR — Phase 4 landed directly on the feature branch per the project's single-branch convention for this plan.)
- [~] Validate: copy a generated social post → post to a test account → confirm readability.
      **Readability half done 2026-09-04 (`7f3c0bf`); posting is still manual.**
      `domain/examples/pr_pack_preview.rs` (`cargo run -p event-checkin-domain
      --example pr_pack_preview`) prints every field of a full pack for two
      fixtures modelled on real events from `.plans/018` §8 — the recurring
      hybrid with a THB 500 deposit, and a no-deposit online session. Reading the
      output found **two copy defects that all 17 existing unit tests missed**:

      1. **`deposit_terms` advertised "$0" on every production event.**
         `format_usdc(event.deposit_amount_usdc)` was emitted unconditionally,
         and production deposits are THB-only (`deposit_amount_usdc == 0`), so a
         real event's pack read *"A deposit is required to secure your spot: $0
         (or 500 THB via PromptPay)"* — which a reader parses as "this is free".
         The amount clause now branches on which currencies are actually set:
         both, USDC-only, THB-only, or neither (deposits enabled with no amount
         is a misconfiguration, so the copy states the requirement without
         inventing a price). Now reads *"…your spot: 500 THB via PromptPay."*
      2. **An empty tagline left a blank line mid-post.** `social_post` always
         rendered the tagline line, so a tagline-less event produced
         `🗓️ … @ Online\n\nhttps://…`. The line is now omitted, not emptied.

      The existing suite passed throughout because it only ever exercised the
      both-currencies, tagline-present fixture. Four regression tests added
      (THB-only, USDC-only, no-amount, no-tagline); the suite is 17 → 21.

      **Still manual:** actually posting to a test account. That is an
      outward-facing publish, not something to do unprompted.

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

All API-level criteria below were exercised against **local D1** under
`wrangler dev --local` on 2026-09-04 (harness: `.plans/020_sql_parameter_binding.md`).
The two boxes that need a *browser* rather than an API call are marked `[~]` and
belong to the Manual section.

- [x] An organizer can toggle post-event registration on a Completed event, with an optional deadline.
      (Verified 2026-09-04 against local D1, `PUT /api/events/{id}/post-event-registration`
      as the organizer: open + future deadline → `200 {"open":true,"until_ms":…}` and the
      `events` row carries both; open + past deadline → **400** `"deadline … must be in the
      future"`; open on a KV-cold `active` event → **400** `"can only be opened for completed
      events (current status: active)"`; close → `200` and the deadline is cleared to `NULL`.
      Closing is deliberately allowed in any status. Frontend toggle control itself is UI —
      see Manual.)

      **Harness note, not a defect.** `events::common::load_event` is KV-first, so the
      status gate reads the *cached* config. Editing `events.status` straight in D1 (as a
      seed script does) is invisible to this handler until KV is refreshed — every
      in-app status change mirrors to KV, so this only bites out-of-band writes. Seed a
      fresh event id when you need a KV-cold read.
- [~] A signed-in user can register post-event; the form captures developer-profile fields.
      (API half verified 2026-09-04 — see the `post_event_registration.rs` integration note
      above, which also found and fixed two defects. The remaining browser check starts at
      `/e/{slug}` and must confirm the completed-event gateway, archive link, login return,
      form rendering, and submission.)
- [x] The new `attendees` row has `registration_phase = 'post_event'` and `approval_status = 'post_event_registered'`.
      (Verified against local D1 2026-09-04 — integration note above.)
- [x] The registrant's `developer_profiles` row is upserted with submitted fields.
      (Verified against local D1 2026-09-04 — integration note above; the consent-divergence
      defect found there is fixed in `2f25910`.)
- [x] Post-event registrants are NOT counted in capacity, check-in, or normal-attendance queries.
      (Verified against local D1 2026-09-04. Holds *by construction*: `count_registered`
      selects `approval_status = 'approved'` (`db/dashboard.rs:57`) and post-event rows are
      never written to Sheets, so neither the dashboard nor the Sheets capacity check can
      see them. `post_event_registered` appears in exactly two files repo-wide.)
- [x] The summary page's "post-event registrations" tile increments correctly.
      (Verified against local D1 2026-09-04 during the freeze run: a seeded
      `post_event_registered` row was excluded from `registered_count` and counted by
      `post_event_reg_count` in the same snapshot. The tile renders that field.)
- [x] When the deadline passes (or the toggle is flipped off), the public form 404s/410s.
      (Verified 2026-09-04 against local D1 on `POST /api/public/event/{slug}/register-post-event`:
      past deadline → **410** `"post-event registration for this event has closed"`;
      toggle off → **409** `"post-event registration is not open for this event"`.)

      **Defect — the recap page kept advertising a closed form (fixed, `3cee479`).**
      `GET /api/public/event/{slug}/recap` returned the raw
      `post_event_registration_open` flag, and `pages/public/event_recap.rs:304` renders the
      "Missed this event? / Join the community" CTA straight from it. The flag stays `true`
      after the deadline lapses — only the *submit* endpoint checks the deadline — so a
      visitor arriving one minute late was invited to sign in with Google, fill the whole
      form, and only then be told `410 Gone`. The public payload now reports whether
      registration **accepts a submission now**, computed on the server clock (the same
      clock the submit endpoint compares against) rather than the browser's.

      The comparison itself is now shared: `EventConfig::post_event_registration_accepting`
      / `post_event_registration_deadline_passed` in `domain/src/models/event.rs`, used by
      both the payload and `register::post_event` — so the CTA cannot disappear while the
      endpoint still accepts, or vice versa. The two error branches stay separate because
      the codes differ (never-opened is a 409, opened-then-lapsed a 410). 4 new unit tests
      cover the `now_ms >= until` boundary, the `None` = indefinite case, and toggle-off
      beating a future deadline. Verified end-to-end: past deadline → payload flag `false`
      *and* submit `410`; future deadline → `true` *and* the form accepts; toggle off →
      `false` *and* `409`.

      **Out of scope, flagged not fixed:** `hard_delete_event` removes the KV entry and the
      `events` row only (`event_store::write::index::sync_delete_event_from_d1` →
      `db::events::delete_event`). `event_summaries` has no FK to `events` and no cascade,
      so permanently deleting an event leaves its frozen funnel snapshot behind — observed
      while cleaning up the fixtures above. `attendees` and the audit rows survive too, which
      may well be intentional for record-keeping; the orphan summary probably is not.

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
      Deployed to production D1 successfully with Phase 1; Phase 2/3/4 merged to `develop` 2026-07-16 via PR #20 (originally on `feature/event_recap`, rebased); production D1 deploy of Phase 2+ still pending.)
- [x] No new clippy warnings introduced (existing 183-warning debt is documented separately).
      (Verified 2026-07-08: worker + domain clippy clean with `-D warnings`. The 183-warning debt
      is in the out-of-workspace `frontend-leptos` crate, built via trunk not clippy-gated by CI.)
- [x] `bash scripts/check_size.sh` passes (worker gzip ≤ 2.5 MiB) on the merged Phase 1 build. Baseline before plan 008: **1.446 MiB** (authoritative wrangler measurement, captured 2026-06-23). If the guard fails, do not ship — investigate the dependency/code responsible before raising the budget.
      (Corrected 2026-07-09, updated 2026-07-16: only Phase 1 (`48d25b1`) is deployed to production; Phase 2/3/4 are now merged to `develop` via PR #20 (originally on `feature/event_recap` `9549532`, rebased). Production deploy of Phase 2+ still pending. Size guard run locally on the Phase 2 build; script exists at `worker/scripts/check_size.sh`.)

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
