-- 0051 — Moving an event's start date never reached D1 (`.issues/152`).
--
-- `db::events::upsert_event` writes the event row with
-- `INSERT … ON CONFLICT (id) DO UPDATE`. When the start date changes, that
-- fires `notification_reschedule`, whose `INSERT OR IGNORE` re-enrolls every
-- attendee's reminder. SQLite lets the conflict policy of the statement that
-- fired a trigger override the trigger body's own, so under the upsert the
-- `OR IGNORE` is dropped and the first reminder that already exists aborts
-- the whole write: `UNIQUE constraint failed: notification_outbox.dedup_key`.
-- `sync_event_to_d1` only logs the failure, so KV took the new date and D1
-- kept the old one. A plain `UPDATE` does not trip it, which is why every
-- hand test passed.
--
-- Found when RTM#6 was postponed (27 Sep → 4 Oct 2026, flooding): three saves,
-- the public page showed 4 Oct, D1 still said 27 Sep, and the credit ledger
-- (which reads `events.event_end_ms` from D1) would have released the locked
-- credit on the old date.
--
-- The dedup now lives in the SELECT (`NOT EXISTS`), which no outer conflict
-- policy can override. Behaviour is otherwise unchanged.

DROP TRIGGER IF EXISTS notification_reschedule;

CREATE TRIGGER notification_reschedule AFTER UPDATE OF event_start_ms,time_tba ON events
WHEN NEW.event_start_ms IS NOT OLD.event_start_ms OR NEW.time_tba IS NOT OLD.time_tba
BEGIN
    UPDATE notification_outbox SET due_at=MAX(unixepoch(),NEW.event_start_ms/1000-86400)
    WHERE event_id=NEW.id AND kind='reminder' AND status='pending';
    INSERT INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array(NEW.id,ne.attendee_id,'reminder'),NEW.id,ne.attendee_id,'reminder',MAX(unixepoch(),NEW.event_start_ms/1000-86400)
    FROM notification_enrollments ne
    WHERE ne.event_id=NEW.id AND NEW.time_tba=0 AND NEW.event_start_ms>unixepoch()*1000
      AND NOT EXISTS (SELECT 1 FROM notification_outbox o
                      WHERE o.dedup_key=json_array(NEW.id,ne.attendee_id,'reminder'));
END;
