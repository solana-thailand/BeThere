-- 0036 — The notification inbox was invisible to half the people who attended.
--
-- `attendees.approval_status` is a progression, not a set of alternatives:
-- PendingApproval → Approved → Invited → CheckedIn (`CheckInStatus`,
-- domain/src/models/attendee/status.rs). Someone who was scanned at the door
-- ends at `checked_in`, which is *past* `approved`, not instead of it.
--
-- Both the inbox view and the survey trigger filtered `approval_status =
-- 'approved'`, so every attendee whose row had advanced to `checked_in` was
-- excluded. In production that is exactly half of the eligible rows — 45
-- `approved` and 45 `checked_in` — and for the post-event survey it excluded
-- precisely the people the survey is for.
--
-- It surfaced because the organizer checked his own account: 5 check-ins on 5
-- events with the form open, 4 blocks on the feedback page. The missing one was
-- RTM #2, the single row of his that reads `checked_in`.
--
-- The filter is now `<> 'pending_approval'` rather than an IN list. An IN list
-- fails closed when a new status appears — silently dropping people, which is
-- this bug. Failing open means at worst a pending registration sees a message,
-- which is visible and recoverable. For a notification system, visible-and-wrong
-- beats silent-and-missing.

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
      AND a.checked_in_at IS NOT NULL AND a.checked_in_at<>'';
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
    AND a.checked_in_at IS NOT NULL AND a.checked_in_at<>''
  ));
