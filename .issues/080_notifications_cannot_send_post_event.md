# 080 — The notification pipeline cancels every post-event send by design

**Status:** open
**Found:** 2026-09-13, answering the DevRel `BETHERE-ASKS.md` item 4
**Severity:** medium (a whole class of message is unreachable; no data at risk)

## What

`notifications::prepare` re-reads eligibility immediately before transport and
cancels the job if the event is over:

```rust
// worker/src/notifications/prepare.rs:26-31
if event.status != EventStatus::Active
    || (event.event_end_ms > 0 && event.event_end_ms <= now)
{
    return Ok(Prepared::Cancel);
}
```

Every notification kind goes through this. So **no message can ever be
delivered after an event ends**, regardless of kind, enrolment or scheduling.

## Why it matters now

DevRel want to send a post-event satisfaction survey and have evidence the
manual alternative does not work: two Google Forms, lists cut for 55 onsite and
96 online people, ready for weeks, **zero responses**. Their conclusion —
proximity beats intent — is the same insight that made the deposit hold work.

`.issues/068` and the post-event registration endpoint already give us the
retrospective plumbing. The outbox is the natural carrier for "the event you
attended has a two-question survey". This gate is what stops it, and it is
invisible from outside the Worker: the job would be enqueued, marked cancelled,
and nobody would be told why.

## Why the gate exists

It is correct for the kinds that exist today. `reminder`, `deposit_confirmed`
and `deposit_rejected` are all pre-event or during-event; delivering a "your
event is tomorrow" mail after the event would be worse than not sending. The
mistake is that the rule is applied to the *pipeline* rather than to the
*kind*.

## Proposed fix

Move the liveness rule from `prepare` into the per-kind policy:

1. Give each kind a delivery window — `PreEvent` (current behaviour) or
   `PostEvent`.
2. `prepare` keeps cancelling `PreEvent` kinds on a finished event, and stops
   cancelling `PostEvent` ones.
3. A `PostEvent` kind should additionally require what a pre-event kind does
   not: the recipient actually attended (`attendees.checked_in_at` non-null),
   so a survey does not go to no-shows.
4. Keep the enrolment join as-is — `notification_enrollments` is already keyed
   per (attendee, event), so a single event can be targeted without touching
   the others.

Do not fix this by special-casing a `survey` string in `prepare`; the defect is
that a per-kind property was encoded as a pipeline invariant, and the next kind
would hit it again.

## This is not the only thing blocking a send

Worth stating together so nobody fixes this one and expects mail to move.
Production has the pipeline switched off at three independent points, all in
`worker/wrangler.toml`:

1. `NOTIFICATIONS_ENABLED = "0"` — `dispatch()` returns immediately
   (`notifications/mod.rs:48`). Set in prod **and** staging.
2. `NOTIFICATION_FROM = ""` — `Sender::from_env` rejects it as not a plausible
   email, so no job is taken even when enabled.
3. The `[[send_email]]` binding named `EMAIL` is commented out, with the reason
   in the file: *"Enable after onboarding a sender domain to Cloudflare Email
   Sending."*

(3) is not a code change — it needs a verified sender domain on the Cloudflare
account. `notification_enrollments` and `notification_outbox` both have **0
rows** in prod; nothing has ever been enqueued, so there is no backlog waiting
to fire when this is switched on.

Sequencing: the sender domain unblocks pre-event sends (DevRel's 27 September
logistics message would pass `prepare` — that event is Active and in the
future). This issue is what additionally unblocks the survey.

## Verification

A test that enqueues a `PostEvent`-window kind for an event whose
`event_end_ms` is in the past and asserts `Prepared::Ready`, alongside the
existing behaviour for a `reminder` on the same event asserting
`Prepared::Cancel`. Both directions — a fix that makes everything deliverable
has removed the guard rather than scoped it.

## Related

- `.issues/068_retrospective_learning_hub.md` — the retrospective flow this
  would carry.
- DevRel `reports/phase-2/BETHERE-REPLY.md` items 2 and 4.
- `.issues/079` — filed from the same review.
