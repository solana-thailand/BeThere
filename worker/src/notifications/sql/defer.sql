UPDATE notification_outbox
SET status = 'pending', due_at = ?1, attempts = MAX(0, attempts - 1),
    attempted_at = NULL, error_code = NULL
WHERE id = ?2 AND status = 'sending'
