# 150: Renaming an event could take a slug another event already uses

**Status:** fixed on develop 2026-09-24 (session `event-checkin-72`), not
deployed. Prod had no collisions when checked (read-only query below), so
nothing needs repairing.
**Found by:** session `event-checkin-d6`, while doing `.plans/028` W11 (recorded
there as a known limit).
**Severity:** low. Only organisers and super admins can rename, but a
collision makes a public event page serve the wrong event with no error.

## What happened

- `PUT /api/events/{id}` (both `handlers/events/update.rs` and
  `event_store::update_event`) applied `slugify(req.slug)` and saved it. It
  never checked whether another event already held that slug.
- `create_event` deduplicated the new slug against existing **ids** only. An
  event's slug equals its id until someone renames it. After that, create could
  also hand out a slug that was already in use.
- With two events on one slug, `resolve_event_by_slug` serves whichever
  matches first. Since W11 each isolate caches its own answer, so two isolates
  can serve different events for the same `/e/{slug}`.
- A slug equal to another event's **id** is broken too. `resolve_event` tries
  the id first, so id-first paths never reach the renamed event.
- A slug that slugifies to empty (`"!!!"`) was accepted.

## Fix

- The rule is `domain::models::event::slug_taken_by_other`: a slug is taken
  when another event holds it as a slug or as an id.
- `event_store::apply_update_checked` wraps `apply_update`. When the slug
  changes, it rejects an empty slug and checks for a conflict in D1
  (`db::event_slugs`, indexed, `LIMIT 1`) and in the KV index. It fails
  closed. Both PUT entry points call it.
- `create_event` now treats existing slugs, not just ids, as taken.
- Error text: `slug '<s>' is already used by another event` (400, validation).

## Evidence

- Tests:
  - `domain/tests/event_slug_ownership.rs` (5). A mutant that drops the id rule
    fails it.
  - `worker/tests/event_slug_uniqueness_guard.rs` (3). Putting the handler back
    on `apply_update` fails it.
- Local `wrangler dev --local` probe with the KV index seeded. Seed events:
  `alpha/alpha` and `beta-old/beta`. Results:

  | Request | Result |
  |---|---|
  | rename alpha → `beta`, `beta-old`, `Beta` | 400 |
  | rename alpha → `alpha-2` | 200 |
  | repeat alpha → `alpha-2` (no-op) | 200 |
  | rename beta-old → `alpha` (still alpha's id) | 400 |
  | rename beta-old → `!!!` | 400 |
  | create `slug=beta` | `beta-1` (old code: `beta`, a duplicate) |

  D1 read back matched. The log showed no Sheets writes and no panics.
- Prod read-only check, 2026-09-24:
  `SELECT … FROM events a JOIN events b ON a.id <> b.id AND (a.slug = b.slug OR a.slug = b.id)`
  returned 0 rows (53 rows read).

## Not done

- There is no DB-level guarantee. Two concurrent renames onto the same free
  slug can both pass the check.
  - A `UNIQUE` index on `events(slug)` would close that for slug-vs-slug, and
    prod has no duplicates today.
  - It would not cover slug-vs-id.
  - It is a migration, so it is the owner's call.
  - Reopen if a collision is ever seen.
- The seed path's legacy `default` → slugified-name migration
  (`write/seed.rs`) does not run the check. It only fires for a single legacy
  event.
