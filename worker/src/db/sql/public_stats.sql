-- GET /api/public/stats: aggregates as (kind, stored participation_type, n,
-- checked_in) rows. The track of each stored value is decided in Rust by
-- ParticipationType::parse (domain/src/models/public_stats.rs), never by a
-- SQL predicate, so the two cannot drift (same rule as track_counts.rs).
WITH
  live AS (
    SELECT id, event_end_ms FROM events WHERE status IN ('active', 'completed')
  ),
  regs AS (
    SELECT a.participation_type AS pt,
           (a.checked_in_at IS NOT NULL AND a.checked_in_at <> '') AS cin
    FROM attendees a JOIN live e ON e.id = a.event_id
    WHERE COALESCE(a.approval_status, '') <> 'invited'
  ),
  paid AS (
    SELECT d.amount_thb AS thb, a.participation_type AS pt,
           (a.checked_in_at IS NOT NULL AND a.checked_in_at <> '') AS cin
    FROM thb_deposits d
    JOIN live e ON e.id = d.event_id
    LEFT JOIN attendees a ON a.id = d.attendee_id AND a.event_id = d.event_id
    WHERE d.verified = 1 AND d.amount_thb > 0
      AND (d.deposit_source IS NULL OR d.deposit_source IN ('cash', 'credit'))
  )
SELECT 'held' AS kind, '' AS pt, COUNT(*) AS n, 0 AS came FROM live
  WHERE event_end_ms > 0 AND event_end_ms < CAST(strftime('%s', 'now') AS INTEGER) * 1000
UNION ALL
SELECT 'reg', COALESCE(pt, ''), COUNT(*), COALESCE(SUM(cin), 0) FROM regs GROUP BY pt
UNION ALL
SELECT 'paid', COALESCE(pt, '(no attendee)'), COUNT(*), COALESCE(SUM(cin), 0) FROM paid GROUP BY pt
UNION ALL
SELECT 'thb', '', COALESCE(SUM(thb), 0), 0 FROM paid
