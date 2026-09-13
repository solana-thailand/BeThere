UPDATE notification_outbox
SET status = 'sending', attempts = attempts + 1, attempted_at = unixepoch()
WHERE id = (
    SELECT n.id
    FROM notification_outbox n
    JOIN events e ON e.id = n.event_id
    WHERE n.status = 'pending'
      AND n.due_at <= unixepoch()
      AND (
          n.kind <> 'reminder'
          OR (e.time_tba = 0 AND e.event_start_ms <= (unixepoch() + 86400) * 1000)
      )
    ORDER BY n.due_at, n.id
    LIMIT 1
) AND status = 'pending'
RETURNING *
