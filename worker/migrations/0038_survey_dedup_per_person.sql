-- 0038 — One survey message per person, not one per event they attended.
--
-- Asked for in `reports/phase-2/BETHERE-ASKS-2.md` (solana-thailand-devrel-helper),
-- and the diagnosis in it is exactly right.
--
-- `dedup_key` is `[event, attendee, kind]`. That is the correct shape for every
-- kind whose destination is a single event — `reminder`, `deposit_rejected`,
-- `registration`. It is the wrong shape for `survey`, whose destination is
-- `/feedback`, a page deliberately not scoped to an event.
--
-- `dispatch()` sends one message per row, so prod held:
--
--     425 pending survey rows · 206 distinct people · most for one person: 12
--     83 people with duplicates · 219 surplus rows
--
-- The 17 people with five or more are the regulars. The heaviest send landed on
-- the people the programme can least afford to annoy.
--
-- The key's first element is `events.id`, not the slug — they diverge on every
-- event created by duplication (`.issues/079`), e.g. RTM #6's id is still
-- `...mainnet-5-bangkok-copy`. Irrelevant once the survey stops using it, but
-- worth stating so nobody writes a script against the wrong field.
--
-- Identity for a survey is the **person**, and `attendee_id` is per
-- (person, event). The outbox has no email column, but the trigger already
-- joins `attendees`, so it can read one. Lowercased to match
-- `notification_inbox_visible`, which compares `lower(email)`.

DROP TRIGGER IF EXISTS notification_post_event_survey;

CREATE TRIGGER notification_post_event_survey AFTER UPDATE OF post_event_registration_open ON events
WHEN NEW.post_event_registration_open=1 AND COALESCE(OLD.post_event_registration_open,0)<>1
BEGIN
    INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)
    SELECT json_array('*',lower(a.email),'survey'),NEW.id,ne.attendee_id,'survey',unixepoch()
    FROM notification_enrollments ne
    JOIN attendees a ON a.id=ne.attendee_id AND a.event_id=ne.event_id
    WHERE ne.event_id=NEW.id AND a.approval_status<>'pending_approval'
      AND (a.participation_type='online'
           OR (a.checked_in_at IS NOT NULL AND a.checked_in_at<>''));
END;

-- The one-off repair travels with the trigger because the two are inseparable:
-- leave the queued rows on the old key and the next enqueue adds a second,
-- differently-keyed row for the same person instead of colliding with the first.
-- A no-op on any database that has not run the old trigger.

-- Keep the earliest pending survey per person; the rest are superseded, not
-- sent. `cancelled` rather than deleted: `notification_inbox_visible` already
-- excludes it, and an audit trail of what was collapsed is worth the rows.
UPDATE notification_outbox SET status='cancelled'
WHERE kind='survey' AND status='pending' AND id NOT IN (
    SELECT MIN(n.id) FROM notification_outbox n
    JOIN attendees a ON a.id=n.attendee_id
    WHERE n.kind='survey' AND n.status='pending'
    GROUP BY lower(a.email)
);

-- Move the survivors onto the new key. Without this they keep a per-event key
-- that will never again collide with what the trigger writes. Safe against the
-- UNIQUE constraint because the statement above left exactly one per email.
UPDATE notification_outbox
SET dedup_key = json_array(
        '*',
        (SELECT lower(a.email) FROM attendees a WHERE a.id = notification_outbox.attendee_id),
        'survey')
WHERE kind='survey' AND status='pending';
