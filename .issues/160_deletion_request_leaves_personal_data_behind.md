# 160: A deletion request leaves personal data behind

**Status:** open (2026-09-28). Found while writing `docs/pdpa_ropa.md` (§15–16); the list comes from that review of `worker/src/handlers/privacy.rs`. Re-run the review before fixing.

## What happens

`POST /api/privacy/delete-request` clears attendee, contact and developer PII
and the event Sheet cells, but does not touch: `thb_deposits` (bank account and
name), `credit_ledger` (email), `person_emails`, `deposit_statuses`,
`claim_locks`, `nft_mint_jobs`, `audit_log`, `contacts.email`, or the
sign-in-log and waitlist Sheets. Its R2 slip/refund-image deletes ignore
errors (`let _ = crate::storage::delete(..)`, `privacy.rs:182,191`), so a
failed delete still reports success.

Some of these may have to be kept (deposit amounts for accounting, the credit
ledger for money still owed); which ones is a legal question for the owner.
Bank details (`thb_deposits`, the legacy `attendees.bank_*` columns and the
event Sheet) are the clearest case to erase once a refund is settled.

## Fix

Decide per table: erase, pseudonymise, or keep with a stated reason. Then make
the handler do it, fail loudly on R2 errors, and record the outcome in the RoPA.
Related: plan 029 §3 retention and field-level encryption.
