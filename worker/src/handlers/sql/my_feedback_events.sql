SELECT event_id, attendee_id, slug, event_name, event_start_ms, location,
       poster_url, nft_image_url, participation_type, answered
FROM feedback_eligible_events
WHERE lower(email) = lower(?1)
-- Unanswered first, newest first within each group: the work that remains
-- is at the top, and what is done sinks without disappearing.
ORDER BY answered ASC, event_start_ms DESC
