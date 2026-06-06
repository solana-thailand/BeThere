-- Issue #049: Add dev_profile_enabled flag to events table.
-- Allows organizers to toggle the developer profile section on the registration form.

ALTER TABLE events ADD COLUMN dev_profile_enabled INTEGER NOT NULL DEFAULT 0;
