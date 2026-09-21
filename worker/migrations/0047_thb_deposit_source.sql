-- 0047_thb_deposit_source.sql — `.issues/129` Gap 1
--
-- Approving a payment slip is the ONLY way an attendee receives their ticket QR
-- (`slip_verify.rs`, approval-only auto-QR), and approving is simultaneously the
-- promise to refund their ฿500. So an organizer who knows somebody did not
-- really pay has exactly two moves: admit them and owe them ฿500, or refuse
-- them entry. There is no third state, which is why the knowledge of who
-- bypassed lives in the organizer's memory instead of in the system.
--
-- The state that should carry it already exists — `DepositSource::Comp`, which
-- every refund/hold/roll rule already honours through `is_non_cash()` — but it
-- can only be created at signup, for staff and organizers
-- (`register/signup::record_staff_comp`), and it is *detected* by sniffing
-- sentinels out of two columns that mean other things:
--
--     verified_by = 'SYSTEM_STAFF_WAIVE'   slip_url = 'STAFF_COMP_WAIVED'
--     verified_by = 'SYSTEM_ROLLING_CREDIT' slip_url = 'ROLLING_CREDIT_AUTO_APPLIED'
--     amount_thb = 0
--
-- A real slip cannot be reclassified without destroying the record of what was
-- uploaded (blanking slip_url) or what was claimed (zeroing amount_thb). This
-- column makes the classification a fact the row states outright.
--
-- BACKFILLED FROM THE EXISTING CLASSIFIER, EXACTLY.
-- The three WHEN arms below are a transcription of `ThbDeposit::source()` in
-- domain/src/models/deposit.rs, in the same order, because order is load-bearing:
-- a row that is both credit-applied AND ฿0 classifies as Credit, not Comp. Every
-- existing row therefore keeps precisely the classification it had yesterday —
-- `deposit_source_backfill_matches_the_legacy_classifier` in
-- worker/tests/deposit_source_guards.rs asserts that both ways.
--
-- NULLABLE with a CHECK, not NOT NULL with a default: a row whose source has
-- never been decided must be distinguishable from one decided as 'cash'. The
-- Rust classifier reads the column when set and falls back to the sentinels when
-- NULL, so the sentinels keep working for anything this backfill misses and for
-- any writer not yet updated.
--
-- No table rebuild: SQLite ADD COLUMN is O(1) metadata, safe to apply days
-- before RTM #6 (2026-09-27). The UNIQUE that does need a rebuild is `.issues/127`,
-- deferred to 0048.

ALTER TABLE thb_deposits ADD COLUMN deposit_source TEXT
    CHECK (deposit_source IS NULL OR deposit_source IN ('cash', 'credit', 'comp'));

UPDATE thb_deposits
SET deposit_source = CASE
    WHEN verified_by = 'SYSTEM_ROLLING_CREDIT' OR slip_url = 'ROLLING_CREDIT_AUTO_APPLIED'
        THEN 'credit'
    WHEN verified_by = 'SYSTEM_STAFF_WAIVE' OR slip_url = 'STAFF_COMP_WAIVED' OR amount_thb = 0
        THEN 'comp'
    ELSE 'cash'
END
WHERE deposit_source IS NULL;

-- The comp list is read per event on the admin screen and by the refund queue.
CREATE INDEX IF NOT EXISTS idx_thb_deposits_source
    ON thb_deposits(event_id, deposit_source);
