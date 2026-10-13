# 165: Public registration stays open after the event starts (and after it ends)

**Status:** fixed on develop (2026-10-13, session `event-checkin-11`; owner decision below). In-person seats already closed at the end server-side (`in_person_registration_closed`); the page now agrees (pull 262, deployed) and, after the end, registering is how people watch the recording (this PR). Originally opened 2026-09-29 by session `event-checkin-83` during the `.issues/164` gap-2 rehearsal on staging.

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

## Evidence 2026-10-13 (session `event-checkin-11`): the page says "Happening now!" nine days after the event

Opening prod `/e/solana-x-ai-builders-the-road-to-mainnet-6-bangkok` on
2026-10-13 (RTM #6 ran on 4 Oct) shows **"Happening now!"**, the **Reserve
Your Spot** button and the sign-in-to-register block.

The cause is in the frontend: `public_event/page.rs` sets `event_completed`
only from `status == "completed"`. When the countdown is empty and the status
is still active, `details_card.rs` renders "Happening now!", and
`show_reg_form = !event_completed` keeps the form. Nothing compares the time
with `event_end_ms`, so every past event whose status was never set to
completed reads as live and open. This predates the plan 046 release and is
not caused by it.

The fix belongs to this issue's decision. If registration closes at the end
(the recommendation above), the page should treat `now > event_end_ms` as
over (completed gateway, no form, no "Happening now") and the API should
refuse registration past the end. If registration closes at the start, the
form goes at `event_start_ms`, and "Happening now!" stays only between start
and end.

## Owner decision 2026-10-13: after the event, register to watch the recording

"I just want registration to be there so people can watch afterwards." So
seats close when the event ends, and from then on registering is how people
get the recording:

- One rule, `domain::models::event::{ended, is_over}`, decides "over" for the
  page, for the in-person seat cutoff (`in_person_registration_closed` now
  calls `ended`), and for who may register to watch. A time-TBA event ends a
  day after its end time, which was already the seat rule.
- Post-event registration opens on its own once an event is over **and** has
  a public `https://` recording (`EventConfig::recording_url`). The
  organizer's toggle and deadline still work.
- The public event API sends `has_recording`, never the link. The ended page's
  button reads "Register to watch the recording" (`?watch=1`), the form says
  so, and the register response returns `video_url` and `course_id` to the
  registrant, so the confirmation shows "Watch the recording" and "Continue
  the course".
- Kept open on purpose: course pages (`/events/{course}`) still show their
  recordings to everyone ("Knowledge should be free for everyone"); the event
  page is a soft gate for lead capture, not a lock.
