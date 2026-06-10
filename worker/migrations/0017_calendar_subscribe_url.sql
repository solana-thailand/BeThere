-- Add calendar subscribe URL for organization calendar integration.
-- When set on an event, the ticket page shows a "Subscribe to our calendar" link.
ALTER TABLE events ADD COLUMN calendar_subscribe_url TEXT DEFAULT '';
