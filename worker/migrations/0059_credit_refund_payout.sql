-- 0059_credit_refund_payout.sql — .issues/190 (owner option (a), 2026-10-08).
--
-- Where an attendee wants their held rolling credit paid back to. One row per
-- contact email; the organizer's payout queue and the attendee's masked
-- preview read it.
--
-- Two sources (`source`):
--   * 'deposit'  — the refund account the attendee typed on their THB deposit
--                  slip upload (`thb_deposits.bank_*`, validated by
--                  `validate_bank_refund_fields`). Snapshotted when the deposit
--                  is held as credit (`thb_deposits::try_settle_hold_credit`)
--                  and backfilled below, because `thb_deposits` rows are
--                  deleted 90 days after the event while credit can live
--                  longer. `source_deposit_ref` is the deposit's ledger key,
--                  `{event_id}:{attendee_id}` — event-scoped, since attendee
--                  ids are global. `captured_at` is the deposit's upload time.
--   * 'attendee' — an account the attendee entered on the credit refund card.
--                  Never overwritten by a later hold (their explicit choice
--                  wins). `replaced_deposit_account = 1` when it differs from
--                  the deposit account it replaced, so the organizer is told
--                  to confirm before paying.
--
-- A table of its own rather than columns on `contacts`:
--   * the row IS the personal data — deleting it at payout, in the nightly
--     purge, and on PDPA erasure is one DELETE;
--   * the CHECKs below only bind this table's own writers
--     (`db::credit_refund_accounts::{SAVE_SQL, SNAPSHOT_FROM_DEPOSIT_SQL}` and
--     the backfill below). On `contacts` they would bind every existing writer
--     too (memory: migration-constraints-break-existing-writers);
--   * no `event_id` column, so `event_purge` (which must cover every table with
--     one — worker/tests/security/test_event_purge.py) is unaffected.
--
-- Retention (docs/pdpa_ropa.md): kept while the person has an open request or
-- holds credit (payable, or locked to an event that has not returned it);
-- deleted at payout, by the nightly purge once neither holds, and on PDPA
-- erasure.
--
-- Account numbers are never logged; the attendee API returns a masked preview
-- only, and the staff payout queue is the only reader of the full number.
--
-- Apply BEFORE the code deploy that writes it, then read the schema back:
--   PRAGMA table_info(credit_refund_accounts);
CREATE TABLE IF NOT EXISTS credit_refund_accounts (
    email        TEXT PRIMARY KEY,               -- lowercased contacts.email
    method       TEXT NOT NULL CHECK (method IN ('promptpay', 'bank')),
    promptpay_id TEXT,                           -- digits only
    bank_name    TEXT,
    bank_account TEXT,
    account_name TEXT,
    source       TEXT NOT NULL CHECK (source IN ('deposit', 'attendee')),
    source_deposit_ref TEXT,                     -- '{event_id}:{attendee_id}'
    captured_at  TEXT NOT NULL,                  -- deposit upload / attendee entry
    replaced_deposit_account INTEGER NOT NULL DEFAULT 0
                 CHECK (replaced_deposit_account IN (0, 1)),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (
        (method = 'promptpay' AND promptpay_id IS NOT NULL
            AND bank_name IS NULL AND bank_account IS NULL AND account_name IS NULL)
        OR
        (method = 'bank' AND promptpay_id IS NULL
            AND bank_name IS NOT NULL AND bank_account IS NOT NULL
            AND account_name IS NOT NULL)
    ),
    CHECK (
        (source = 'deposit' AND source_deposit_ref IS NOT NULL
            AND replaced_deposit_account = 0)
        OR
        (source = 'attendee' AND source_deposit_ref IS NULL)
    )
);

-- Backfill: everyone who holds credit now gets the account from their most
-- recent held deposit (latest `held_as_credit_at`) that still has a complete
-- refund account.
--
-- The mapping is the one `SNAPSHOT_FROM_DEPOSIT_SQL` applies at hold time
-- (`test_credit_payout.py` runs both on the same deposits and compares):
--   * all three fields non-blank after trim, else no row;
--   * a bank name of "PromptPay" / "พร้อมเพย์" (the deposit form's bank name is
--     free text) becomes method 'promptpay' when the number is a valid
--     PromptPay ID (10 digits from 0, or 13 digits; spaces and dashes
--     dropped), and no row otherwise;
--   * anything else is method 'bank', fields trimmed.
-- The email comes from the attendee row matched on BOTH id and event (attendee
-- ids are global). "Holds credit" is the purge's retention test — a positive
-- ledger bucket, or credit applied to an event and not yet returned, over the
-- person's linked emails — so the purge never deletes a fresh backfill row.
-- No compound SELECT beyond the two-term person set every credit query uses
-- (remote D1 caps UNION terms). Idempotent: DO NOTHING.
INSERT INTO credit_refund_accounts
    (email, method, promptpay_id, bank_name, bank_account, account_name,
     source, source_deposit_ref, captured_at, replaced_deposit_account, updated_at)
SELECT r.email,
       CASE WHEN r.pp THEN 'promptpay' ELSE 'bank' END,
       CASE WHEN r.pp THEN r.digits END,
       CASE WHEN r.pp THEN NULL ELSE r.bn END,
       CASE WHEN r.pp THEN NULL ELSE r.ba END,
       CASE WHEN r.pp THEN NULL ELSE r.an END,
       'deposit', r.ref, r.uploaded_at, 0, datetime('now')
FROM (
    SELECT LOWER(TRIM(a.email)) AS email, x.*,
           ROW_NUMBER() OVER (
               PARTITION BY LOWER(TRIM(a.email))
               ORDER BY x.held_as_credit_at DESC, x.id DESC
           ) AS rn
    FROM (
        SELECT d.id, d.event_id || ':' || d.attendee_id AS ref,
               d.event_id, d.attendee_id, d.uploaded_at,
               COALESCE(d.held_as_credit_at, '') AS held_as_credit_at,
               TRIM(COALESCE(d.bank_name, '')) AS bn,
               TRIM(COALESCE(d.bank_account, '')) AS ba,
               TRIM(COALESCE(d.account_name, '')) AS an,
               REPLACE(REPLACE(TRIM(COALESCE(d.bank_account, '')), ' ', ''), '-', '') AS digits,
               LOWER(REPLACE(TRIM(COALESCE(d.bank_name, '')), ' ', ''))
                   IN ('promptpay', 'พร้อมเพย์') AS pp
        FROM thb_deposits d
        WHERE d.held_as_credit = 1 AND d.refunded = 0
    ) x
    JOIN attendees a ON a.id = x.attendee_id AND a.event_id = x.event_id
    WHERE x.bn <> '' AND x.ba <> '' AND x.an <> ''
      AND TRIM(a.email) <> ''
      AND (NOT x.pp
           OR (x.digits NOT GLOB '*[^0-9]*'
               AND (LENGTH(x.digits) = 13
                    OR (LENGTH(x.digits) = 10 AND x.digits GLOB '0*'))))
) r
WHERE r.rn = 1
  AND (EXISTS (SELECT organization_id, currency, SUM(delta) AS balance
               FROM credit_ledger
               WHERE email IN (SELECT r.email AS email UNION SELECT m.email FROM person_emails o
                               JOIN person_emails m ON m.person_id = o.person_id
                               WHERE o.email = r.email)
               GROUP BY organization_id, currency HAVING SUM(delta) > 0)
       OR EXISTS (SELECT 1 FROM credit_ledger l
                  WHERE l.reason = 'apply' AND l.delta < 0
                    AND l.email IN (SELECT r.email AS email UNION SELECT m.email FROM person_emails o
                                    JOIN person_emails m ON m.person_id = o.person_id
                                    WHERE o.email = r.email)
                    AND NOT EXISTS (SELECT 1 FROM credit_ledger rt
                                    WHERE rt.reason = 'return' AND rt.event_id = l.event_id
                                      AND rt.email = l.email)))
ON CONFLICT (email) DO NOTHING;
