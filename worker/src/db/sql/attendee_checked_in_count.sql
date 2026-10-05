SELECT COUNT(*) AS n
FROM attendees
WHERE event_id = ?1
  AND checked_in_at IS NOT NULL
  AND checked_in_at <> ''
