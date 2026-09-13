UPDATE notification_outbox
SET status='pending',attempts=0,due_at=unixepoch(),error_code=NULL
WHERE event_id=?1 AND id=?2 AND status='failed'
RETURNING id
