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
  timed AS (
    SELECT d.uploaded_at, d.verified_at, d.refunded, d.refunded_at, e.event_end_ms
    FROM thb_deposits d
    JOIN live e ON e.id = d.event_id
    WHERE d.verified = 1 AND d.amount_thb > 0
      AND (d.deposit_source IS NULL OR d.deposit_source IN ('cash', 'credit'))
  )
SELECT
  CASE WHEN julianday(verified_at) IS NOT NULL AND julianday(uploaded_at) IS NOT NULL
       THEN MAX(0, CAST(ROUND((julianday(verified_at) - julianday(uploaded_at)) * 86400.0) AS INTEGER))
  END AS slip_s,
  CASE WHEN refunded = 1 AND event_end_ms > 0 AND julianday(refunded_at) IS NOT NULL
       THEN MAX(0, CAST(ROUND((julianday(refunded_at) - (event_end_ms / 86400000.0 + 2440587.5)) * 86400.0) AS INTEGER))
  END AS refund_s
FROM timed
