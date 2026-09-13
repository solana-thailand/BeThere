UPDATE notification_outbox SET read_at=unixepoch()
WHERE id IN (
  SELECT id FROM notification_inbox_visible
  WHERE lower(email)=lower(?1) AND read_at IS NULL
)
