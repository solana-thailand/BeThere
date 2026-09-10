-- Only explicitly enrolled, verified-email self-registrations receive messages.
-- No historical backfill. Enrollment and registration commit in one D1 batch.
-- User-scoped core reads cannot use the event-leading uniqueness index.
CREATE INDEX IF NOT EXISTS idx_attendees_email_nocase ON attendees(LOWER(email));
CREATE TABLE notification_enrollments (
    attendee_id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE notification_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dedup_key TEXT NOT NULL UNIQUE,
    event_id TEXT NOT NULL,
    attendee_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('registration','reminder','deposit_confirmed','deposit_rejected')),
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
CREATE INDEX notification_due ON notification_outbox(status, due_at);
CREATE INDEX notification_event ON notification_outbox(event_id, id);
CREATE INDEX notification_attendee ON notification_outbox(attendee_id, event_id);
CREATE INDEX notification_unread ON notification_outbox(attendee_id, read_at, due_at) WHERE read_at IS NULL AND status<>'cancelled';
CREATE INDEX notification_enrollment_event ON notification_enrollments(event_id);
CREATE INDEX notification_inflight ON notification_outbox(attempted_at) WHERE status='sending';
-- One eligibility definition is shared by list, unread and read mutations.
-- Reminders disappear after check-in, on TBA/cancelled events, or after start.
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
  ));
CREATE TRIGGER notification_registration AFTER INSERT ON notification_enrollments BEGIN
    INSERT INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    VALUES(json_array(NEW.event_id,NEW.attendee_id,'registration'),NEW.event_id,NEW.attendee_id,'registration',unixepoch());
    INSERT INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array(NEW.event_id,NEW.attendee_id,'reminder'),NEW.event_id,NEW.attendee_id,'reminder',
           MAX(unixepoch(),event_start_ms/1000-86400)
    FROM events WHERE id=NEW.event_id AND event_start_ms>unixepoch()*1000 AND time_tba=0;
END;
-- Changes to dates reschedule unsent reminders; cancelled events are also checked at send time.
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
-- Erasure or identity changes must not leave queued personal notifications.
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
