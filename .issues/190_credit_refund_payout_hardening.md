# 190 · Held-credit payout: stale amount, unguarded reversal, no account, no audit

**Status:** deployed (2026-10-09, `event-checkin-8a`): prod version `4d0203f0` at main `71720705` (tree = develop `9d37fb8f`), migrations 0058 + 0059 applied to prod and staging; staging `b5ca12fd`. Earlier: in progress. Built on `feature/190-credit-refund-payout` (from
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
  nightly cleanup once the person has no open request and no held credit
  (below; runs before the event-index read that can abort the pass), and by
  PDPA erasure (`clear_contact_pii`). Never logged.
- **7-day report.** `scripts/verify/refund_window_report.py` lists open credit
  requests and exits 1 on one open more than 7 days (backlog before
  2026-09-28 excepted); contacts appear as a BLAKE2b reference, never an email.
- **Docs.** `docs/deposit-refund-flows.md` §4.2 describes the real flow.

### The account from the deposit (owner request 2026-10-08)

"The person already gave an account with their deposit — use that instead of
asking again." The THB slip upload stores a refund account on `thb_deposits`
(`bank_name`, `bank_account`, `account_name`, `validate_bank_refund_fields`).
`thb_deposits` rows are purged 90 days after the event, credit lives longer,
and `attendees.bank_*` (Sheets sync, provenance unclear) is not read. So:

- **Snapshot at hold time.** The one writer of `held_as_credit = 1`
  (`db::thb_deposits::try_settle_hold_credit`, reached from the attendee
  `/hold` and the admin hold) now takes the credit holder's email — the same
  email the ledger credits — and, only after its CAS won, runs
  `credit_refund_accounts::SNAPSHOT_FROM_DEPOSIT_SQL`: copy that deposit's
  account (matched on event id AND attendee id), `source='deposit'`,
  `source_deposit_ref='{event_id}:{attendee_id}'`, `captured_at` = the slip
  upload time. Best-effort: no usable account or a failed copy never fails the
  hold. It replaces an older `deposit` row and never an `attendee` row.
- **Mapping** (snapshot and backfill alike): all three fields non-blank after
  trim, else nothing. The deposit form's bank name is free text, so a bank
  name of "PromptPay"/"พร้อมเพย์" with a valid PromptPay ID becomes method
  `promptpay` (digits only); with an invalid ID, nothing (the attendee is asked).
- **One tap.** `GET /api/deposit/credit-refund-request` adds `saved_account`, a
  masked `SavedAccountPreview` (method, bank name, last four digits — none if
  the number has four or fewer — holder's first word and initial, `source`,
  `captured_at`). The full number never reaches the attendee API. The card
  says "We'll send it to KBank •••• 7890 · Somchai J. — the account from your
  deposit on 30 Aug 2026" with "Request refund to this account"
  (`{use_saved: true}`) and "Use a different account" (the form). The
  one-tap flag `UPDATE` sets only if the person has an account on file
  (`SET_FLAG_WITH_SAVED_ACCOUNT_SQL`); none → 400 and the form.
- **A different account** is stored `source='attendee'`;
  `replaced_deposit_account=1` when it differs from a deposit account of the
  person (sticky across re-requests). The queue shows "From deposit on
  <date>", "Entered by attendee", or the warning "Changed from the deposit
  account — confirm with the attendee before paying".
- **Which row speaks for a person** with linked emails:
  `chosen_account_email_of!` — attendee rows first, then latest — shared by
  the preview and the queue.
- **Retention** is no longer by age: `PURGE_SQL` deletes a row only when the
  person has no open request, no positive ledger bucket
  (`positive_buckets_of!`) and no unreturned apply (`unreturned_apply_of!`),
  i.e. payable + locked credit is 0. Delete-at-payout and PDPA erasure stay.
  `docs/pdpa_ropa.md` §6 records the rule; the `/privacy` notice (bank details
  for refunds kept until the person asks for deletion) already covers it and
  is unchanged.
- **Backfill in 0059** (edited in place — unmerged, never applied): for each
  email with held credit (the purge's own test, so the purge never deletes a
  fresh row), the account of the most recent held deposit (`held_as_credit=1`,
  latest `held_as_credit_at`) with a usable account; email from `attendees`
  joined on `id` AND `event_id`; `ON CONFLICT (email) DO NOTHING`; no compound
  SELECT beyond the two-term person set every credit query already uses.

Delete-at-payout is unchanged, so after a payout that leaves credit locked to
an event, the account is gone and the next request asks again (or a later hold
copies one). `.issues/124` owns that policy.

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
  sibling) and by erasure; the purge keeps a row while credit is payable,
  locked or requested (whole person) and deletes it at 0; the hold snapshot
  (event-scoped, held + unrefunded only, incomplete fields skipped, PromptPay
  mapping, newer deposit replaces older, never the attendee's row); the
  replaced flag; one-tap needs an account; the backfill (latest held deposit,
  id AND event join, people without credit skipped, idempotent) and its
  equivalence with the snapshot on 11 deposit shapes.
- `worker/tests/credit_payout_guards.rs`: one writer of `refund` rows; the
  guard terms of `TRY_REFUND_SQL`; clear order (amount → compare → guarded
  write → 409 → audit → clear); both endpoints org-scoped; account deleted with
  the request / in cleanup before the index read / on erasure; no tracing call
  names account fields (now also `thb_deposits.rs`, both hold handlers); the
  slip key carries no email; one writer of `held_as_credit = 1`, which copies
  the account after its flip, non-fatally, to the email the ledger credits on
  both hold paths; the snapshot's event scope and attendee precedence; the
  attendee status carries `SavedAccountPreview`, never `RefundAccount`; the
  purge is by balance, not age.
- `worker/tests/credit_ledger_guards.rs` updated (person scope now includes
  `try_refund` and `positive_buckets_of!`; the queue resolves the person four
  times).
- `domain/tests/credit_payout.rs` (incl. the masked preview: the serialized
  JSON lacks the full number and the surname),
  `frontend-leptos/tests/credit_payout_form.rs` (card sentence, label, queue
  badge).
- `refund_window_report.py --self-test` (6 checks, incl. a clean control).

## Deploy order (owner-gated)

1. Before applying: run the backfill's `SELECT` read-only on remote D1 —
   its window function and two-term `UNION` person set have not run on remote
   yet (memory: d1-compound-select-cap).
2. Apply `0059_credit_refund_payout.sql` to staging, then prod, **before** the
   code deploy (`deploy.sh` does not apply migrations). The old code never
   touches the new table, so applying early is safe. The migration also runs
   the backfill; holds made between the migration and the code deploy get no
   snapshot (the old code does not write one), so keep that window short or
   re-run the backfill `INSERT` from the file after the deploy (idempotent,
   `DO NOTHING`).
3. Read the schema back: `PRAGMA table_info(credit_refund_accounts);` and
   `SELECT sql FROM sqlite_master WHERE name = 'credit_refund_accounts';`, and
   count the backfill: `SELECT source, COUNT(*) FROM credit_refund_accounts
   GROUP BY source;` (expect only `deposit`).
4. Deploy the worker and the frontend together: the new clear needs `paid`, so
   an old admin page gets a 400 ("confirm the amount you transferred…"), and an
   old ticket page posting `{}` gets a 400 asking for an account.
5. After the deploy, check write volume on every table this touches:
   `credit_refund_accounts`, `contacts` (`credit_refund_requested_at`),
   `credit_ledger` (`reason = 'refund'`), `audit_log` (`action =
   'credit_refund_paid_out'`) — a request and a payout on staging first. After
   the first hold on staging, check a `deposit` row appeared.

## Not verified

- Verified locally (2026-10-08, `wrangler dev --local`, fresh state, no
  credentials, migrations 0001–0059 applied, headless Chrome): the attendee
  `POST /api/deposit/hold` wrote the `deposit` snapshot
  (`e2e-event:e2e-att-01`); `GET credit-refund-request` returned only the
  masked preview; the landing wallet card read "We'll send it to Kasikornbank
  (KBANK) •••• 7890 · Somchai E. — the account from your deposit on 30 Aug
  2026" (and the Thai text), with no account digits in the page HTML; one tap
  queued the request and the queue showed the full account with "From
  deposit on 30 Aug 2026"; "Use a different account" + a PromptPay ID stored
  `source='attendee'`, `replaced_deposit_account=1`, and the queue showed the
  red "Changed from the deposit account" warning. No account digits in the
  worker log.
- Not verified: the admin hold path at runtime (source guard only), the
  backfill on remote D1 (window function, `UNION` cap), any staging run, and
  `pii_log_probe.sh` does not drive these endpoints (its request list
  predates them).
