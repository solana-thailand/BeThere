UPDATE notification_outbox SET read_at=COALESCE(read_at,unixepoch())
WHERE id IN (
  SELECT id FROM notification_inbox_visible
  WHERE id=?1 AND lower(email)=lower(?2)
)
RETURNING id
