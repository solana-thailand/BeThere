# 102 — Collapsing the message collapsed the form with it

**Status:** fixed 2026-09-14, **not deployed**
**Found:** 2026-09-14, by the organizer noticing his own page
**Severity:** high — a regression I shipped four hours earlier, on the page the
whole campaign runs through

## What

> *"แล้วผมมี event ที่ลงไว้ตั้งเยอะ เหมือนก่อนหน้าสามารถทำได้ทุกอัน ตอนนี้เหลืออันเดียวขึ้นมาให้เลือกแล้วหรอ"*

Correct, and it was my doing. Migration 0038 (`.issues/101`) collapsed the
survey outbox to one row per person. `/feedback` built its question blocks from
that outbox — `.issues/091` deliberately reused `notification_inbox_visible` so
that eligibility had exactly one definition — so collapsing the message
collapsed the form. Confirmed against production before touching anything:

```sql
SELECT COUNT(*) FROM notification_inbox_visible
WHERE lower(email)='ratchapon.poc@gmail.com' AND kind='survey';
-- 1
```

One block, for a person eligible on eleven sessions.

## Why the reuse was wrong, and it was not wrong when it was written

The message and the form want different grains, and neither is a mistake:

```
message  ->  one per PERSON          "how were the sessions you attended"
form     ->  one per (PERSON, EVENT)  the per-event answers are the whole signal
```

While the queue was per (person, event) the two coincided and reusing one for
the other was free. 0038 separated them, and the reuse became a bug in the same
commit that made the message correct.

DevRel's ask said this outright and I read past it: *"The mail needs to speak
about 'the sessions you attended' and **let /feedback enumerate them**."*
Enumerating needs a source that is not the collapsed queue.

## The fix

`feedback_eligible_events` (migration 0039) — the same eligibility rule, keyed
to **enrolment** rather than to a queued message:

- enrolled, not `pending_approval`, event not cancelled
- `post_event_registration_open`, deadline not passed
- onsite by check-in, online by registration (`.issues/097`)

`GET /api/my-feedback-events` returns it for the signed-in identity, with the
poster, date, venue and participation type included — so the page also stops
firing one public GET per event to fetch the poster it now gets for free.

Verified on the prod export taken after 0038:

| | |
|---|---|
| `feedback_eligible_events` | **425 rows / 206 people** |
| pending survey messages | 206 |
| the reporter's own blocks | **11**, up from 1 |

Eleven, not the five he saw yesterday, because he registered online for the
Latent Space sessions too — which the `.issues/097` widening made eligible.

## The cost, stated rather than hidden

There are now two places expressing the survey's eligibility: this view, and
the survey clause inside `notification_inbox_visible`. They read the same
columns and must agree. That is the price of a queue whose grain differs from
the form's, and it is the exact shape of defect this repo keeps producing
(`.issues/086`, `.issues/095`, `.issues/101`). Test
`test_form_stays_per_event_while_the_message_is_per_person` asserts both at once
— one message, two blocks, from one fixture — so a change to either that does
not change the other fails.

## Related

- `.issues/101` — the collapse this regressed out of.
- `.issues/091` — where the reuse was introduced, correctly at the time.
