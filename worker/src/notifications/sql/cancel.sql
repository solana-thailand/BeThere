-- Bulk-cancel queued rows an event can no longer receive, so a dead event does
-- not spend the per-cron claim budget one row at a time.
--
-- The timing half is split by delivery window: a pre-event kind dies when the
-- event ends, a post-event kind is *waiting* for that (`.issues/080`). The kind
-- list mirrors `policy::NotificationKind::delivery_window`; the Rust test
-- `cancel_sql_covers_every_pre_event_kind` reads it back out of this file.
UPDATE notification_outbox
SET status = 'cancelled', error_code = 'NO_LONGER_ELIGIBLE'
WHERE status IN ('pending', 'failed')
  AND (
      NOT EXISTS (
          SELECT 1 FROM attendees a
          WHERE a.id = notification_outbox.attendee_id
            AND a.event_id = notification_outbox.event_id
            AND a.approval_status = 'approved'
      )
      OR NOT EXISTS (
          SELECT 1 FROM events e
          WHERE e.id = notification_outbox.event_id
            AND CASE
                WHEN notification_outbox.kind IN ('registration', 'reminder', 'deposit_confirmed', 'deposit_rejected')
                    THEN e.status = 'active'
                         AND (e.event_end_ms = 0 OR e.event_end_ms > unixepoch() * 1000)
                ELSE e.status IN ('active', 'completed')
                     AND (e.status = 'completed'
                          OR (e.event_end_ms > 0 AND e.event_end_ms <= unixepoch() * 1000))
            END
      )
  )
