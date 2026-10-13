# 193 — The ticket still says "Deposit Required" after the event

**Status:** deployed (prod `cdd8be9a`, version `15532abb`, 2026-10-13, pull 270 via release pull 271; session `event-checkin-11`): the ticket API sends `event_over` (`EventConfig::is_over`); the hero reads "This event has ended", and the deposit, reclaim and moved-online cards (in-person and online views) and the check-in polling stop once it is over.

## What happens

A registrant who never paid and never checked in opens a finished event
(`/e/{slug}`). Since pull 267 they follow the normal flow to their ticket,
which is right, but the ticket's banner reads **"Deposit Required"** and its
action card offers to pay. A deposit cannot be paid once the event is over
(`/deposit` for an ended event already says so: pull 247).

Repro (local fixture worker, `e2e-past`: ended, status active, deposit on,
with a recording): register `e2e-admin@example.com` with no deposit and no
check-in, open `/e/e2e-past-night` signed in, and you land on
`/ticket/{id}?event_id=e2e-past` with the hero "Deposit Required". The
recording (`VideoSection`) is shown correctly.

## Proposal

The ticket's state should use the same "over" rule as the event page
(`domain::models::event::is_over`, pull 264). Once the event is over:

- no deposit on file: the banner says the event has ended (not "Deposit
  Required"), with no pay action;
- the recording and the NFT claim (for someone checked in) stay as they are.

Check every place that decides the ticket's deposit state (the hero, the
action cards, the polling tier `AwaitingDeposit`). They are separate writers
of the same state, see memory `duplicated-state-transition-paths`.
