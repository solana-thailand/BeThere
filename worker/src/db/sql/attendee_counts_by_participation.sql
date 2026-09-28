SELECT participation_type, COUNT(*) AS n
FROM attendees
WHERE event_id = ?1
GROUP BY participation_type
