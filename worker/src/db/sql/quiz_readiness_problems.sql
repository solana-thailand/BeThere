SELECT e.id AS event_id,
       e.name AS event_name,
       e.status AS status,
       q.config_json AS config_json,
       SUM(CASE
             WHEN a.checked_in_at IS NOT NULL
              AND (a.claim_asset_id IS NULL OR a.claim_asset_id = '')
             THEN 1 ELSE 0
           END) AS blocked_attendees
FROM events e
LEFT JOIN quiz_configs q ON q.event_id = e.id
LEFT JOIN attendees a ON a.event_id = e.id
WHERE e.quiz_enabled = 1
  AND (?1 = 1 OR
    INSTR(',' || REPLACE(LOWER(e.organizer_emails),' ','') || ',', ',' || ?2 || ',') > 0 OR
    INSTR(',' || REPLACE(LOWER(e.staff_emails),' ','') || ',', ',' || ?2 || ',') > 0)
GROUP BY e.id, e.name, e.status, q.config_json
ORDER BY CASE WHEN LOWER(e.status) = 'active' THEN 0 ELSE 1 END,
         blocked_attendees DESC,
         e.name ASC
