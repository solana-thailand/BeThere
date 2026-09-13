-- Backfill notification enrolment for people who actually attended, on events
-- whose post-event form is open. `signup.rs` only enrols Google-verified
-- self-registrations, so the 477 attendees imported via sheet sync have none.
INSERT OR IGNORE INTO notification_enrollments(attendee_id, event_id)
SELECT a.id, a.event_id
FROM attendees a
JOIN events e ON e.id = a.event_id
WHERE a.approval_status = 'approved'
  AND a.email <> ''
  AND e.post_event_registration_open = 1
  AND a.checked_in_at IS NOT NULL
  AND a.checked_in_at <> '';

-- The `notification_registration` trigger fires unconditionally on that INSERT,
-- so it just queued a "Registration saved" message for events that ended months
-- ago. Nothing has ever read these rows; drop them rather than cancel them.
DELETE FROM notification_outbox
WHERE kind = 'registration'
  AND event_id IN (SELECT id FROM events WHERE post_event_registration_open = 1);

-- The survey trigger is AFTER UPDATE OF post_event_registration_open and the
-- flag was already flipped to 1 before migration 0035 created it, so it can
-- never fire for these events. Enqueue exactly what it would have.
INSERT OR IGNORE INTO notification_outbox(dedup_key, event_id, attendee_id, kind, due_at)
SELECT json_array(e.id, ne.attendee_id, 'survey'), e.id, ne.attendee_id, 'survey', unixepoch()
FROM notification_enrollments ne
JOIN events e ON e.id = ne.event_id
JOIN attendees a ON a.id = ne.attendee_id AND a.event_id = ne.event_id
WHERE e.post_event_registration_open = 1
  AND a.approval_status = 'approved'
  AND a.checked_in_at IS NOT NULL
  AND a.checked_in_at <> '';
