-- 0064_event_meals.sql
-- Meals the organizer ordered for an event (.plans/045 R4.11), entered after
-- ordering. The public stats compare it with the on-site registrations:
-- meals not ordered = registered - ordered, summed over the events that have
-- a count. Aggregate only; an event without a row is not in it.
--
-- Its own table, not an `events` column: it is a report, not config, so it
-- stays out of the KV-cached event config, and duplicating an event cannot
-- copy last month's count. The event purge deletes it.
CREATE TABLE IF NOT EXISTS event_meals (
  event_id TEXT PRIMARY KEY,
  ordered INTEGER NOT NULL CHECK (ordered >= 0 AND ordered <= 100000),
  updated_at TEXT NOT NULL
);
