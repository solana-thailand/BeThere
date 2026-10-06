-- GET /api/public/stats: aggregates as (kind, stored participation_type, n,
-- checked_in) rows. Timings are a separate statement
-- (public_stats_timings.sql): D1 caps a compound SELECT at a few terms. The
-- track of each stored value is decided in Rust by
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
  -- Live deposits plus the ones the nightly purge moved to the archive
  -- (.issues/186), each counted once: an archived row whose live row still
  -- exists (mid-purge) is skipped by its source_deposit_id. The archive has
  -- no deposit_source; a comp is a ฿0 waive, so amount_thb > 0 is the same
  -- rule there. Attendee rows are not purged, so check-ins survive.
  money AS (
    SELECT d.event_id, d.attendee_id, d.amount_thb AS thb
    FROM thb_deposits d
    WHERE d.verified = 1 AND d.amount_thb > 0
      AND (d.deposit_source IS NULL OR d.deposit_source IN ('cash', 'credit'))
    UNION ALL
    SELECT r.event_id, r.attendee_id, r.amount_thb
    FROM thb_deposit_archive r
    WHERE r.verified = 1 AND r.amount_thb > 0
      AND NOT EXISTS (SELECT 1 FROM thb_deposits d WHERE d.id = r.source_deposit_id)
  ),
  paid AS (
    SELECT m.thb, a.participation_type AS pt,
           (a.checked_in_at IS NOT NULL AND a.checked_in_at <> '') AS cin
    FROM money m
    JOIN live e ON e.id = m.event_id
    LEFT JOIN attendees a ON a.id = m.attendee_id AND a.event_id = m.event_id
  )
SELECT 'held' AS kind, '' AS pt, COUNT(*) AS n, 0 AS came FROM live
  WHERE event_end_ms > 0 AND event_end_ms < CAST(strftime('%s', 'now') AS INTEGER) * 1000
UNION ALL
SELECT 'reg', COALESCE(pt, ''), COUNT(*), COALESCE(SUM(cin), 0) FROM regs GROUP BY pt
UNION ALL
SELECT 'paid', COALESCE(pt, '(no attendee)'), COUNT(*), COALESCE(SUM(cin), 0) FROM paid GROUP BY pt
UNION ALL
SELECT 'thb', '', COALESCE(SUM(thb), 0), 0 FROM paid
