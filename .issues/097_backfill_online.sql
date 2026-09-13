-- Backfill for `.issues/097` — enrol and queue the online attendees that the
-- `checked_in_at` gate could never reach.
--
-- "Onsite by check-in, online by registration" (DevRel's own counting rule).
-- Safe on top of the earlier passes: every statement is INSERT OR IGNORE
-- against a unique key.
INSERT OR IGNORE INTO notification_enrollments(attendee_id, event_id)
SELECT a.id, a.event_id
FROM attendees a
JOIN events e ON e.id = a.event_id
WHERE a.approval_status <> 'pending_approval'
  AND a.email <> ''
  AND e.post_event_registration_open = 1
  AND (a.participation_type = 'online'
       OR (a.checked_in_at IS NOT NULL AND a.checked_in_at <> ''));

-- `notification_registration` fires unconditionally on enrolment insert and
-- queues a "Registration saved" message for events that ended months ago.
DELETE FROM notification_outbox
WHERE kind = 'registration'
  AND event_id IN (SELECT id FROM events WHERE post_event_registration_open = 1);

INSERT OR IGNORE INTO notification_outbox(dedup_key, event_id, attendee_id, kind, due_at)
SELECT json_array(e.id, ne.attendee_id, 'survey'), e.id, ne.attendee_id, 'survey', unixepoch()
FROM notification_enrollments ne
JOIN events e ON e.id = ne.event_id
JOIN attendees a ON a.id = ne.attendee_id AND a.event_id = ne.event_id
WHERE e.post_event_registration_open = 1
  AND a.approval_status <> 'pending_approval'
  AND (a.participation_type = 'online'
       OR (a.checked_in_at IS NOT NULL AND a.checked_in_at <> ''));
