# Plan 008 — Event Lifecycle: Summary, Recap, Post-Event Registration, PR Generator

> **Status**: DRAFT — not started
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

### 3.0 Migration 0019 — schema foundation

New file: `worker/migrations/0019_event_summaries_post_event.sql`

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

- [ ] Add migration file.
- [ ] Verify idempotency (`IF NOT EXISTS` on table + index; `ADD COLUMN` is one-shot — relies on `d1_migrations` tracker).
- [ ] Document the `breakdown_json` shape inline as a comment (e.g. `{"by_format": {"in_person": 20, "online": 8}, "top_roles": [...]}`) even though v1 leaves it as `{}`.

### 3.1 Phase 1 — Post-Event Summary (internal record)

#### 3.1.1 Domain types

New file: `domain/src/models/event_summary.rs`

- [ ] `pub struct EventSummary` mirroring the table columns, with `#[serde(...)]` matching API conventions.
- [ ] `pub struct FunnelSnapshot` (registered, deposited, checked_in, no_show, claimed, refunded, post_event_reg) — embedded in `EventSummary` for the response payload.
- [ ] `pub struct FinancialSnapshot` (usdc_deposited_total, usdc_refunded_total, thb_deposited_total, thb_refunded_total) — embedded similarly.
- [ ] Re-export from `domain/src/models/mod.rs`.

#### 3.1.2 DB layer

New file: `worker/src/db/event_summaries.rs`

- [ ] `pub async fn get_summary(db, event_id) -> Result<Option<EventSummaryRow>, String>` — raw row read.
- [ ] `pub async fn upsert_summary(db, summary: &EventSummary) -> Result<(), String>` — write freeze.
- [ ] `pub async fn compute_snapshot(db, event_id) -> Result<EventSummary, String>` — **the core aggregation**. Reuse existing primitives where possible:
  - `count_registered(db, event_id)` from `db/dashboard.rs` (approved attendees with `registration_phase = 'pre_event'`)
  - `count_checked_in(db, event_id)` from `db/dashboard.rs`
  - `count_claims_minted(db, event_id)` from `db/dashboard.rs`
  - `verified_usdc_summary(db, event_id)` from `db/dashboard.rs` for `usdc_deposited_total`
  - New helpers in this file for: `no_show_count` (registered − checked_in), `refunded_count` + `usdc_refunded_total` (from `deposit_statuses` + `thb_deposits` where refund markers set), `thb_deposited_total` (from `thb_deposits`), `post_event_reg_count` (from `attendees` where `registration_phase = 'post_event'`).
- [ ] Follow the NULL-safe raw-JS-interop pattern from `db/dashboard.rs` (avoid `.first::<T>()` panics on `JsValue(null)`).
- [ ] Follow the `sqlx::raw_sql` style note from the handover rules (parameter binding via D1's `bind_refs` is fine — that note applies to sqlx/pg, not Cloudflare D1's JS-binding API).

#### 3.1.3 Handler

New file: `worker/src/handlers/events/summary.rs`

Two endpoints, both protected (organizer+ only, resolved via `auth::resolve_user_role`):

- [ ] `GET /api/events/{id}/summary` — **lazy freeze**:
  1. Load event config (KV → D1 fallback, existing pattern).
  2. Role check: reject if Staff.
  3. If `event.status == Draft` → return 409 "event not yet active".
  4. If existing frozen row in `event_summaries` → return it.
  5. If `now_ms < event_end_ms` → return live-computed snapshot with `frozen_at: null` and a `"frozen": false` flag (organizer can preview the would-be freeze).
  6. If `now_ms >= event_end_ms` and no frozen row → call `compute_snapshot` + `upsert_summary`, then return with `"frozen": true`. Write an audit entry (`AuditAction::EventSummaryFrozen`).
- [ ] `POST /api/events/{id}/summary/freeze` — manual trigger:
  1. Same role check.
  2. Reject if event is still `Active` and `now_ms < event_end_ms` (cannot freeze early — would mislead). Allow only if `event.status == Completed` or `now_ms >= event_end_ms`.
  3. Compute + upsert + audit. Return frozen summary.
- [ ] Add new `AuditAction::EventSummaryFrozen` variant to `worker/src/audit_store.rs` + the `FromStr`/serde impls used by `audit.rs::get_event_audit`.

#### 3.1.4 Route wiring

- [ ] In `worker/src/handlers/events/mod.rs`: add `pub mod summary;` + re-exports.
- [ ] In `worker/src/handlers/mod.rs::routes()` (protected group, ~L261-283 block):
  ```rust
  .route("/events/{id}/summary", get(events::get_event_summary))
  .route("/events/{id}/summary/freeze", post(events::freeze_event_summary))
  ```

#### 3.1.5 Frontend — organizer summary view

New file: `frontend-leptos/src/pages/organizer/event_summary.rs`

- [ ] Route: `/events/{id}/summary` (protected — redirect to login if no JWT).
- [ ] Sections:
  - **Header**: event name, date range, status badge, "Frozen at {timestamp}" or "Live preview (not yet frozen)" badge.
  - **Funnel tiles**: registered → deposited → checked-in → claimed, with conversion percentages. No-show count + rate. Post-event registration count (always 0 in v1 of Phase 1; populated by Phase 3).
  - **Financials**: USDC deposited / refunded (atomic → human via existing `format_usdc`), THB deposited / refunded.
  - **Freeze button**: shown only when `frozen == false` and `now_ms >= event_end_ms`. Calls `POST /summary/freeze`. Confirms with a dialog ("This snapshot is permanent — later refunds will not change these numbers.").
  - **Audit trail snippet**: last 10 entries for this event (reuse existing `/audit` endpoint).
- [ ] Link from the existing organizer dashboard ("View Summary" button per event row).

### 3.2 Phase 2 — Public Recap + Past Events Listing

#### 3.2.1 Recap authoring (organizer)

New file: `worker/src/handlers/events/recap.rs`

- [ ] `PUT /api/events/{id}/recap` (protected, organizer+):
  - Body: `{ recap_markdown: String, recap_image_url: String, publish: bool }`.
  - Validates: markdown ≤ 16KB; image_url must be https if non-empty.
  - Ensures a frozen `event_summaries` row exists (refuses to publish a recap for an event with no frozen summary — recaps without numbers are misleading). If none, returns 409 with a helpful message ("Freeze the summary first").
  - Updates `event_summaries.recap_markdown`, `recap_image_url`, `recap_published_at` (set to now if `publish=true`, null if false).
  - Mirrors `recap_published` flag on the `events` row (denormalized for cheap public-listing query).
  - Audit: `AuditAction::EventRecapPublished` / `EventRecapUnpublished`.
- [ ] `GET /api/events/{id}/recap` (protected) — returns draft recap to the organizer (even if unpublished).

#### 3.2.2 Public recap + past events listing

Extend `worker/src/handlers/public_event.rs`:

- [ ] `GET /api/public/events/past` — list `status == Completed AND recap_published == 1` events, sanitized (same field exclusion as `list_public_events`). Sorted by `event_end_ms DESC`. Cache 60s.
- [ ] `GET /api/public/event/{slug}/recap` — returns `{ event_meta, recap_markdown, recap_image_url, frozen_at, funnel: { registered, deposited, checked_in } }` for a published recap. Sensitive financials (refunded totals, no-show counts) are **excluded** from the public payload — only headline funnel + recap content. Cache 120s.
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

- [ ] New page `frontend-leptos/src/pages/public/past_events.rs` — grid of completed events with published recaps. Each card: name, date, tagline, location, attendance count, "Read recap" CTA.
- [ ] New page `frontend-leptos/src/pages/public/event_recap.rs` — the recap view: hero image, event name + date, recap markdown (rendered), funnel headline ("X developers gathered, Y checked in"), link back to past-events listing.
- [ ] Link the landing page's "Past Events" nav entry to `/past-events`.
- [ ] Link each past-event card to `/events/{slug}/recap`.

#### 3.2.5 Frontend — organizer recap editor

- [ ] Extend `event_summary.rs` page (from 3.1.5) with a "Recap" tab.
- [ ] Markdown editor (textarea + live preview via existing markdown renderer, or pull in `pulldown-cmark` if not already in deps — check `frontend-leptos/Cargo.toml`).
- [ ] Image URL field (organizer pastes an R2/Cloudflare Images URL — no upload flow in v1).
- [ ] "Save Draft" + "Publish" buttons. Publish confirms ("Public immediately at /events/{slug}/recap").

### 3.3 Phase 3 — Post-Event Registration (lead capture)

#### 3.3.1 Backend toggle

New file: `worker/src/handlers/events/post_event_registration.rs`

- [ ] `PUT /api/events/{id}/post-event-registration` (protected, organizer+):
  - Body: `{ open: bool, until_ms: Option<i64> }`.
  - Validates: `event.status == Completed` (cannot open post-event reg for a not-yet-started event — that's just normal registration). If `open == true` and `until_ms` is `Some`, require `until_ms > now_ms`.
  - Updates `events.post_event_registration_open` + `post_event_registration_until_ms`.
  - Audit: `AuditAction::PostEventRegistrationToggled`.

#### 3.3.2 Public registration endpoint

Extend `worker/src/handlers/register.rs`:

- [ ] `POST /api/public/event/{slug}/register-post-event` (public, JWT-required for spam resistance — anon users must sign in with Google first, same as normal registration):
  - Loads event by slug. Rejects 404 if not found, 409 if `status != Completed`, 409 if `post_event_registration_open != 1`, 410 if `until_ms` is set and `now_ms >= until_ms`.
  - Accepts a subset of `RegisterRequest` (name, contact_channel, contact_handle, consent flags, all developer profile fields, `profile_fields` map). Ignores `participation_type`, `deposit_agreed`, `photo_consent_given` (not relevant — they're not attending).
  - Creates `attendees` row with:
    - `registration_phase = 'post_event'`
    - `approval_status = 'post_event_registered'` (new value — naturally excluded from existing `approval_status = 'approved'` queries)
    - `participation_type = 'online'` (placeholder; not used for capacity)
    - `checked_in_at = NULL`, no `claim_token` (no NFT to claim)
  - Upserts `contacts` and `developer_profiles` exactly like normal registration (reuse existing helpers).
  - Returns `{ attendee_id, message: "Thanks! We'll notify you about future events." }`.

#### 3.3.3 Route wiring

```rust
// public group (JWT still required — wired into the auth-required public sub-router)
.route("/public/event/{slug}/register-post-event", post(public_register::register_post_event))

// protected group
.route("/events/{id}/post-event-registration", put(events::put_post_event_registration))
```

#### 3.3.4 Frontend — post-event registration form

- [ ] Extend `event_recap.rs` page (from 3.2.4): if `event.post_event_registration_open == true`, render a "Missed this event? Join the community" CTA below the recap.
- [ ] New component `frontend-leptos/src/pages/public/post_event_register.rs` — form mirroring the normal registration form but stripped of deposit/participation fields. Shows developer-profile questions (experience_level, tech_stack, interests, etc.) — this is the **primary value** of post-event reg.
- [ ] Submit success state: "You're on the list. We'll email you about the next event."

### 3.4 Phase 4 — Upcoming PR Generator

#### 3.4.1 Backend

New file: `worker/src/handlers/events/pr_pack.rs`

- [ ] `GET /api/events/{id}/pr-pack` (protected, organizer+):
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
- [ ] No persistence — generated on every call. Deterministic.

#### 3.4.2 Frontend — PR pack preview

- [ ] New page `frontend-leptos/src/pages/organizer/pr_pack.rs` — route `/events/{id}/pr-pack`.
- [ ] One card per generated field. Each card has copy-to-clipboard.
- [ ] "Regenerate" button (re-fetches — useful after editing the event config).
- [ ] "Open event editor" link to make tweaking source fields easy.
- [ ] Read-only — no editing here. Edit the event config, regenerate.

### 3.5 `events_joined` derivation (read-side fix)

The `contacts.events_joined` CSV (`worker/src/db/contacts.rs#L22-31`) is overwritten on every upsert and not queryable. The source of truth for "which events did this contact attend" is the `attendees` table (one row per event per email, scoped by `registration_phase = 'pre_event' AND approval_status IN ('approved')`).

- [ ] New helper `pub async fn list_contact_events(db, email) -> Result<Vec<EventMeta>, String>` in `worker/src/db/contacts.rs` — joins `attendees` → `events` for the email.
- [ ] New endpoint `GET /api/contacts/{email}/history` (protected) returning the event list.
- [ ] Document in a code comment that `contacts.events_joined` is **deprecated as a read path** and will be removed in a future migration. Write paths continue updating it for backward compat with any external consumer.
- [ ] Note: full removal (deleting the column + write paths) is out of scope — logged as follow-up tech debt.

---

## 4. Testing

### Unit

- [ ] `domain/src/pr_pack.rs` — snapshot-style tests for each generator function (input `EventConfig` fixture → expected output string). Cover: missing tagline, missing location, multi-organizer CSV, deposit disabled, very long name (truncation behavior).
- [ ] `domain/src/models/event_summary.rs` — serde round-trip tests (mirror the pattern in `frontend-leptos/tests/serde_contract.rs`).
- [ ] `worker/src/db/event_summaries.rs::compute_snapshot` — test against a fixture D1 with known attendee/deposit rows. Assert exact counts + totals. This is the most important unit test in the plan.

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

- [ ] `worker/scripts/check_size.sh` committed (already created alongside this plan).
- [ ] Wire into Phase 1 rollout: run `bash scripts/check_size.sh` before every `bash deploy.sh`. Document in the per-phase checklist below.
- [ ] Optional: add as a CI gate (separate from this plan's scope — flagged for plan 005's harness work).
- [ ] On any deploy where the guard fails: trim dependencies, or split into a second Worker via Service Bindings (e.g. extract escrow/indexer handlers). Do **not** raise `SIZE_BUDGET_MIB` without a deliberate decision.

### Per-phase rollout

**Phase 1**

- [ ] Migration 0019 (all sections — schema is shared across phases).
- [ ] Domain + db + handler + route.
- [ ] Frontend summary page.
- [ ] Commit on `develop/feature/008_event_lifecycle_summary_pr`.
- [ ] PR review.
- [ ] Merge → `develop` → deploy production worker + frontend.
- [ ] Validate against one real completed event.

**Phase 2**

- [ ] Recap authoring + public endpoints + frontend pages.
- [ ] Same branch / PR flow.
- [ ] Validate: organizer publishes recap → incognito user sees it within cache TTL.

**Phase 3**

- [ ] Toggle + register + frontend form.
- [ ] Same branch / PR flow.
- [ ] Validate: register as a brand-new email → confirm `developer_profiles` row appears with expected fields → confirm `approval_status = 'post_event_registered'` excludes from capacity / check-in queries.

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

- `worker/migrations/0019_event_summaries_post_event.sql` — **new**

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

- [ ] After an event ends, an organizer can visit `/events/{id}/summary` and see a frozen snapshot of the funnel (registered, deposited, checked-in, no-show, claimed) and financials (USDC + THB deposited/refunded totals).
- [ ] The first visit after `event_end_ms` triggers an automatic freeze; subsequent visits return the same numbers even if underlying data changes (verified by the integration test in §4).
- [ ] An organizer can manually trigger freeze via the UI button (only enabled when `now_ms >= event_end_ms`).
- [ ] Staff role is blocked from the endpoint (403).
- [ ] An audit entry is written on every freeze.

### Phase 2 — Public Recap

- [ ] An organizer can author recap markdown + image URL via the summary page's Recap tab.
- [ ] On publish, the event appears in `GET /api/public/events/past`.
- [ ] On publish, `/events/{slug}/recap` renders the recap publicly (incognito-verifiable).
- [ ] Unpublish removes it from both surfaces.
- [ ] Sensitive fields (refunded totals, no-show counts, financials) are NOT in the public payload.

### Phase 3 — Post-Event Registration

- [ ] An organizer can toggle post-event registration on a Completed event, with an optional deadline.
- [ ] A signed-in user can register post-event; the form captures developer-profile fields.
- [ ] The new `attendees` row has `registration_phase = 'post_event'` and `approval_status = 'post_event_registered'`.
- [ ] The registrant's `developer_profiles` row is upserted with submitted fields.
- [ ] Post-event registrants are NOT counted in capacity, check-in, or normal-attendance queries.
- [ ] The summary page's "post-event registrations" tile increments correctly.
- [ ] When the deadline passes (or the toggle is flipped off), the public form 404s/410s.

### Phase 4 — PR Pack

- [ ] An organizer can visit `/events/{id}/pr-pack` for any event and see generated fields (headline, short_blurb, social_post, calendar_text, email_snippet, deposit_terms, organizers).
- [ ] Each field has copy-to-clipboard.
- [ ] Editing the event config and regenerating reflects the changes.
- [ ] Generation is deterministic — no external API calls.

### Cross-cutting

- [ ] `cargo check` + `cargo clippy` on changed files = clean.
- [ ] `pnpm test` + `cargo test` green.
- [ ] Migration 0019 applies cleanly on a DB with prior migrations 0001–0018.
- [ ] No new clippy warnings introduced (existing 183-warning debt is documented separately).
- [ ] `bash scripts/check_size.sh` passes (worker gzip ≤ 2.5 MiB) on the merged Phase 1 build. Baseline before plan 008: **1.446 MiB** (authoritative wrangler measurement, captured 2026-06-23). If the guard fails, do not ship — investigate the dependency/code responsible before raising the budget.

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
