-- GET /api/public/stats: deposit payers per event, for the landing's payers'
-- hall (.plans/045 R4.7, R4.2). The same `money` and `paid` rules as
-- public_stats.sql, so the per-event payers sum to `deposit_payers` over the
-- events listed here (worker/tests/security/test_public_stats_by_event.py
-- pins it). Public events only: a private event's deposits count in the
-- totals, but its name never reaches this public endpoint. A separate
-- statement because D1 caps a compound SELECT at a few terms. The track of
-- each stored participation_type is decided in Rust (fold_event_payers), as
-- for the totals.
WITH
  live AS (
    SELECT id, name, slug, event_start_ms,
           ',' || REPLACE(LOWER(COALESCE(staff_emails, '') || ',' || COALESCE(organizer_emails, '')), ' ', '') || ',' AS crew
    FROM events
    WHERE status IN ('active', 'completed') AND visibility = 'public'
  ),
  money AS (
    SELECT d.event_id, d.attendee_id
    FROM thb_deposits d
    WHERE d.verified = 1 AND d.amount_thb > 0
      AND (d.deposit_source IS NULL OR d.deposit_source IN ('cash', 'credit'))
    UNION ALL
    SELECT r.event_id, r.attendee_id
    FROM thb_deposit_archive r
    WHERE r.verified = 1 AND r.amount_thb > 0
      AND NOT EXISTS (SELECT 1 FROM thb_deposits d WHERE d.id = r.source_deposit_id)
  ),
  paid AS (
    SELECT m.event_id, a.participation_type AS pt,
           (a.checked_in_at IS NOT NULL AND a.checked_in_at <> '') AS cin,
           (a.email IS NOT NULL AND INSTR(e.crew, ',' || REPLACE(LOWER(a.email), ' ', '') || ',') > 0) AS staff
    FROM money m
    JOIN live e ON e.id = m.event_id
    LEFT JOIN attendees a ON a.id = m.attendee_id AND a.event_id = m.event_id
  )
SELECT p.event_id AS event_id, e.name AS name, e.slug AS slug,
       e.event_start_ms AS start_ms, COALESCE(p.pt, '(no attendee)') AS pt,
       COUNT(*) AS n, COALESCE(SUM(p.cin), 0) AS came
FROM paid p JOIN live e ON e.id = p.event_id
WHERE NOT p.staff
GROUP BY p.event_id, p.pt
ORDER BY e.event_start_ms, p.event_id
