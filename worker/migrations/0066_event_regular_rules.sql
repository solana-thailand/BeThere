-- 0066_event_regular_rules.sql
-- An event's regular rule (Super GOAT SG1b), opt-in: a person whose door
-- record (domain::models::record::door_history) shows at least
-- min_door_checkins holds a seat for deposit_thb. No row = the event
-- registers exactly as before; no event has a row after this migration.
--
-- Its own table, not an events column: read once at registration (D1), kept
-- out of the KV-cached event config, and a duplicated event starts without
-- it (an organizer opts in per event). deposit_thb > 0 is allowed by the
-- table but refused by the code until the deposit and refund paths carry a
-- reduced amount; today a qualifying seat is recorded as a comp. The event
-- purge deletes the row.
CREATE TABLE IF NOT EXISTS event_regular_rules (
  event_id TEXT PRIMARY KEY,
  min_door_checkins INTEGER NOT NULL CHECK (min_door_checkins BETWEEN 1 AND 100),
  deposit_thb INTEGER NOT NULL DEFAULT 0 CHECK (deposit_thb >= 0),
  updated_at TEXT NOT NULL
);
