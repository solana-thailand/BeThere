SELECT e.id AS event_id,e.name AS event_name,e.slug AS event_slug,
       e.event_start_ms,e.event_format,a.id AS attendee_id,a.name,
       a.participation_type,a.claim_token,
       (a.checked_in_at IS NOT NULL AND a.checked_in_at<>'') AS checked_in,
       (a.claimed_at IS NOT NULL AND a.claimed_at<>'') AS claimed,
       (d.event_id IS NOT NULL) AS deposit_exists,
       COALESCE(d.verified,0) AS deposit_verified,
       (d.event_id IS NOT NULL AND (
          d.verified=1 OR d.method<>'usdc' OR COALESCE(d.tx_signature,'')<>''
       )) AS real_deposit
FROM attendees a
JOIN events e ON e.id=a.event_id
LEFT JOIN deposit_statuses d ON d.event_id=a.event_id AND d.attendee_id=a.id
WHERE LOWER(a.email)=LOWER(?1)
  AND e.status NOT IN ('completed','archived')
ORDER BY e.event_start_ms ASC,e.id ASC
