-- Retire every message that outlived its kind's useful life.
--
-- `STALE` rather than `NO_LONGER_ELIGIBLE`: the row was eligible, the queue was
-- simply too slow, and the two are worth telling apart when reading the outbox
-- back. `RETURNING` reports exactly what went, so the audit line is the set
-- that was actually changed rather than a second read of a moving table.
UPDATE notification_outbox
SET status = 'cancelled', error_code = 'STALE'
WHERE status IN ('pending', 'failed')
  AND due_at <= unixepoch() - ({{max_age_secs}})
RETURNING kind, due_at
