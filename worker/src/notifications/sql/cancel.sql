UPDATE notification_outbox
SET status = 'cancelled', error_code = 'NO_LONGER_ELIGIBLE'
WHERE status IN ('pending', 'failed')
  AND (
      NOT EXISTS (
          SELECT 1 FROM attendees a
          WHERE a.id = notification_outbox.attendee_id
            AND a.event_id = notification_outbox.event_id
            AND a.approval_status = 'approved'
      )
      OR NOT EXISTS (
          SELECT 1 FROM events e
          WHERE e.id = notification_outbox.event_id
            AND e.status = 'active'
            AND (e.event_end_ms = 0 OR e.event_end_ms > unixepoch() * 1000)
      )
  )
