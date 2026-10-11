-- GET /api/public/stats: meals ordered against registrations (.plans/045
-- R4.11). One row per (event with a count, stored participation_type); the
-- track of each type and the sums are decided in Rust (fold_meals), under the
-- same registration rule as public_stats.sql (`invited` excluded). An event
-- with a count and no registrations still has one row (n = 0). A separate
-- statement for the D1 compound-SELECT cap.
WITH
  live AS (
    SELECT id FROM events WHERE status IN ('active', 'completed')
  )
SELECT m.event_id AS event_id, m.ordered AS ordered,
       COALESCE(a.participation_type, '') AS pt,
       COUNT(a.id) AS n,
       COALESCE(SUM(a.checked_in_at IS NOT NULL AND a.checked_in_at <> ''), 0) AS came
FROM event_meals m
JOIN live e ON e.id = m.event_id
LEFT JOIN attendees a
  ON a.event_id = m.event_id AND COALESCE(a.approval_status, '') <> 'invited'
WHERE m.ordered > 0
GROUP BY m.event_id, a.participation_type
ORDER BY m.event_id
