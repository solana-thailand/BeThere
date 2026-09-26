-- 0052_attendance_answers.sql
-- What each registrant answered when the organizer asked whether they can
-- still come (RTM#6 was postponed 27 Sep → 4 Oct 2026 by flooding, and every
-- in-person registrant had to be asked again).
--
-- A table of its own rather than a column on `attendees`: an answer belongs to
-- one round of asking, is written only by staff, and must not ride along in
-- the many `attendees` writers (sheet sync, walk-ins, registration upserts),
-- any of which could otherwise clobber it.
--
-- No row = not asked or no answer yet. The CHECK is safe here because this
-- table is new and `db::attendance_answers` is its only writer; the values
-- mirror `AttendanceAnswer::as_str` in the domain crate.

CREATE TABLE IF NOT EXISTS attendance_answers (
    event_id    TEXT NOT NULL,
    attendee_id TEXT NOT NULL,
    answer      TEXT NOT NULL CHECK (answer IN ('coming', 'undecided', 'not_coming')),
    updated_by  TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (event_id, attendee_id)
);
