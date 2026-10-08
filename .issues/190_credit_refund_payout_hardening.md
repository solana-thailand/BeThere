# 190 · Held-credit payout: stale amount, unguarded reversal, no account, no audit

**Status:** in progress. Built on `feature/190-credit-refund-payout` (from
`develop` `4d9cf386`, session `event-checkin-8a`), not merged, not deployed.
Owner chose option (a), "harden the existing credit cash-out flow", on
2026-10-08.

## Defects (verified on `develop` `4d9cf386`)

1. **The clear reverses a number nobody confirmed.**
   `ClearCreditRefundRequest { email }`
   (`worker/src/handlers/deposit/thb/handlers/hold_refund_request.rs:267`)
   carried no amount. `reverse_held_credit` (`:292`) wrote `-bucket.balance`
   (`:312`) for whatever `positive_balances` (`:396`) read **at clear time**,
   not what the organizer saw in the queue and transferred. If the balance
   moved in between (a return landed, a registration spent some), the ledger
   recorded a payout of a different amount from the one paid.
2. **The reversal write was unguarded.** `credit_ledger::record`
   (`worker/src/db/credit_ledger.rs:96`) is a plain `INSERT … VALUES`,
   idempotent only on `(deposit_id, reason)` (`:112`). Unlike `try_spend`
   (`:149`), nothing tied the written amount to the balance at write time, so a
   registration spend racing a clear could drive the person's balance negative
   (the reversal read ฿500, the spend took ฿500, the reversal still wrote
   −฿500).

Plus the gaps option (a) closes: the attendee gave no payout account, so the
organizer had to ask out-of-band; nothing recorded who paid
(`hold_refund_request.rs:172`, "Audit entry deferred"); the queue was readable
by every authenticated staff member (per-event scanners included); and
`docs/deposit-refund-flows.md:271` said credit is paid via `/refund/mark`,
which refuses held deposits by design.

## Fix

- **One guarded writer of `refund` rows.** `credit_ledger::try_refund` runs
  `TRY_REFUND_SQL`: one `INSERT … SELECT` that reverses every positive bucket of
  the person, `-balance` per bucket as read in the same statement, **only if**
  the THB and USDC totals equal what the organizer confirmed (`?3`, `?4`) and no
  bucket is in a currency that cannot be confirmed. 0 rows → `AlreadyRecorded`
  (the request key `refund:{email}:{requested_at}:…` exists — a double clear)
  or `Mismatch` (→ 409, nothing written). The bucket read is one macro,
  `positive_buckets_of!`, shared with `positive_balances`, so the number the
  organizer confirms and the number reversed have one definition.
  `record()` now refuses `reason = 'refund'`. Writers of `refund` found: one
  (`reverse_held_credit` → `record`), now `try_refund`; no SQL elsewhere
  inserts the reason (guard below).
- **The clear carries the amount.** Body `{email, paid: {thb, usdc}, proof?}`
  (`PaidAmounts`, integers in the ledger's unit — whole baht, the unit of
  `amount_thb` and every THB ledger row). Mismatch → 409 with
  `payout_mismatch_message` (no `://`), before and inside the write.
- **Payout account at request time.** The ticket card asks for a PromptPay ID
  (10-digit mobile from 0, or 13-digit ID) or a bank account. The bank shape is
  the THB deposit refund account's (`bank_name`, `bank_account`,
  `account_name`, all required): `domain::models::credit_payout::
  validate_bank_refund_fields` is now what `slip_upload.rs` and
  `slip_admin_upload.rs` call too, with the same messages. Stored in
  `credit_refund_accounts` (migration `0059`) in one D1 batch with the flag.
- **Organizer view, org-scoped.** The queue shows the account, the age against
  the 7-day promise (D3), and a "Record payout" form (amount + optional slip).
  `payout_scope`: super-admin all; global admin/organizer the default org plus
  owned orgs; org owners their orgs; scanners 403. A row is listed only if every
  org of the person's ledger rows is in scope; the clear re-checks against the
  buckets it reverses.
- **Proof + audit.** The slip (image, magic bytes, ≤ 3 MB, `validate_slip_url`)
  goes to R2 `credit-payouts/{org}/{blake3(email)[..16]}/{requested_at digits}`
  (served staff-only at `/api/storage/credit-payouts/…`); a failed upload fails
  the clear before anything is written. `AuditAction::CreditRefundPaidOut` in
  the global audit log: actor = staff email, target = contact, metadata =
  amounts, `requested_at`, slip path. A failed audit aborts before the flag
  clears (the retry is a ledger no-op and re-audits).
- **Account retention.** Deleted with the flag clear (same batch), by the
  nightly cleanup (`credit_refund_accounts::purge`: older than 90 days or
  request no longer open; runs before the event-index read that can abort the
  pass), and by PDPA erasure (`clear_contact_pii`). Never logged.
- **7-day report.** `scripts/verify/refund_window_report.py` lists open credit
  requests and exits 1 on one open more than 7 days (backlog before
  2026-09-28 excepted); contacts appear as a BLAKE2b reference, never an email.
- **Docs.** `docs/deposit-refund-flows.md` §4.2 describes the real flow.

### Schema choice

A table (`credit_refund_accounts`, PK `email`) rather than columns on
`contacts`: the row *is* the personal data, so deleting it is one statement;
its `CHECK`s (method in `promptpay|bank`, exactly that method's fields
non-NULL) bind only its one writer, where on `contacts` they would bind six
existing writers (memory: migration-constraints-break-existing-writers); and
it has no `event_id`, so `event_purge`'s coverage test is unaffected.

### `.issues/124` (partial payouts)

Not fixed here. A partial payout (payable ฿300, ฿500 locked) still pays the
payable part and closes the request. The fix #124 prefers (re-stamp and keep
the request open) would keep the account number after a payout, which this
issue deletes at payout; choosing between the two is a policy call, not a
by-product of defects 1–2.

## Guards

- `worker/tests/security/test_credit_payout.py` (real SQL, full migration
  chain): stale amount writes nothing; spend→clear and clear→spend never go
  negative; double clear is a no-op; people, orgs and currencies isolated;
  wildcard emails; account saved only with an open request, replaced on
  re-request, schema CHECKs, queue columns, deleted at payout (incl. linked
  sibling), by the purge (old + closed only) and by erasure.
- `worker/tests/credit_payout_guards.rs`: one writer of `refund` rows; the
  guard terms of `TRY_REFUND_SQL`; clear order (amount → compare → guarded
  write → 409 → audit → clear); both endpoints org-scoped; account deleted with
  the request / in cleanup before the index read / on erasure; no tracing call
  names account fields; the slip key carries no email.
- `worker/tests/credit_ledger_guards.rs` updated (person scope now includes
  `try_refund` and `positive_buckets_of!`; the queue resolves the person four
  times).
- `domain/tests/credit_payout.rs`, `frontend-leptos/tests/credit_payout_form.rs`.
- `refund_window_report.py --self-test` (6 checks, incl. a clean control).

## Deploy order (owner-gated)

1. Apply `0059_credit_refund_payout.sql` to staging, then prod, **before** the
   code deploy (`deploy.sh` does not apply migrations). The old code never
   touches the new table, so applying early is safe.
2. Read the schema back: `PRAGMA table_info(credit_refund_accounts);` and
   `SELECT sql FROM sqlite_master WHERE name = 'credit_refund_accounts';`.
3. Deploy the worker and the frontend together: the new clear needs `paid`, so
   an old admin page gets a 400 ("confirm the amount you transferred…"), and an
   old ticket page posting `{}` gets a 400 asking for an account.
4. After the deploy, check write volume on every table this touches:
   `credit_refund_accounts`, `contacts` (`credit_refund_requested_at`),
   `credit_ledger` (`reason = 'refund'`), `audit_log` (`action =
   'credit_refund_paid_out'`) — a request and a payout on staging first.

## Not verified

- No browser check of the request card or the payout row yet (see the
  session report), no staging run, and `pii_log_probe.sh` does not drive these
  endpoints (its request list predates them).
