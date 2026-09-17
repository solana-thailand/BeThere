-- 0039 — `/feedback` must enumerate the sessions itself.
--
-- Migration 0038 collapsed the survey outbox to one row per person, which is
-- right for *sending*. But `/feedback` derived its question blocks from that
-- same outbox — `.issues/091` deliberately reused `notification_inbox_visible`
-- so eligibility had one definition — so collapsing the message also collapsed
-- the form. A person with five sessions was offered one.
--
-- The two need different shapes and that is not a contradiction:
--
--     the message  -> one per PERSON   ("how were the sessions you attended")
--     the form     -> one per (PERSON, EVENT)
--
-- DevRel's own ask said so: *"The mail needs to speak about 'the sessions you
-- attended' and let /feedback enumerate them."* Enumerating requires a source
-- that is not the collapsed queue.
--
-- So eligibility still has one definition, just not one keyed to the outbox.
-- This view is the enrolment side of the rule that
-- `notification_inbox_visible`'s survey clause applies to a queued row; both
-- read the same columns and must agree. Anything that changes one has to change
-- the other, which is the cost of having a queue whose grain differs from the
-- form's. Recorded in `.issues/102` rather than hidden.

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
       a.participation_type
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
