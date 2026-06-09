-- Issue 053 Phase 3f: Add form_config JSON column to events table.
-- Stores per-event registration form configuration (RegistrationFormConfig)
-- previously stored in KV under "event:{id}:form:config".

ALTER TABLE events ADD COLUMN form_config TEXT;
