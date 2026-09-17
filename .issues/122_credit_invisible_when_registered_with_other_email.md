# 122 — Rolling credit is invisible when the attendee registers with a different email

**Status:** open. The design gap is confirmed from prod data; the fix is an owner decision.
**Found:** 2026-09-17, when the owner reported that Apply Credit could not be
selected for one attendee on the RTM #6 in-person admin roster
**Severity:** medium (a paying returner is stranded at the deposit step, and
staff have no in-app way to fix it; no money is lost)

## What happened

The attendee holds ฿500 of rolling credit. The prod `credit_ledger` shows a
hold on RTM #4, an apply on RTM #5, and a return on RTM #5 on 2026-09-17, so the
balance is ฿500. All of it sits under their **personal Gmail**.

They registered for RTM #6 (event id `…-5-bangkok-copy`) with a **work
email** on a different domain. The row is in person, `deposit_status = none`,
and there is no `thb_deposits` row.

Credit is keyed by email at every layer:

- `worker/src/handlers/attendee/list.rs` annotates `credit_thb` by looking up
  the row's own email in `thb_balances_by_email`. The work email has no ledger
  rows, so `credit_thb = 0`.
- `frontend-leptos/src/pages/admin.rs` sets `can_apply_credit = in_person &&
  credit_thb > 0 && …`. The result is no credit badge and no Apply Credit
  button, with no explanation.
- `admin_apply_credit_handler` spends from `attendee.email`. Even if it were
  called directly, it would return `insufficient rolling credit: balance ฿0`.
- Signup auto-apply (`register/signup.rs` 5c) has the same email identity gate.

This is the intended identity rule (credit belongs to one verified email), not a
decoding bug. The gap is that one person with two emails has no supported path,
and the roster gives no hint of the cause.

## Options

- **A. Attendee re-registers with the credited email.** No code or data change;
  signup auto-applies. Cost: the attendee has to act, and the work-email row
  must be cancelled so it doesn't count twice.
- **B. One-off ledger transfer (owner go, prod money write).** Two
  `reason='adjust'` rows move −500 from the personal email and +500 to the work
  email, each with a note naming the approver. Then Apply Credit shows up
  normally. Cost: a hand-written D1 write on the money ledger.
- **C. Admin "transfer credit between emails" action (feature).** An audited
  admin endpoint writes the paired `adjust` rows atomically, and requires proof
  that both emails belong to the same person (for example, a Google session on
  both, or a bound wallet). Cost: build time, plus an abuse surface, because
  staff could move anyone's credit.

Recommendation: **A** now if the attendee is reachable before the event,
otherwise **B**. Build **C** only if this happens again.

Cheap, ungated improvement either way: when `credit_thb = 0`, the roster could
flag a row whose *name* matches a credit holder under another email, so staff
see why instead of a missing button.

## Related

- [120](120_credit_withdrawal_path_hard_to_reach.md): the credit withdrawal path
- `docs/deposit-commitment-model.md`: credit rules (D1–D5)
