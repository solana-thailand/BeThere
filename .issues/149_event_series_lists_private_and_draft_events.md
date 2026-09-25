# 149: The public event-series endpoint lists private and draft events

**Status:** deployed 2026-09-24 (prod version `36eae0db`, git `897aa07`); fixed in session `event-checkin-aa`.
Found from code; the live repro query below has not been run, so
whether anything leaked in prod is still unknown.
**Found by:** session `event-checkin-9f`, while fixing `.plans/028` W4 (the
private-event `Cache-Control` bug).
**Severity:** low-medium. It is an information disclosure: names and slugs
only, with no attendee data. A leaked slug is enough to reach the private
event's page, and that page still asks for auth.

## What happens

`GET /api/public/event-series/{event_id}` (`worker/src/handlers/event_series.rs`)
is in the public router with no auth. It returns every event in the campaign
that contains `{event_id}`. The query behind it,
`db/campaigns/series.rs::list_campaign_event_summaries`, filters only on
`ce.campaign_id = ?`:

```sql
SELECT ce.event_id, e.name, e.slug, … FROM campaign_events ce
LEFT JOIN events e ON e.id = ce.event_id
WHERE ce.campaign_id = ?
```

Nothing checks `e.visibility` or `e.status`. So one public event in a
campaign exposes the `name` and `slug` of every sibling event:
`Private`, `Draft` and `Archived` ones alike. `get_public_event` hides all
three (Draft/Archived → 404; Private → 401/403). The response is also cached
`public, max-age=120`.

## Fix sketch

- Filter in SQL: `AND e.visibility = 'public' AND e.status IN ('active','completed')`.
  Use the stored enum strings (`EventVisibility::as_str`, `EventStatus::as_str`).
- Treat the requested `{event_id}` the same way. If it is not public, return
  404, as `get_public_event` does.
- Test both the neighbour computation over a filtered list and the SQL
  predicate string, so that the rule cannot silently regress.

## Repro to run before fixing

```sql
-- prod/staging: campaigns whose series would leak a non-public sibling
SELECT ce.campaign_id, e.id, e.visibility, e.status
FROM campaign_events ce JOIN events e ON e.id = ce.event_id
WHERE e.visibility <> 'public' OR e.status NOT IN ('active','completed');
```

An empty result means nothing has leaked so far. The fix is still owed,
because the endpoint makes no promise about what a campaign contains.

## Fix (develop, 2026-09-24)

- `db/campaigns/series.rs::series_summaries_sql()` is now a pure builder:
  `INNER JOIN events` plus `e.visibility = 'public' AND e.status IN
  ('active', 'completed')`, spelled from `EventVisibility::as_str` and
  `EventStatus::as_str`.
- `handlers/event_series.rs` returns 404 when the requested event is not in
  the filtered list (private, draft, archived or dangling), so its campaign is
  not named either.
- Test `db::campaigns::tests::series_sql_lists_only_public_live_events`.
  Mutant (visibility line deleted) → 1 red. The SQL was also run against a
  scratch SQLite table with one event of each kind plus a dangling link: only
  the public active and public completed rows came back.
- Not covered natively: the handler's 404 branch (needs D1). Check it on
  staging with a private event in a campaign.
- Behaviour change: a dangling `campaign_events` row used to return the
  series with `current_index = -1`; it is now a 404, which `SeriesNav`
  already treats as "hide".
- Left alone: `get_campaign_for_event` does not check the campaign's own
  `status`, so a `draft` campaign's title is served for any public member
  event. File separately if drafts are meant to be secret.
