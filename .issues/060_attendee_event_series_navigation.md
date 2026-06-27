# 060 — Attendee Event Series Navigation (related events on ticket page)

> First attendee-facing slice of the campaign/series experience. Surfaces the
> **existing** campaign infrastructure (#051, all backend phases done) that was
> previously invisible to attendees. Built as Plan 013 (`.plans/013`).
> **Shipped, deployed, and verified.** Dormant until an organizer creates a
> campaign.

## Status
- ✅ **Implemented** — commit `b3e923d` on `develop`, pushed.
- ✅ **Deployed** — prod version `aba9161c`, 100% traffic (2026-06-26T16:05Z).
  Supersedes the prior `085fb790` deploy. Tier A (#058 hygiene) + Tier B 3.2
  (#059 write-path unification) are now also live (they were in `develop` but
  the previous prod deploy predated them).
- 🔄 **Superseded** — `aba9161c` was itself superseded by the Handover 121 deploy
  (2026-06-27, `main` → `b432ac5`, frontend bundle `d334df8c0d54958b`). 060's code
  (`b3e923d`) remains live — it's an ancestor of `main` at `b432ac5`. The newer
  deploy was a superset (Plan 014 Phase 1.7 wire format + Plan 008 endpoint), not a
  regression of 060. Exact Cloudflare version ID of the Handover 121 deploy not
  recorded in-doc; verify via `wrangler deployments list` if needed.
- ✅ **Verified** — read path tested end-to-end against real prod data (see §3).
- ⏳ **Dormant** — no campaigns exist in prod yet (`campaigns`=0,
  `campaign_events`=0). Feature self-hides (SeriesNav renders nothing on 404)
  until an organizer creates a campaign via `/admin`.

## 1. What shipped
A "Part of {Series}" badge + prev/next event cards on the ticket page, driven by
one public read endpoint. No new data model — reuses `campaigns` +
`campaign_events.sequence_order` (which is literally a playlist ordering).

| Layer | File | Change |
|---|---|---|
| Endpoint | `worker/src/handlers/event_series.rs` | NEW `GET /api/public/event-series/{event_id}` (no auth, 120s cache) |
| DB | `worker/src/db/campaigns.rs` | +`get_campaign_for_event` (reverse lookup), `list_campaign_event_summaries` (ordered join), `compute_series_neighbors` (pure prev/next) |
| API client | `frontend-leptos/src/api/event_series.rs` | NEW typed client; 404 → `Ok(None)` |
| UI | `frontend-leptos/src/pages/ticket/series_nav.rs` | NEW `SeriesNav` component (self-hiding) |
| UI wiring | `in_person_view.rs`, `online_view.rs` | `<SeriesNav event_id=…/>` after community links |
| Styles | `frontend-leptos/style.css` | `.ticket-series-*` (badge, grid, cards, empty spacer) |

**Design decisions (applied defaults):**
1. **404 when no campaign** (vs 200 with `campaign: null`) — frontend cache treats
   it as a clean miss; component hides without a null-check dance.
2. **Links go to `/e/{slug}` (public event page)**, not neighbor tickets — the
   attendee isn't registered for the neighbor, so a ticket link would 404.
3. **No auth** — campaign structure is public (like event listings).
4. **Empty neighbor → inert `<div>` spacer** (not `<a href="">`) to keep the
   two-column grid balanced without a navigable dead link.

## 2. Tests
Prev/next logic extracted into pure `compute_series_neighbors(events, id)` so edge
cases are unit-testable without a D1 mock. 9 inline tests in `db/campaigns.rs`
(following the codebase convention — `db/dashboard.rs`, `db/event_summaries.rs`
all use inline `#[cfg(test)]`):

empty list · orphan (linked but missing) · single event · first · last · middle ·
position-by-index-not-sequence_order · full-payload clone · duplicate-id-first-match.

The D1-dependent reverse-lookup and ordered-summary functions have no in-process
D1 mock harness, so they're covered by the prod smoke test below (§3) + manual.

## 3. Verification done (2026-06-26, against prod)
- **Admin route protected**: `GET /api/campaigns` without auth → 401 ✓.
- **Read path, all branches** — reversible D1 smoke test (inserted `plan013-smoke-test`
  campaign linking the 3 real Road to Mainnet events, then deleted it):

| Event under test | current_index | previous | next | Result |
|---|---|---|---|---|
| #1 Bangkok (first) | 0 | `null` | #2 | ✓ |
| #2 (middle) | 1 | #1 | #3 | ✓ |
| #3 Bangkok (last) | 2 | #2 | `null` | ✓ |
| unlinked event | — | — | — | 404 ✓ |

- **Cleanup confirmed**: both tables back to 0 rows; endpoint returns 404 for the
  unlinked event post-cleanup.
- Prod behavior matched the 9 unit tests exactly.

## 4. Remaining (organizer action, not code)
1. Create a campaign via `/admin` → campaigns. Best candidate: "The Road to
   Mainnet" linking events `...-1-bangkok`, `...-2`, `...-3-bangkok` (all exist in
   prod; #1/#2/#3 Bangkok, ordered by `event_start_ms`).
2. Open a linked event's ticket → confirm "Part of The Road to Mainnet" badge +
   prev/next cards render.
3. (Optional) "Solana in Latent Space" — only Part 1 exists in prod, so it's a
   single-event series today (badge shows, no neighbors).

## 5. Deferred (future slices, same foundation endpoint)
- Public playlist page (`/campaign/{slug}`) — reuses `GET /api/public/event-series`.
- Attendee campaign progress display — needs an authed variant of the endpoint.
- Interactive discussion — independent feature, needs its own plan + moderation
  decisions; no tables exist yet.

## 6. Deploy notes
- Frontend WASM rebuilt before deploy (`trunk build --release`, new asset hash
  `c8fd97a372588044`) — required because the worker bundles `frontend-leptos/dist`
  as assets; the deploy script does NOT rebuild the frontend.
- Deploy via `worker/deploy.sh` (versions-API `10013` fallback to PUT API worked;
  DO binding omitted on the PUT path as documented).
- D1 backup taken pre-deploy: `backups/backup-20260626-013.sql` (gitignored).
