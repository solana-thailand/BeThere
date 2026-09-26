INSERT INTO attendance_answers (event_id, attendee_id, answer, updated_by, updated_at)
SELECT ?1, id, ?3, ?4, datetime('now') FROM attendees
WHERE id = ?2 AND event_id = ?1
ON CONFLICT (event_id, attendee_id) DO UPDATE SET
    answer = excluded.answer,
    updated_by = excluded.updated_by,
    updated_at = excluded.updated_at
