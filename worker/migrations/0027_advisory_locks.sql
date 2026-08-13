-- Generic short-lived advisory locks for serializing non-atomic critical
-- sections that span an external store (e.g. Google Sheets, which has no CAS).
--
-- First use: per-email rolling-credit spend during registration
-- (docs/SECURITY-FINDINGS-2026-08-13.md #4) — two concurrent registrations by
-- the same email must not both read-then-decrement the same credit balance.
--
-- Acquire = INSERT ... ON CONFLICT DO UPDATE ... WHERE expires_at < now (steals
-- an expired lock); `meta().changes > 0` means acquired. Expired rows are
-- self-healing (stolen on next acquire); no background sweeper required.
CREATE TABLE IF NOT EXISTS advisory_locks (
    lock_key   TEXT PRIMARY KEY,
    expires_at TEXT NOT NULL
);
