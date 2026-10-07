-- Hard delete of an event: the rows that belong to it (.issues/187).
-- One statement per table, children first, each bound to ?1 = event id, run
-- as one D1 batch (all or nothing) by db/event_purge.rs AFTER the deposits'
-- amounts are archived (archive_thb_deposits_for_event) and BEFORE the
-- events row goes. Statements are separated by a line holding only ";".
--
-- Kept on purpose (EVENT_PURGE_KEEPS in event_purge.rs): audit_log (the trail
-- of who did what), credit_ledger (a person's balance, not the event's),
-- thb_deposit_archive (the money record), onchain_events, escrow_index and
-- nft_mint_jobs (mirrors of on-chain state). worker/tests/security/
-- test_event_purge.py fails if a migration adds an event_id table that is in
-- neither list.
DELETE FROM slip_proposals WHERE event_id = ?1
;
DELETE FROM deposit_statuses WHERE event_id = ?1
;
DELETE FROM thb_deposits WHERE event_id = ?1
;
DELETE FROM registration_responses WHERE event_id = ?1
;
DELETE FROM attendance_answers WHERE event_id = ?1
;
DELETE FROM quiz_progress WHERE event_id = ?1
;
DELETE FROM adventure_progress WHERE event_id = ?1
;
DELETE FROM notification_outbox WHERE event_id = ?1
;
DELETE FROM notification_enrollments WHERE event_id = ?1
;
DELETE FROM claim_locks WHERE event_id = ?1
;
DELETE FROM campaign_events WHERE event_id = ?1
;
DELETE FROM staff WHERE event_id = ?1
;
DELETE FROM quiz_configs WHERE event_id = ?1
;
DELETE FROM adventure_configs WHERE event_id = ?1
;
DELETE FROM attendees WHERE event_id = ?1
