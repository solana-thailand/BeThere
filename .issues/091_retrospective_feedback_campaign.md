# 091 — Asking 30 people for feedback on events that already happened

**Status:** live in prod 2026-09-13 — backfill applied (45 survey rows / 30 people) and `/feedback` deployed as `8d7ed82d`
**Found:** 2026-09-13, deciding how to reach attendees with no email transport
**Severity:** medium (the feature shipped in `.issues/080`/`087` currently reaches nobody)

## The question this answers

*Can we send a link for an event that is already over and have people fill the
form retrospectively?* **Yes, and it is live as of `253717a5`.** Nothing further
is needed to make the link work:

- 12 events are `completed` with `post_event_registration_open = 1`.
- `post_event_registration_until_ms` is NULL on all of them, which
  `post_event_registration_deadline_passed` documents as *"open indefinitely, so
  it never passes"* (`domain/src/models/event/config.rs:369`). Not an oversight —
  the designed value for exactly this case.
- Proven against prod: `GET /api/public/event/solana-in-latent-space-part-5`
  returns `"post_event_registration_accepting": true`.

So `/events/{slug}/post-event-register` is a working link today for any of the
twelve. What is missing is that **nothing tells anyone the link exists.**

## Reach: 30 people, 45 check-ins

Only people who actually checked in are asked how the event went. That is 45
attendance records belonging to 30 distinct people:

| event | checked in |
|---|---|
| `solana-x-ai-builders-the-road-to-mainnet-4-bangkok` | 16 |
| `solana-x-ai-builders-the-road-to-mainnet-5-bangkok` | 15 |
| `solana-x-ai-builders-the-road-to-mainnet-3-bangkok` | 13 |
| `intro-to-vibing-on-solana` | 1 |

The other eight open events have no check-ins at all. 45 messages is inside
every free email tier that exists, so **cost is not what is blocking this.**

## The free delivery path is already built

`NOTIFICATIONS_ENABLED` gates `dispatch()` only — the cron that sends **email**
(`notifications/mod.rs:51`). The in-app inbox does not go through it: it reads
the `notification_inbox_visible` view, which requires `status <> 'cancelled'`
and **not** `status = 'sent'`. A row sitting at `pending` because email is off
is still visible in the app.

Everything needed is deployed: `GET /api/my-notifications` plus read/read-all
(`handlers/mod.rs:185-190`), and the attendee UI at
`frontend-leptos/src/pages/landing/notifications.rs`.

Cost: nothing. No domain, no plan upgrade, no provider.

## Two gaps stop it reaching anyone

### 1. `notification_enrollments` has 0 rows

The only writer is `signup.rs:406`, on a Google-verified self-registration. The
477 attendees in prod arrived through sheet sync, which does not enrol. The
inbox view inner-joins the enrolment table, so with 0 rows the inbox is empty
for everyone.

### 2. The survey trigger already missed its only chance

`notification_post_event_survey` is `AFTER UPDATE OF post_event_registration_open
... WHEN NEW=1 AND OLD<>1`. The twelve events were flipped to 1 at **08:05 UTC**;
migration 0035, which creates the trigger, was applied at **10:40 UTC**. The
trigger was created after the edge it watches for, and the flag is already 1, so
there will never be another 0→1 transition. It cannot fire for these events.

## The backfill, rehearsed and A/B verified

`.issues/091_backfill.sql`. Three statements, in order.

The middle one is the part that is easy to miss: `notification_registration` is
an unconditional `AFTER INSERT ON notification_enrollments`, so backfilling
enrolment **queues a "Registration saved" message for events that ended months
ago**. (Its sibling `reminder` insert is guarded by `event_start_ms > now`, so
only `registration` leaks.) Those rows would sit `pending` — the sweep in
`cancel.sql` only runs inside `dispatch()`, which is off — and `pending` is
visible in the inbox.

Rehearsed against a copy of the prod export with 0035 applied:

| | with the DELETE | without it |
|---|---|---|
| inbox rows | **45 survey** | 45 survey + **45 registration** |
| distinct people | 30 | 30 |

Both directions were run, so the DELETE is proven load-bearing rather than
assumed to be.

### Applied

Run by the owner on 2026-09-13; the agent session could not, because the
permission classifier refuses a prod D1 write. Verified straight afterwards:

```
notification_outbox  →  survey | pending | 45     (no 'registration' rows)
notification_inbox_visible  →  45 rows | 30 people
```

Both numbers match the rehearsal exactly.

### Reversing it

Additive only; the exact inverse is two statements:

```sql
DELETE FROM notification_outbox WHERE kind = 'survey';
DELETE FROM notification_enrollments;
```

Safe because both tables were empty before this: no enrolment or outbox row in
prod predates the backfill. Backup for the day: `~/bethere-backups/bethere-db-20260913.sql`.

### Verifying it

```sql
SELECT kind, status, COUNT(*) FROM notification_outbox GROUP BY kind, status;
-- expect exactly: survey | pending | 45   (and no 'registration' rows)
SELECT COUNT(*), COUNT(DISTINCT email) FROM notification_inbox_visible;
-- expect: 45 | 30
```

## The message copy

Neutral, and now pointed at the combined page:

- title `How was the event?` (`notifications/content.rs:54`)
- CTA `Share your feedback` → `/feedback`

The comment at `content.rs` records why it avoids the ticket: there is nothing
left to deposit for an event that is over. The single-event route is unchanged
and still serves the QR codes printed on the recap posters — a notification just
must not send anyone down it, and a test asserts that.

## Built: one page for all of a person's events

`/feedback` — `frontend-leptos/src/pages/public/feedback.rs`.

Of the two shapes, the second was chosen: **per-event answers on one page**. One
set of answers for everything would read better and lose the per-event signal,
which is the only signal DevRel reports on.

The question set is transcribed from DevRel's own Google Form rather than
invented, so the numbers stay comparable with the Phase 1 figures already
submitted to the Foundation. Read out of the live form's `FB_PUBLIC_LOAD_DATA_`:

| key | question | type |
|---|---|---|
| `post.satisfaction.content` | ด้านเนื้อหา | ไม่พึงพอใจ / พึงพอใจ / พึงพอใจมาก |
| `post.satisfaction.venue` | ด้านสถานที่ | same scale |
| `post.satisfaction.catering` | ด้านอาหารเครื่องดื่ม | same scale |
| `post.satisfaction.promotion` | ด้านการประชาสัมพันธ์ | same scale |
| `post.comment` | ข้อเสนอแนะ | free text |
| `post.next_topics` | เนื้อหาที่ท่านสนใจ…ครั้งต่อไป | free text, asked once |
| `post.latent_space_continue` | ซีรีส์ Latent Space — อยากให้จัดต่อไหม? | 4 options, asked once |

Three things worth knowing about the implementation:

- **Eligibility is not re-derived.** The event list comes from
  `GET /api/my-notifications`; `notification_inbox_visible` already means
  "enrolled, approved, checked in, form still accepting". A second definition in
  the client is a second thing to keep in step.
- **The two closing questions ride with the first block only.** They are about
  the programme, not an event; sending them per block would multiply one opinion
  by however many events the person attended.
- **A partial failure keeps what succeeded.** Someone who answers for three
  events does not lose two because the third closed mid-submission.

`InboxNotification` gained `event_slug` for this — the view always had `slug`,
it was just folded into `action_url`, and parsing a slug back out of a URL is
coupling that breaks the day the URL changes.

### The contact-wipe this turned up

`upsert_post_event_attendee` did `contact_channel = excluded.contact_channel`,
and the handler collapses an absent field to `""`. A feedback-only submission
sends no contact details — so every one of the 30 people would have had their
Telegram handle erased by answering. Now
`COALESCE(NULLIF(excluded.contact_channel, ''), attendees.contact_channel)`.

Latent since the endpoint shipped and never triggered, because it had no callers
(0 `retrospective` rows in prod). This page would have been the first.

### Deployed

Prod version `8d7ed82d-ad91-4b3e-8dfe-c13b4282fd15`, commit `4414de1`, bundle
`event-checkin-frontend-7a60e85e0fcd0d36`. `GET /feedback` returns 200 with
`text/html`, and `cmp` confirms the served wasm is byte-identical to the local
build — which was checked before deploying to contain both
`post.satisfaction.content` and `ด้านอาหารเครื่องดื่ม`. Staging served the same
bytes first. Prod health after the release: `attendees 477`, `events 16`,
`d1.connected true`.

One link in the chain is **not** verified from here: the inbox payload now says
`action_url: /feedback`, but `GET /api/my-notifications` needs a Google session,
so that is covered by the unit test
(`survey_points_at_the_combined_feedback_page_not_a_deposit_or_calendar`) rather
than by a probe against production. The first person to open their inbox is the
first real check.

### Still not done

- No styling of its own — it reuses `card` / `dev-profile-field` /
  `dev-profile-input` rather than adding a 20th stylesheet.
- `post_event_register.rs` still asks its own three questions
  (`post.satisfaction.overall`, `post.nps`, `post.would_return`) for the QR
  path. Two question sets for one thing is the duplication this repo keeps
  getting bitten by; unify once the campaign has run.

### Points for answering — recommended against, for now

Asked 2026-09-13. **No for this round**, for one reason that outweighs the rest:
a reward attached to a *satisfaction* score biases the score, and these numbers
go to the Solana Foundation. "We paid people to rate us" is not a footnote worth
earning for 30 responses.

The other two: 30 people is small enough that a direct ask works better than an
incentive, and it is a second build on top of an undeployed one two days before
the call.

Worth revisiting for RTM #6, where the survey is same-day and completion is a
volume problem rather than a reach problem. If it happens, reward *completion*
and not sentiment, and keep it non-monetary — the deposit credit ledger is
right there and is exactly the thing that must not be involved.

## Related

- `.issues/080` — the survey kind and its delivery window.
- `.issues/087` — the four questions the form asks.
- `.issues/090` — the release that made all of it live.

## Follow-up: the unverified link is now tested, not probed

The deploy note above recorded one gap — the inbox payload's
`action_url: /feedback` could not be checked against production, because
`GET /api/my-notifications` needs a Google session.

The right answer to "I cannot probe this" was not to probe harder. The decision
lived inside `list_for_attendee`, which takes a live `D1Database` and therefore
could not be unit-tested at all; the survey's destination was covered only on
the **email** path (`content.rs`). Two code paths that must agree, one of them
untested, is the shape of `.issues/086`.

`inbox_action(kind, deposit_needed, attendee_id, event_id, label)` is now a pure
function, and three tests cover it:

- `survey_inbox_button_points_at_the_combined_feedback_page` — `/feedback`, even
  with `deposit_needed = true`, so an unpaid deposit cannot hijack a post-event
  message.
- `every_other_kind_keeps_its_own_destination` — ticket vs deposit, and that a
  `DepositConfirmed` row goes to the ticket while `deposit_needed` is still
  true, which makes the match-arm order load-bearing and asserted.
- `action_urls_encode_their_ids` — `att/1` reaches the path as `att%2F1`.

A/B: reverting the arm to `/events/x/post-event-register` fails the first test
(`left: "/events/x/post-event-register", right: "/feedback"`); restored to green.
Worker suite 287 passing, up from 284.

`Action::PostEventForm` was renamed `Action::Feedback` — the variant named the
destination it no longer has.
