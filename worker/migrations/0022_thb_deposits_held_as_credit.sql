-- Issue #061: THB hold-deposit idempotency.
-- Adds `held_as_credit` to thb_deposits so `POST /api/deposit/hold` can settle
-- the source deposit and reject re-calls (prevents double-credit).
--
-- Distinct from `refunded`: `refunded` releases funds back to the attendee,
-- while `held_as_credit` retains funds as organizer liability (rolling credit
-- the attendee spends on a future event registration). Keeping them separate
-- avoids polluting the refund reconciliation view and the attendee-facing
-- RefundCard, both of which key off `refunded`.
--
-- Column conventions mirror `refunded` / `refunded_at` from migration 0013:
--   - boolean as INTEGER NOT NULL DEFAULT 0
--   - timestamp as nullable TEXT (ISO 8601)
--
-- Applied once by the D1 migration tracker (wrangler d1 migrations apply).
-- SQLite has no ALTER TABLE ADD COLUMN IF NOT EXISTS; re-running manually will
-- error with "duplicate column name", which is the expected signal.

ALTER TABLE thb_deposits ADD COLUMN held_as_credit INTEGER NOT NULL DEFAULT 0;
ALTER TABLE thb_deposits ADD COLUMN held_as_credit_at TEXT;
