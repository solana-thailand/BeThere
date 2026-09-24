-- What `cancel` mode would cancel, without cancelling it. Returns one row per
-- stale message so the caller can count by kind and name the oldest; the set is
-- bounded by the queue and read once a day.
--
-- Shares its predicate with `stale_cancel.sql` by inspection, not by
-- construction — `report_and_cancel_agree_on_what_is_stale` holds them level.
SELECT kind, due_at
FROM notification_outbox
WHERE status IN ('pending', 'failed')
  AND due_at <= unixepoch() - ({{max_age_secs}})
ORDER BY due_at, id
