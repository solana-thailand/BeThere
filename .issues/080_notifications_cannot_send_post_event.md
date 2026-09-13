# 080 — The notification pipeline cancelled every post-event send by design

**Status:** fixed 2026-09-13, not yet deployed
**Found:** 2026-09-13, answering the DevRel `BETHERE-ASKS.md` item 4
**Severity:** medium (a whole class of message was unreachable; no data at risk)

## What it was

`notifications::prepare` re-read eligibility immediately before transport and
cancelled the job if the event was over:

```rust
// worker/src/notifications/prepare.rs:26-31 (before)
if event.status != EventStatus::Active
    || (event.event_end_ms > 0 && event.event_end_ms <= now)
{
    return Ok(Prepared::Cancel);
}
```

Every kind went through it, so **no message could ever be delivered after an
event ended**, regardless of kind, enrolment or scheduling.

The rule was correct for the kinds that existed — `reminder`,
`deposit_confirmed`, `deposit_rejected` are all pre-event. The mistake was that
a per-kind property had been encoded as a pipeline invariant.

### It was in two places, not one

The original write-up named only `prepare.rs`. `sql/cancel.sql` carried the same
rule independently (`e.event_end_ms > unixepoch() * 1000`) as a bulk sweep that
runs *before* any job is claimed, so fixing the dispatcher alone would have left
post-event jobs cancelled in SQL before `prepare` ever saw them — the failure
mode in `.issues/086` and in the `duplicated-state-transition-paths` note.

## The fix

The window is now a property of the kind, and the kind is a type.

- `policy::NotificationKind` — the five kinds as an enum, parsed once at the
  edge. `policy::DeliveryWindow` — `BeforeEventEnds` or `AfterEventEnds`.
- `policy::in_delivery_window` — one pure decision, unit-tested in both
  directions over an event's lifetime. `Completed` counts as ended whatever
  `event_end_ms` says; `event_end_ms == 0` means no end was ever recorded, so
  such an event never becomes post-event.
- `prepare` asks for the kind's window instead of applying one rule to all.
  Pre-event behaviour is byte-for-byte what it was.
- `cancel.sql` partitions the same way. The kind list there is a copy of
  `delivery_window`, so `cancel_sql_covers_every_pre_event_kind` reads the
  literals back out of the `.sql` file and compares them to the enum — it fails
  if either side moves.

`survey` is the first `AfterEventEnds` kind, which is what makes the model
testable rather than speculative:

- **Enqueued** by a trigger on `post_event_registration_open` flipping to 1 —
  the organizer action that makes the questions answerable at all — and only
  for enrolled attendees with a non-null `checked_in_at`. A no-show is not
  asked how the event went.
- **Rendered** pointing at `/events/{slug}/post-event-register`, the form that
  carries DevRel's four questions (`.issues/087`). Not the ticket, not a
  deposit that can no longer be paid, and no "add to calendar" for a date that
  has passed.
- **Re-checked at send time** against `post_event_registration_accepting` — the
  same helper the public recap CTA uses — so a form the organizer closes, or
  whose deadline lapses, does not leave a message inviting people into a 410.
- **Withdrawn from the inbox** by the same condition in
  `notification_inbox_visible`.

Migration `0035_notification_delivery_window.sql` rebuilds `notification_outbox`
(SQLite cannot alter a CHECK) and recreates the view and all seven triggers that
name the table. They are dropped first: with `legacy_alter_table` off SQLite
reparses the whole schema during `ALTER TABLE ... RENAME` and fails on a trigger
that references a momentarily-absent table.

## Other stringly-typed kind matches this turned up

Both had a catch-all arm, so a `survey` row would have been shown to the person
under another kind's wording:

- `notifications::outbox::presentation` — `_ =>` fell through to "Registration
  saved". Now exhaustive over the enum, with a test asserting no two kinds share
  a title.
- `frontend-leptos` organizer notifications table — `_=>"Notification"`.

`content::render` also lost its vestigial `Result`: it could only fail on an
unknown kind string, which is now unrepresentable.

## Verified

- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
  full workspace + worker suites, frontend wasm32 clippy + tests, worker wasm32
  release build — all clean.
- **A/B, not just green.** Restoring the old `cancel.sql` makes
  `test_finishing_an_event_retires_pre_event_jobs_but_not_the_survey` fail
  (`'cancelled' != 'pending'`); adding `'survey'` to the SQL kind list makes
  `cancel_sql_covers_every_pre_event_kind` fail. Both restored to green after.
- Four new SQL-behaviour tests in `worker/tests/notifications/test_outbox.py`
  (25 total), which execute every production migration against SQLite — so 0035
  is exercised by CI on every run.

## Still blocking an actual send — none of it code

Unchanged by this fix, and all in `worker/wrangler.toml`:

1. `NOTIFICATIONS_ENABLED = "0"` — `dispatch()` returns immediately. Set in prod
   **and** staging.
2. `NOTIFICATION_FROM = ""` — `Sender::from_env` rejects it, so no job is taken
   even when enabled.
3. The `[[send_email]]` binding `EMAIL` is commented out: *"Enable after
   onboarding a sender domain to Cloudflare Email Sending."*

(3) needs a verified sender domain on the Cloudflare account — **owner action**.
`notification_enrollments` and `notification_outbox` both have 0 rows in prod, so
there is no backlog waiting to fire when it is switched on.

## Known gaps

- The enqueue trigger is `AFTER UPDATE OF post_event_registration_open` only.
  There is no INSERT counterpart because the flag can only be set on a Completed
  event and events are not created Completed — but an import that wrote a
  Completed event with the flag already on would not enqueue.
- Question wording on the form is still ours, not DevRel's (`.issues/087`).
- Nothing surfaces the answers to an organizer yet.

## Related

- `.issues/087` — the four questions the survey links to.
- `.issues/083` — the admin control that opens the form, and now the send.
- `.issues/068` — the retrospective flow this carries.
- DevRel `reports/phase-2/BETHERE-REPLY.md` items 2 and 4.
