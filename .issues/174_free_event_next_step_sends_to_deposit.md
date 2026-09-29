# 174: A registered attendee of a free event was sent to "Complete Deposit"

**Status:** fixed on develop (2026-09-29, session `event-checkin-ba`). Not deployed. Found while building P2-a of `.plans/038` (the inline ticket on the landing).

## What happened

`build_next_step_from_presence` (`worker/src/handlers/register/my_registration.rs`)
decided an in-person attendee's next step from whether a real deposit
exists. It never looked at whether the event takes one. On a free event
(`deposit_enabled = 0`) nobody has a deposit row, so every in-person attendee
got:
- `next_step = {type: "deposit", url: "/deposit/{id}?…"}`;
- on the landing's "Your events", a **"Complete Deposit →"** button;
- on `/e/{slug}`, the registered-visitor redirect, which follows `next_step`,
  pointed at the deposit page;
- the same answer from `POST /api/public/register` for a returning
  registrant.

The deposit page for an event with no amounts then shows its "no payment
methods" message: a dead end for a ticket that was already valid.

Reproduced locally (`e2e/fixtures/seed.sql`, event `e2e-free`): before the
fix the card said "Complete Deposit"; after it, "View Ticket →", and
`/api/my-registrations` returns `type: "ticket"`.

## Fix

- The rule moved to `domain::models::next_step::next_step_kind`, a pure
  function over `NextStepFacts` with a new `deposit_required` fact, the
  event's own `deposit_enabled`. The worker turns the kind into a URL.
- All five call sites pass `config.deposit_enabled`: the two in
  `my_registration.rs`, the D1 path via `e.deposit_enabled` in
  `sql/my_registrations.sql`, and both in `signup.rs`.
- Checked-in → claim, claimed → ticket, online → waiting, and the
  orphaned-USDC → deposit retry behave as before.

**Tests:** `domain/tests/next_step_kind.rs` (4). The negative control is the
same attendee on a deposit event, who is still sent to pay.

## Not checked

- How many prod events are free and have in-person attendees affected. That
  needs a prod read, which is owner-run.
- Whether the frontend has its own copy of this rule elsewhere, for example
  the ticket page's deposit callout. The earlier ticket-page audit
  (`.issues/173` C7) saw a "Deposit Required" callout only on a deposit
  event.
