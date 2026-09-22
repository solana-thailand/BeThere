-- Claim exactly one due job. A single UPDATE takes the row, so overlapping
-- crons cannot pick up the same one.
--
-- `?1` is `NOTIFICATIONS_STALENESS`. The age-table placeholder below is spliced
-- in by `staleness::render` from `policy::max_age_secs` — never edit ages here.
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
      -- Age guard (`.issues/128`). Withholding stale rows here rather than at
      -- `prepare` is what makes `report` observable: a stale row never spends
      -- one of the 25 claim slots a run has, so the guard can be watched for a
      -- while without the backlog draining past it in the meantime.
      AND (?1 = 'off' OR n.due_at > unixepoch() - ({{max_age_secs}}))
    ORDER BY n.due_at, n.id
    LIMIT 1
) AND status = 'pending'
RETURNING *
