DELETE FROM attendance_answers
WHERE event_id = ?1 AND attendee_id = ?2
  AND EXISTS (SELECT 1 FROM attendees WHERE id = ?2 AND event_id = ?1)
