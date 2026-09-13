-- Supports stable keyset pagination for the authenticated event listing.
CREATE INDEX idx_events_created_id ON events(created_at DESC, id DESC);
