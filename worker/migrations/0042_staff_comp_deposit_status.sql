-- Staff / organizer comps get the deposit status every attendee-facing reader
-- routes on.
--
-- The comp at registration wrote only a ฿0 `thb_deposits` row
-- (`STAFF_COMP_WAIVED`). The next-step router, ticket page and deposit deadline
-- all read `deposit_statuses`, so a comped organizer landed on the ticket once
-- and on the deposit page on every later visit. Registration now writes both
-- (`handlers::register::signup::record_staff_comp`); this backfills the comps
-- recorded before that. One row in prod on 2026-09-17 (RTM #6).
--
-- ฿0, verified, non-refundable, method 'thb' — the same shape the new writer
-- saves. `DO NOTHING` keeps any status that already exists (a real payment is
-- never overwritten), and the `WHERE` is required by SQLite's upsert-on-select
-- parse rule.
INSERT INTO deposit_statuses
    (attendee_id, event_id, method, amount, currency, verified, deposited_at,
     deposit_order, refundable, rejected)
SELECT d.attendee_id, d.event_id, 'thb', 0, 'THB', 1,
       COALESCE(d.verified_at, d.uploaded_at), 0, 0, 0
FROM thb_deposits d
WHERE d.slip_url = 'STAFF_COMP_WAIVED'
ON CONFLICT (event_id, attendee_id) DO NOTHING;
