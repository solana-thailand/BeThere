-- 0046_thb_deposit_slip_hash.sql — `.issues/129` (the "anyone can upload any
-- image" half)
--
-- Nothing about an uploaded payment slip is checked. The deposit page accepts
-- any JPEG/PNG/WebP under 5 MB, and the organizer catches the people who did
-- not really pay by recognising them — which is why every refund still needs
-- them present, going down the list from memory.
--
-- This column is the smallest thing that moves that knowledge out of their head
-- and into the system: BLAKE3 of the decoded image bytes, so two attendees who
-- submit a byte-identical image become detectable. It catches the common case
-- (forwarding someone else's slip), not a re-screenshot, and it says nothing
-- about whether a payment actually happened — that needs the bank reference in
-- the slip's mini-QR, which needs a vendor account.
--
-- ADDITIVE AND NULLABLE, on purpose:
--
--   * Every row uploaded before today has no hash and must keep none. A
--     backfill would have to re-read 243 images out of R2 inside a migration,
--     and an invented value would be worse than an absent one.
--   * NULL means "not known", never "not a duplicate". The comparison in
--     `slip_fingerprint::duplicate_of` matches on `Some(hash)` only, so NULL
--     rows can never collide with each other — see `rows_with_no_hash_never_match`.
--   * Deliberately NOT UNIQUE. A rejected attendee re-uploading the same image
--     is legitimate, and the organizer must still be able to accept two
--     genuinely identical submissions if they decide to. Uniqueness here would
--     be enforced against the wrong thing; the duplicate decision belongs in
--     the handler, where it can tell "same person again" from "different
--     person, same image". (`.issues/127` is the opposite case — a UNIQUE that
--     genuinely should exist, on (event_id, attendee_id), and is still missing.)
--
-- No table rebuild: SQLite `ADD COLUMN` is O(1) metadata, so this is safe to
-- apply days before an event. The `(event_id, attendee_id)` UNIQUE that DOES
-- need a rebuild is deferred to 0048, after RTM #6 on 2026-09-27.

ALTER TABLE thb_deposits ADD COLUMN slip_blake3 TEXT;

-- Partial: only hashed rows are ever looked up, and skipping the NULLs keeps
-- the index at the size of "slips uploaded since 2026-09-22" rather than the
-- whole table.
CREATE INDEX IF NOT EXISTS idx_thb_deposits_slip_hash
    ON thb_deposits(slip_blake3)
    WHERE slip_blake3 IS NOT NULL;
