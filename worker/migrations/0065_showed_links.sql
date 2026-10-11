-- 0065_showed_links.sql
-- "I showed up" share links (Super GOAT SG4). A person who came through the
-- door of an event (domain::models::record::door_history) can make a public
-- link, /showed/{token}, that names the event and nothing about them. The
-- token is random (32 hex), carries no id, and revoking it kills the page.
--
-- One live link per (email, event): asking again returns it. PDPA erasure
-- and the event purge delete the rows.
CREATE TABLE IF NOT EXISTS showed_links (
  token TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  event_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_showed_links_email ON showed_links(email, event_id);
