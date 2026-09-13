SELECT event_id, slug, event_name, event_start_ms, location,
       poster_url, nft_image_url, participation_type
FROM feedback_eligible_events
WHERE lower(email) = lower(?1)
ORDER BY event_start_ms DESC
