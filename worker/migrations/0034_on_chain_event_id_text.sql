-- 0034 — store `on_chain_event_id` exactly.
--
-- Issue 085. The column is a **u64** (it seeds the EventEscrow PDA), stored in
-- a signed-64-bit INTEGER column and read back through `JSON.stringify`. Two
-- independent precision bugs followed:
--
--   1. Write: a value above i64::MAX cannot be an INTEGER literal, so SQLite
--      silently stored it as REAL and the low digits were lost for good.
--   2. Read: every row arrives in Rust as a JavaScript number, so any value
--      above 2^53 is rounded — which is effectively all of them.
--
-- Both make the derived escrow PDA wrong, and every escrow transaction builder
-- derives that PDA from this value. TEXT is the only SQLite storage class that
-- holds a full u64 exactly and survives the JSON round trip unchanged.
--
-- This migration preserves whatever is currently stored, corruption included —
-- it cannot recompute FNV-1a in SQL. Repair is a separate, verifiable step:
--   python3 scripts/verify/onchain_event_id_audit.py --db <db> --repair
-- which recomputes the id and refuses to write one that does not derive to the
-- already-known `escrow_address`.

ALTER TABLE events ADD COLUMN on_chain_event_id_text TEXT NOT NULL DEFAULT '';

-- Carry the existing values across. `CAST(... AS TEXT)` renders an INTEGER
-- exactly; a REAL renders in scientific notation, which is precisely the
-- signature the audit script looks for when deciding a row needs repair.
UPDATE events
SET on_chain_event_id_text = CAST(on_chain_event_id AS TEXT)
WHERE on_chain_event_id IS NOT NULL AND on_chain_event_id != 0;
