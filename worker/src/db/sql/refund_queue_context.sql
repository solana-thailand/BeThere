SELECT a.id AS attendee_id,
       a.participation_type AS participation_type,
       (CASE WHEN a.checked_in_at IS NOT NULL AND a.checked_in_at <> '' THEN 1 ELSE 0 END) AS checked_in,
       aa.answer AS answer
FROM attendees a
LEFT JOIN attendance_answers aa
  ON aa.event_id = a.event_id AND aa.attendee_id = a.id
WHERE a.event_id = ?1
