# 192 · Organizer pays held credit back without an attendee request

**Status:** in progress. The backend is on `feature/192-organizer-credit-payout`
(session `event-checkin-9c`, 2026-10-09): backend and staff UI. Not yet
merged or deployed.

## Why

The owner asked whether an admin can pay credit back without an attendee
request (2026-10-09). On `develop` the answer was no: held credit came back
only through `request-credit-refund`, followed by the organizer clearing the
request. Since `.issues/190`, the refund account the attendee typed with their
THB deposit is copied to `credit_refund_accounts` (`source = 'deposit'`). That
means the organizer already knows where to send the money.

## Design (built)

- `GET /api/deposit/credit-payout-candidates`: lists people who hold payable
  credit, have a deposit account that speaks for them, and have no open
  request on any linked email. The rows use the request queue's shape
  (`CreditRefundRequest`). The list is org-scoped like the queue.
- `POST /api/deposit/organizer-credit-payout {email, paid, proof}`:
  - the slip is **required**;
  - it pays only when the chosen account is `AccountSource::Deposit`;
  - it returns 409 when a request is open (the attendee chose an account, so
    pay it from the queue).
- Both payout paths share one core, `credit_payout::settle_payout`. It does
  the compare, then the slip, then the guarded `try_refund`, then a 409 on
  mismatch, then the audit. The clear handler now calls it too.
- **No migration.** The plan was for 0060 `credit_refund_requested_by` to
  open a request on the person's behalf. Instead, the reversal is keyed
  `organizer-{epoch ms}` and no flag is opened, so a refusal leaves nothing to
  roll back. A double click gets a new key against a zero balance, which is a
  409. The audit meta carries `initiated_by: organizer | attendee`.
- After a payout, the account row is deleted (best-effort; the nightly purge
  is the backstop).

## Tests

- `worker/tests/security/test_credit_payout.py`: 6 new cases on the real SQL
  (who is a candidate, person-wide open request, typed account excluded, no
  payable credit, linked emails as one row, a double payout refused). A
  mutation that dropped the `source = 'deposit'` filter turned the suite red.
- `credit_ledger_guards.rs` and `credit_payout_guards.rs` follow the core and
  pin the organizer handler's order: checks, scope, settle, then delete.

## Remaining

- [x] Staff UI: `CreditPayoutCandidates` under the request queue on the
      Held-as-Credit tab. It reuses `CreditRefundRow` with `PayoutRowKind::
      Unrequested` (badge "Not requested", no request age, and the button
      stays disabled until a slip is attached). It loads its own data, so
      `admin_deposit.rs` (999 lines) gains only the mount.
- [x] Attendee ticket card: "Credit Paid Back — the organizer sent you
      500 THB on 9 Oct 2026" (branch `feature/192-payout-receipt`, stacked
      on this one, session `event-checkin-d0`, 2026-10-09). The problem it
      fixes: the in-person view mounts the card on `held_as_credit`, so
      after an organizer payout the attendee was offered a refund of money
      already sent. `GET /api/deposit/credit-refund-request` gains
      `paid_back` (`db::credit_payout_receipt::SETTLED_PAYOUT_SQL`, read
      from the ledger, no migration). It is set only while the latest payout
      settled everything: no positive row after it, nothing locked. One
      payout is the person's `refund` rows after their last other row, not a
      `created_at` match, which is only to the second.
      `test_credit_payout_receipt.py` has 7 cases; three mutants (the lower
      bound, the newer-credit check, the lock check) each turn it red. It
      parses on SQLite 3.45.1. Staff shell +1.5 KB br4 (97.27%).
      **Not masked account:** the payout deletes the account row, so there
      is nothing to mask. Keeping a masked copy (bank + last four) in an
      append-only ledger is new retention; owner's call, not built.
- [ ] Fix the cramped payout form and the Buddhist-era date on English pages.
- [ ] Staging: run the payout end to end with a held deposit, and open the
      ticket page afterwards: the card must read "Credit Paid Back" (not yet
      seen in a browser).
