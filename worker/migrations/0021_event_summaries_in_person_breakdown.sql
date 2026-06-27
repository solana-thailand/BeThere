-- Plan 008 follow-up: distinguish in-person no-shows from online attendance.
--
-- Online events have no physical check-in; quest completion (virtual check-in)
-- is opt-in and joining the call is not recorded. Counting unchecked-in online
-- registrants as no-shows was misleading. `no_show_count` now reflects only the
-- in-person slice, and these two columns expose the basis so the UI can show a
-- correct no-show rate and distinguish online-only events.
--
-- Additive: two NOT NULL INTEGER columns with default 0. Existing frozen rows
-- get 0/0 (they were computed under the old, buggy logic and should be cleared
-- so the next GET refreezes — see the deploy notes for this change).

ALTER TABLE event_summaries ADD COLUMN in_person_registered_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event_summaries ADD COLUMN in_person_checked_in_count INTEGER NOT NULL DEFAULT 0;
