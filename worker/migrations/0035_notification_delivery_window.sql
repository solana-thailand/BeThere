-- 0035_notification_delivery_window.sql — `.issues/080`
--
-- Until now every notification kind was pre-event, so the pipeline carried one
-- rule: never deliver once the event is over. That rule was written into the
-- dispatcher (`notifications/prepare.rs`), into `sql/cancel.sql`, and into the
-- `kind` CHECK below by omission — no kind existed that had to wait.
--
-- `survey` is the first kind that only makes sense *after* the event. It asks
-- the four post-event questions (`.issues/087`) carried by
-- `/events/{slug}/post-event-register`, so it is enqueued exactly when the
-- organizer opens that form and is cancelled again if they close it.
--
-- SQLite cannot alter a CHECK constraint, so the table is rebuilt. The view and
-- every trigger that names `notification_outbox` are dropped first: with
-- `legacy_alter_table` off, SQLite reparses the whole schema during
-- `ALTER TABLE ... RENAME` and fails if a surviving trigger references a table
-- that is momentarily absent. They are recreated below, unchanged apart from
-- the view's new `survey` clause.

DROP VIEW IF EXISTS notification_inbox_visible;
DROP TRIGGER IF EXISTS notification_registration;
DROP TRIGGER IF EXISTS notification_reschedule;
DROP TRIGGER IF EXISTS notification_deposit_insert;
DROP TRIGGER IF EXISTS notification_deposit_update;
DROP TRIGGER IF EXISTS notification_attendee_delete;
DROP TRIGGER IF EXISTS notification_attendee_email;
DROP TRIGGER IF EXISTS notification_event_delete;

CREATE TABLE notification_outbox_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dedup_key TEXT NOT NULL UNIQUE,
    event_id TEXT NOT NULL,
    attendee_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('registration','reminder','deposit_confirmed','deposit_rejected','survey')),
    version TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','sending','accepted','failed','uncertain','cancelled')),
    attempts INTEGER NOT NULL DEFAULT 0,
    due_at INTEGER NOT NULL,
    attempted_at INTEGER,
    message_id TEXT,
    error_code TEXT,
    read_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
INSERT INTO notification_outbox_v2
    (id,dedup_key,event_id,attendee_id,kind,version,status,attempts,due_at,attempted_at,message_id,error_code,read_at,created_at)
SELECT id,dedup_key,event_id,attendee_id,kind,version,status,attempts,due_at,attempted_at,message_id,error_code,read_at,created_at
FROM notification_outbox;
DROP TABLE notification_outbox;
ALTER TABLE notification_outbox_v2 RENAME TO notification_outbox;

CREATE INDEX notification_due ON notification_outbox(status, due_at);
CREATE INDEX notification_event ON notification_outbox(event_id, id);
CREATE INDEX notification_attendee ON notification_outbox(attendee_id, event_id);
CREATE INDEX notification_unread ON notification_outbox(attendee_id, read_at, due_at) WHERE read_at IS NULL AND status<>'cancelled';
CREATE INDEX notification_inflight ON notification_outbox(attempted_at) WHERE status='sending';

-- Unchanged from 0030.
CREATE TRIGGER notification_registration AFTER INSERT ON notification_enrollments BEGIN
    INSERT INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    VALUES(json_array(NEW.event_id,NEW.attendee_id,'registration'),NEW.event_id,NEW.attendee_id,'registration',unixepoch());
    INSERT INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array(NEW.event_id,NEW.attendee_id,'reminder'),NEW.event_id,NEW.attendee_id,'reminder',
           MAX(unixepoch(),event_start_ms/1000-86400)
    FROM events WHERE id=NEW.event_id AND event_start_ms>unixepoch()*1000 AND time_tba=0;
END;
CREATE TRIGGER notification_reschedule AFTER UPDATE OF event_start_ms,time_tba ON events
WHEN NEW.event_start_ms IS NOT OLD.event_start_ms OR NEW.time_tba IS NOT OLD.time_tba
BEGIN
    UPDATE notification_outbox SET due_at=MAX(unixepoch(),NEW.event_start_ms/1000-86400)
    WHERE event_id=NEW.id AND kind='reminder' AND status='pending';
    INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array(NEW.id,attendee_id,'reminder'),NEW.id,attendee_id,'reminder',MAX(unixepoch(),NEW.event_start_ms/1000-86400)
    FROM notification_enrollments WHERE event_id=NEW.id AND NEW.time_tba=0 AND NEW.event_start_ms>unixepoch()*1000;
END;
CREATE TRIGGER notification_deposit_insert AFTER INSERT ON deposit_statuses
WHEN (NEW.verified=1 OR NEW.rejected=1)
BEGIN
    INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,version,due_at)
    SELECT json_array(NEW.event_id,NEW.attendee_id,CASE WHEN NEW.rejected=1 THEN 'deposit_rejected' ELSE 'deposit_confirmed' END,NEW.deposited_at),
        NEW.event_id,NEW.attendee_id,CASE WHEN NEW.rejected=1 THEN 'deposit_rejected' ELSE 'deposit_confirmed' END,NEW.deposited_at,unixepoch()
    FROM notification_enrollments WHERE attendee_id=NEW.attendee_id AND event_id=NEW.event_id;
END;
CREATE TRIGGER notification_deposit_update AFTER UPDATE ON deposit_statuses
WHEN (NEW.verified=1 OR NEW.rejected=1) AND (NEW.verified IS NOT OLD.verified OR NEW.rejected IS NOT OLD.rejected OR NEW.deposited_at IS NOT OLD.deposited_at)
BEGIN
    INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,version,due_at)
    SELECT json_array(NEW.event_id,NEW.attendee_id,CASE WHEN NEW.rejected=1 THEN 'deposit_rejected' ELSE 'deposit_confirmed' END,NEW.deposited_at),
        NEW.event_id,NEW.attendee_id,CASE WHEN NEW.rejected=1 THEN 'deposit_rejected' ELSE 'deposit_confirmed' END,NEW.deposited_at,unixepoch()
    FROM notification_enrollments WHERE attendee_id=NEW.attendee_id AND event_id=NEW.event_id;
END;
CREATE TRIGGER notification_attendee_delete AFTER DELETE ON attendees BEGIN
    DELETE FROM notification_outbox WHERE attendee_id=OLD.id AND event_id=OLD.event_id;
    DELETE FROM notification_enrollments WHERE attendee_id=OLD.id;
END;
CREATE TRIGGER notification_attendee_email AFTER UPDATE OF email ON attendees WHEN NEW.email IS NOT OLD.email BEGIN
    DELETE FROM notification_outbox WHERE attendee_id=OLD.id AND event_id=OLD.event_id;
    DELETE FROM notification_enrollments WHERE attendee_id=OLD.id;
END;
CREATE TRIGGER notification_event_delete AFTER DELETE ON events BEGIN
    DELETE FROM notification_outbox WHERE event_id=OLD.id;
    DELETE FROM notification_enrollments WHERE event_id=OLD.id;
END;

-- Opening post-event registration is the organizer action that makes the survey
-- answerable, so it is also what enqueues it. Only people who were actually
-- there are asked how it went; `prepare` re-checks both at send time.
--
-- There is deliberately no INSERT counterpart: `post_event_registration_open`
-- can only be set on a Completed event, and events are not created Completed.
CREATE TRIGGER notification_post_event_survey AFTER UPDATE OF post_event_registration_open ON events
WHEN NEW.post_event_registration_open=1 AND COALESCE(OLD.post_event_registration_open,0)<>1
BEGIN
    INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array(NEW.id,ne.attendee_id,'survey'),NEW.id,ne.attendee_id,'survey',unixepoch()
    FROM notification_enrollments ne
    JOIN attendees a ON a.id=ne.attendee_id AND a.event_id=ne.event_id
    WHERE ne.event_id=NEW.id AND a.approval_status='approved'
      AND a.checked_in_at IS NOT NULL AND a.checked_in_at<>'';
END;

-- One eligibility definition is shared by list, unread and read mutations.
-- Reminders disappear after check-in, on TBA/cancelled events, or after start.
-- Surveys appear only while the form that carries the questions still accepts,
-- and only for the people who were there.
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
WHERE a.approval_status='approved' AND n.due_at<=unixepoch()
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
