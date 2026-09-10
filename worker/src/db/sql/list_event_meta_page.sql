SELECT id,name,slug,status,event_format,event_start_ms,event_end_ms,time_tba,
       sheet_id,created_at,organization_id,organizer_emails,deposit_enabled,
       max_refundable_deposits,escrow_address,escrow_status,tagline,location,
       video_url,nft_image_url,poster_url,recap_published,
       post_event_registration_open,post_event_registration_until_ms,
       in_person_capacity,online_capacity,visibility
FROM events
WHERE (?1 = 1 OR
  INSTR(',' || REPLACE(LOWER(organizer_emails),' ','') || ',', ',' || ?2 || ',') > 0 OR
  INSTR(',' || REPLACE(LOWER(staff_emails),' ','') || ',', ',' || ?2 || ',') > 0)
AND (?3 = '' OR created_at < ?3 OR (created_at = ?3 AND id < ?4))
ORDER BY created_at DESC, id DESC
LIMIT ?5
