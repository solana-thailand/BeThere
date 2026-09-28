# 165: Public registration stays open after the event starts (and after it ends)

**Status:** open (2026-09-29). Found by session `event-checkin-83` during the `.issues/164` gap-2 rehearsal on staging. Changing it is the owner's call: it alters a prod registration path in the week of the 3 Oct deploy and RTM #6, and day-of sign-up at the door may be relied on.

## What happens

`POST /api/public/register` (`worker/src/handlers/register/signup.rs`, step 3)
checks only `config.status == EventStatus::Active`. It never looks at
`event_start_ms` or `event_end_ms`.

The documented rule is different. `EventConfig::is_registration_open`
(`domain/src/models/event/config.rs:497`) is
`status == Active && (event_start_ms == 0 || now < event_start_ms)`, and
`docs/tdd_ddd_architecture.md` (§ domain behaviour, table row "Event must be
Active for registration") and `.plans/014_policy_audit.md` both describe
registration as closing at the start. But no handler calls the method: its only
callers are the unit tests in `domain/src/models/event/tests.rs`.

## Repro (staging `a3d4e7a9`, 2026-09-28)

1. `BETHERE_DEMO_END_MIN=8 cargo run --example demo_fixture` in `bethere-mcp/`.
   Event `agent-demo-meetup-1790614521`: start `1790614941057` (00:02:21 +07),
   end `1790615001057` (00:03:21 +07).
2. At 00:02:32 +07, 11 s after the start, `register` through `bethere-mcp`
   returned 200 and created attendee `01a0e8f7-d76a-77f2-b189-d7259dca0d93`,
   with next step `deposit`.

Nothing in the handler stops the same call after the end. Deposits do stop:
`POST /api/deposit/usdc` refuses once `now > event_end_ms`
(`deposit/usdc/handlers/initiate.rs:59`), so a late registrant on a deposit
event gets a ticket but can't pay.

## Options

1. **Enforce the documented rule:** call `config.is_registration_open(now_ms)`
   in `signup.rs`. This closes registration at the start, which ends day-of
   self-registration. Walk-ins (`/api/walkin/register`, staff-authed) are a
   separate handler and would keep working.
2. **Close at the end instead:** registration stays open through the event
   and closes at `event_end_ms`. Change `is_registration_open` and the two
   docs to match, so that code and docs agree.
3. **Keep today's behaviour:** delete `is_registration_open` or rename it to
   what it is (unused), and fix the two docs.

Whichever is chosen, check the sibling writer, `register/post_event.rs`
([[duplicated-state-transition-paths]]). It only accepts `Completed` events
with `post_event_registration_open`, so an `Active` event past its end is
still `signup.rs`'s job until someone completes it. Add a test with a
started event and one with an ended event.

## Not affected

The on-chain rules held in the same run: `refund` before the end was
rejected with `Custom(1)` = `RefundNotYetAllowed`, and after the end it
finalized (`.issues/164`).
