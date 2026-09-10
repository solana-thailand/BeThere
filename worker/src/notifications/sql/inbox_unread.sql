SELECT COUNT(*) AS count
FROM notification_inbox_visible
WHERE lower(email)=lower(?1) AND read_at IS NULL
