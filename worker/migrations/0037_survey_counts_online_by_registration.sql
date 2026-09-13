-- 0037 — Online attendees could never be asked how the event went.
--
-- The survey gate was `checked_in_at IS NOT NULL`. Nobody watching a livestream
-- is scanned at a door, so on the twelve open events that is:
--
--     participation_type   registered   checked_in
--     online                      337            2
--     in_person                   120           88
--
-- 335 people excluded by a condition they had no way to satisfy.
--
-- DevRel already wrote the rule this should have used, in BETHERE-ASKS.md:
-- **"onsite by check-in, online by registration."** It is the rule their own
-- Phase 1 figures are counted with, so using anything else here would produce
-- survey responses that cannot be compared to the attendance numbers already
-- submitted to the Foundation.
--
-- What stays excluded, and why that is right:
--   * `in_person` who never checked in — 32 people who reserved a seat and did
--     not come. "How was the event?" is the wrong question for them.
--   * `retrospective` — someone who found the form afterwards and filled it in
--     did not attend; asking them to rate the event would put invented numbers
--     in the same column as real ones.
--
-- Reach goes from 90 rows / 55 people to 425 / 206.

DROP VIEW IF EXISTS notification_inbox_visible;
DROP TRIGGER IF EXISTS notification_post_event_survey;

CREATE TRIGGER notification_post_event_survey AFTER UPDATE OF post_event_registration_open ON events
WHEN NEW.post_event_registration_open=1 AND COALESCE(OLD.post_event_registration_open,0)<>1
BEGIN
    INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array(NEW.id,ne.attendee_id,'survey'),NEW.id,ne.attendee_id,'survey',unixepoch()
    FROM notification_enrollments ne
    JOIN attendees a ON a.id=ne.attendee_id AND a.event_id=ne.event_id
    WHERE ne.event_id=NEW.id AND a.approval_status<>'pending_approval'
      AND (a.participation_type='online'
           OR (a.checked_in_at IS NOT NULL AND a.checked_in_at<>''));
END;

CREATE VIEW notification_inbox_visible AS
SELECT n.id,n.kind,n.due_at,n.read_at,n.event_id,n.attendee_id,
       e.name AS event_name,e.slug,e.deposit_enabled,
       a.email,a.participation_type,a.deposit_status,
       COALESCE(d.verified,0) AS deposit_verified,
       COALESCE(d.rejected,0) AS deposit_rejected
FROM notification_outbox n
JOIN notification_enrollments ne ON ne.attendee_id=n.attendee_id AND ne.event_id=n.event_id
JOIN attendees a ON a.id=n.attendee_id AND a.event_id=n.event_id
JOIN events e ON e.id=n.event_id
LEFT JOIN deposit_statuses d ON d.attendee_id=n.attendee_id AND d.event_id=n.event_id
WHERE a.approval_status<>'pending_approval' AND n.due_at<=unixepoch()
  AND n.status<>'cancelled' AND e.status<>'cancelled'
  AND (n.kind<>'reminder' OR (
    e.status='active' AND e.time_tba=0 AND e.event_start_ms>unixepoch()*1000
    AND (a.checked_in_at IS NULL OR a.checked_in_at='')
  ))
  AND (n.kind<>'survey' OR (
    e.post_event_registration_open=1
    AND (e.post_event_registration_until_ms IS NULL OR e.post_event_registration_until_ms>unixepoch()*1000)
    AND (a.participation_type='online'
         OR (a.checked_in_at IS NOT NULL AND a.checked_in_at<>''))
  ));
