-- GET /api/public/stats, timings: one row per verified THB deposit that moved
-- money (the same deposits as 'paid' in public_stats.sql), with two durations
-- in whole seconds, NULL where the deposit has no such time:
--   slip_s   = slip uploaded → verified;
--   refund_s = event end → refund recorded (0 if refunded before the end).
-- Rust takes the medians (Timing::from_seconds).
WITH
  live AS (
    SELECT id, event_end_ms FROM events WHERE status IN ('active', 'completed')
  ),
  -- Live plus archived deposits, each once (.issues/186; same rule as
  -- `money` in public_stats.sql).
  money AS (
    SELECT d.event_id, d.uploaded_at, d.verified_at, d.refunded, d.refunded_at
    FROM thb_deposits d
    WHERE d.verified = 1 AND d.amount_thb > 0
      AND (d.deposit_source IS NULL OR d.deposit_source IN ('cash', 'credit'))
    UNION ALL
    SELECT r.event_id, r.uploaded_at, r.verified_at, r.refunded, r.refunded_at
    FROM thb_deposit_archive r
    WHERE r.verified = 1 AND r.amount_thb > 0
      AND NOT EXISTS (SELECT 1 FROM thb_deposits d WHERE d.id = r.source_deposit_id)
  ),
  timed AS (
    SELECT m.uploaded_at, m.verified_at, m.refunded, m.refunded_at, e.event_end_ms
    FROM money m
    JOIN live e ON e.id = m.event_id
  )
SELECT
  CASE WHEN julianday(verified_at) IS NOT NULL AND julianday(uploaded_at) IS NOT NULL
       THEN MAX(0, CAST(ROUND((julianday(verified_at) - julianday(uploaded_at)) * 86400.0) AS INTEGER))
  END AS slip_s,
  CASE WHEN refunded = 1 AND event_end_ms > 0 AND julianday(refunded_at) IS NOT NULL
       THEN MAX(0, CAST(ROUND((julianday(refunded_at) - (event_end_ms / 86400000.0 + 2440587.5)) * 86400.0) AS INTEGER))
  END AS refund_s
FROM timed
