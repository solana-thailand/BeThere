-- 0040 — The feedback page could not tell answered from unanswered.
--
-- `feedback_eligible_events` means "this person may rate this session". The page
-- needs "this person still needs to rate this session", and had no way to ask.
--
-- Observed on production after the first real submission: four `post.*` rows
-- were written for RTM #4 — the data is fine — and the page still reported
-- `0 จาก 11`, still offered the answered session, and would have written a
-- second set of answers over the first (`.issues/107`).
--
-- `answered` counts `post.%` rows for the (person, event). The submission path
-- writes them through `registration_responses` keyed by `developer_email`, and
-- the join is on lowercased email because that is what every other identity
-- comparison in this schema uses.
--
-- Kept as a column rather than a filter: a session that has been answered should
-- still be listed, so the reader can see it is done and change their mind. A
-- view that silently dropped it would look like the answer had been lost, which
-- is the confusion this is fixing.

DROP VIEW IF EXISTS feedback_eligible_events;

CREATE VIEW feedback_eligible_events AS
SELECT a.email,
       ne.attendee_id,
       e.id   AS event_id,
       e.slug,
       e.name AS event_name,
       e.event_start_ms,
       e.location,
       e.poster_url,
       e.nft_image_url,
       a.participation_type,
       EXISTS (
           SELECT 1 FROM registration_responses r
           WHERE r.event_id = e.id
             AND lower(r.developer_email) = lower(a.email)
             AND r.field_key LIKE 'post.%'
       ) AS answered
FROM notification_enrollments ne
JOIN attendees a ON a.id = ne.attendee_id AND a.event_id = ne.event_id
JOIN events e ON e.id = ne.event_id
WHERE a.approval_status <> 'pending_approval'
  AND e.status <> 'cancelled'
  AND e.post_event_registration_open = 1
  AND (e.post_event_registration_until_ms IS NULL
       OR e.post_event_registration_until_ms > unixepoch() * 1000)
  -- Onsite by check-in, online by registration (.issues/097).
  AND (a.participation_type = 'online'
       OR (a.checked_in_at IS NOT NULL AND a.checked_in_at <> ''));
