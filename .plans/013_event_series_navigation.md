# Plan 013 — Event Series Navigation (Related Events on Ticket Page)

> Spun out of the 2026-06-26 brainstorming session. First slice of a broader
> attendee-facing campaign/series experience. Builds on existing backend
> campaign infrastructure (issue #051, all phases done) — this plan only
> **surfaces** that data to attendees; no new data model.

## Context

- **Campaigns fully built** (admin-side): `campaigns`, `campaign_events`
  (with `sequence_order`), `developer_campaign_progress`. NFT rewards,
  leaderboard, progress tracking all done (#051).
- **Attendees see none of it.** Zero `campaign` references in
  `frontend-leptos/src/`. `campaigns_page.rs` is admin-only.
- **Prod has clear series but no campaign rows**: "Road to Mainnet" (#1, #3, #4)
  and "Solana in Latent Space" (Part 1, Part 2) exist as events but aren't
  grouped. `campaigns` × 0, `campaign_events` × 0 in prod.
- `campaign_events.sequence_order` is literally a playlist ordering — the data
  model for "prev/next" and "playlist" already exists.

## Goal (this plan)

The smallest high-impact unit: **show related events (prev/next + series badge)
on the ticket page.** This:
- Reuses existing data (no migration).
- Is dormant until a campaign is created (graceful no-op).
- Establishes the public read endpoint that the future playlist page + campaign
  progress display will also use.

## Non-goals (deferred)

- Public campaign/playlist page (`/campaign/{slug}`) — next slice, same endpoint.
- Attendee campaign progress display — needs authed variant of the endpoint.
- Interactive discussion — independent feature, separate plan.
- Creating prod campaign data — that's an organizer decision via admin UI.

## Design

### Foundation: one public read endpoint

`GET /api/public/event-series/{event_id}` — no auth, cached 120s.

Returns the campaign containing this event (if any) with ordered events and
the prev/next neighbors:

```json
{
  "campaign": {
    "id": "road-to-mainnet",
    "title": "The Road to Mainnet",
    "description": "..."
  },
  "events": [
    { "event_id": "...rtm-1", "name": "...#1", "slug": "...", "event_start_ms": 1780000000000, "sequence_order": 0 },
    { "event_id": "...rtm-3", "name": "...#3", "slug": "...", "event_start_ms": 1781935200000, "sequence_order": 1 }
  ],
  "current_index": 1,
  "previous": { "event_id": "...", "name": "...", "slug": "..." } | null,
  "next":     { "event_id": "...", "name": "...", "slug": "..." } | null
}
```

When the event has no campaign → `404` (frontend treats 404 as "hide section").
When the campaign has only 1 event → still returns it (current_index 0, no
prev/next) so the "Part of {Series}" badge can show.

### Why by event_id (not campaign_id)

The ticket page knows its `event_id` (query param), not its campaign. A
reverse lookup `campaign_events WHERE event_id = ?` is indexed
(`idx_campaign_events_event`) and cheap.

### DB layer (new, reuses existing patterns)

In `worker/src/db/campaigns.rs`:
- `get_campaign_for_event(db, event_id) -> Option<CampaignRow>` — reverse lookup.
- `list_campaign_event_summaries(db, campaign_id) -> Vec<EventSeriesEntry>` —
  joins `campaign_events` + `events`, returns only public-facing fields
  (id, name, slug, event_start_ms, sequence_order). Ordered by sequence_order.

Follows the existing raw-JsFuture + JSON.stringify pattern (avoid
`.first::<T>()` JsValue(null) crash — see `db/events.rs`).

### Handler (new file)

`worker/src/handlers/event_series.rs` — `get_event_series(event_id)`.
Reads campaign via reverse lookup, lists events, computes current_index/prev/next.

### Frontend

- `frontend-leptos/src/api/event_series.rs` — `get_event_series(event_id)`
  returning the typed response (or `None` on 404).
- `frontend-leptos/src/pages/ticket/series_nav.rs` — `SeriesNav` component.
  Shows "Part of {Campaign}" badge + prev/next cards (links to neighbor
  `public_event` pages). Hidden when no campaign.
- Inject `SeriesNav` into `InPersonView` (after community links, before footer)
  and `OnlineView`.

### Wiring

- Route in `handlers/mod.rs` under `public_events_detail` group (120s cache).
- API client module added to `api/mod.rs`.
- `ticket/mod.rs` exports `series_nav`.

## Files to touch

| File | Change |
|---|---|
| `worker/src/db/campaigns.rs` | + `get_campaign_for_event`, `list_campaign_event_summaries`, `EventSeriesEntry` struct |
| `worker/src/handlers/event_series.rs` | NEW — `get_event_series` handler |
| `worker/src/handlers/mod.rs` | `pub mod event_series;` + route registration |
| `frontend-leptos/src/api/event_series.rs` | NEW — API client |
| `frontend-leptos/src/api/mod.rs` | `pub mod event_series;` |
| `frontend-leptos/src/pages/ticket/series_nav.rs` | NEW — `SeriesNav` component |
| `frontend-leptos/src/pages/ticket/mod.rs` | `mod series_nav;` + re-export |
| `frontend-leptos/src/pages/ticket/in_person_view.rs` | inject `<SeriesNav ... />` |
| `frontend-leptos/src/pages/ticket/online_view.rs` | inject `<SeriesNav ... />` |

## Tests

- **Worker** (inline `#[cfg(test)] mod tests` in `worker/src/db/campaigns.rs`,
  following the codebase convention used by `db/dashboard.rs` & `db/event_summaries.rs`):
  the prev/next logic was extracted into a pure `compute_series_neighbors(events, id)`
  function so edge cases are unit-testable without a D1 mock — covers empty list,
  orphan (linked but missing), single event, first, last, middle, index-vs-sequence_order,
  full payload clone, and duplicate-id first-match. (9 tests, all passing.)
  The D1-dependent `get_campaign_for_event` / `list_campaign_event_summaries` are
  exercised manually against real D1 — there's no in-process D1 mock harness.
- **Manual**: create a campaign via admin UI linking 2+ events, open a ticket,
  confirm the section appears with correct neighbors.

## Status

**Implementation complete** (2026-06-26). Backend, frontend, CSS, and unit tests
all land in this slice. Remaining: commit → deploy → create prod campaign data.

## Rollout

1. Land code + tests on `develop`. No data change, no migration.
2. Deploy.
3. Create a "Road to Mainnet" campaign via admin UI, link the 3 events.
4. Verify the ticket page shows the section.
5. (Next slice) build the public playlist page reusing the same endpoint.

## Open decisions (applied defaults)

1. **404 when no campaign** (vs. 200 with `campaign: null`) — chose 404 so the
   frontend cache layer treats it as a clean miss and the component hides
   without a null-check dance. Reversible.
2. **Links go to `/e/{slug}` (public_event page)**, not neighbor tickets — the
   attendee isn't registered for the neighbor, so the ticket page would 404.
3. **No auth on the endpoint** — campaign structure is public (like event
   listings). Per-event registration status stays on the ticket page itself.
