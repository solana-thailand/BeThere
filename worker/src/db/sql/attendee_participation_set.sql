UPDATE attendees
SET participation_type = ?3, updated_at = datetime('now')
WHERE id = ?2 AND event_id = ?1
