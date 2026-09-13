UPDATE notification_outbox
SET status=?1,message_id=NULLIF(?2,''),error_code=NULLIF(?3,''),due_at=unixepoch()+?4
WHERE id=?5 AND status='sending'
RETURNING id
