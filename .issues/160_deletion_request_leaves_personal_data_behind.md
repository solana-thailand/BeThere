# 160: A deletion request leaves personal data behind

**Status:** in progress (2026-10-13, session `event-checkin-d3`): the silent R2 failures are fixed on develop (below); the per-table decisions are proposed below and wait on the owner. Opened 2026-09-28. Found while writing `docs/pdpa_ropa.md` (§15–16); the list comes from that review of `worker/src/handlers/privacy.rs`.

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


## 2026-10-13: re-run of the review, the R2 fix, and a proposal

**Re-run (develop `673a70e3`+):** still true. One more table found since the
issue was written: `credit_refund_accounts` (`.issues/190`) holds email,
PromptPay id, bank name, account number and account name, and the deletion
request does not touch it.

**Fixed (this change):** the slip and refund-image erases no longer drop
errors. A failed existence check or delete is recorded as a failure
(`delete_slip_image` / `delete_refund_image`), so the reply says `partial`
instead of reporting the image erased (`erase_r2_object`,
`worker/tests/privacy_erase_guard.rs`).

**Proposal per table, for the owner to decide** (not legal advice; Thai PDPA
allows keeping what a legal duty such as accounting requires, and asks that
the rest be erased):

| Data | Holds | Proposal | Why |
|---|---|---|---|
| `thb_deposits` bank_account, bank_name, account_name, attendee_name | bank details, name | **Erase** (set NULL) once the deposit is refunded or held as credit; if a refund is still pending, erase right after it is paid | needed only to pay the refund |
| `thb_deposits` amount, verified, refunded, dates | money facts | **Keep** | accounting; no identity once the above is gone |
| legacy `attendees.bank_*` | bank details | **Erase** | not used by current flows |
| `credit_refund_accounts` | bank / PromptPay details | **Erase** if the person holds no credit; if credit is still owed, keep until it is paid out, then erase | the clearest bank-data case |
| `credit_ledger.email` | email | **Pseudonymise** (keyed fingerprint) when the balance is 0; **keep** while credit is owed, and say so in the reply | money owed to the person |
| `person_emails` | linked emails | **Erase** | identity links only |
| `deposit_statuses.wallet_address`, `claim_locks.wallet` | wallet address | **Erase** the off-chain link | the chain record stays regardless |
| `nft_mint_jobs.wallet` | wallet address | **Keep** | the badge is public on chain already; erasing the job row changes nothing on chain |
| `audit_log` actor/target/description | staff emails; sometimes an attendee email in text | **Pseudonymise** the person's email where it appears | keep the audit trail, drop the identity |
| `thb_deposit_archive` | amounts only | **Keep** | no personal data |
| sign-in log and waitlist Sheets | email, name | **Clear** the person's rows | same as the event Sheet today |

Also proposed: the privacy notice should say plainly that on-chain records
(deposits, badges) cannot be erased, only unlinked from the person off chain.

Once the owner decides, the handler gets one step per row above, each
recording its failure like the R2 erase now does, and `docs/pdpa_ropa.md`
records the outcome.
