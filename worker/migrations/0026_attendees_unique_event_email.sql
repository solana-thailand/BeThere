-- Migration 0026: prevent duplicate self-registrations per event.
--
-- The registration dedup check is an advisory in-memory scan of the Sheets
-- attendee list (register/signup.rs), with no backing constraint — so a race
-- (double-click, concurrent devices, retry within the read window) could create
-- two attendee rows for the same person, inflating capacity/counts and minting
-- multiple claim tokens. This adds the missing backstop.
--
-- Partial + expression unique index:
--   - excludes walk-ins (`participation_type = 'walkin'`), which have their own
--     insert path and may legitimately coexist with a pre-registration.
--   - LOWER(email) so case variants can't slip a duplicate through.
--
-- Verified 0 existing violating rows in prod before adding (2026-08-12).
CREATE UNIQUE INDEX IF NOT EXISTS idx_attendees_unique_event_email
    ON attendees (event_id, LOWER(email))
    WHERE participation_type <> 'walkin';
