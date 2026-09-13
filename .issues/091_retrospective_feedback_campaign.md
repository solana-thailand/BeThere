# 091 — Asking 30 people for feedback on events that already happened

**Status:** open — the SQL is written and rehearsed, the prod write is not done
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

### Running it

```bash
cd worker
mv ~/.pnp.cjs ~/.pnp.cjs.bak
npx wrangler d1 execute DB --remote --file ../.issues/091_backfill.sql
mv ~/.pnp.cjs.bak ~/.pnp.cjs
```

Blocked from the agent session — the permission classifier refuses a prod D1
write. Everything up to it (rehearsal, A/B, the migration, the deploy) was done.

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

Already neutral and already shipped — no change needed:

- title `How was the event?` (`notifications/content.rs:54`)
- CTA `Answer four quick questions` → `/events/{slug}/post-event-register`

The comment at `content.rs:59-61` records why it points at the form rather than
the ticket: there is nothing left to deposit for an event that is over.

## Open: one form for all of a person's events

Requested 2026-09-13. Today the unit is the event, so a person who attended
three gets three inbox rows and three forms — 45 submissions from 30 people.

It is cheaper than it looks. `POST /api/public/event/{slug}/register-post-event`
already takes an arbitrary `profile_fields` map scoped by slug, so a combined
page can submit the same four `post.*` answers once per event from a single
button. **No backend change and no migration** — a new route that lists the
events where the caller has a check-in and the form is accepting, renders the
four questions per event, and POSTs N times on submit.

The tension to resolve before building it: one set of answers for all events
gets a higher completion rate and loses the per-event signal, which is the
signal DevRel wants. Per-event answers on one page keeps both, at the cost of a
longer form. Recommend the second — same page, repeated question block, one
submit.

Not started. Do the backfill first; it is what makes any of this reach anyone.

## Related

- `.issues/080` — the survey kind and its delivery window.
- `.issues/087` — the four questions the form asks.
- `.issues/090` — the release that made all of it live.
