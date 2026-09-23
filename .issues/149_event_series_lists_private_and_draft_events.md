# 149: The public event-series endpoint lists private and draft events

**Status:** open. Found from code; not reproduced against live data. No
campaign is yet known to hold a private or draft event.
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
