# 101 — One person would have received twelve identical emails

**Status:** live in prod 2026-09-14 — migration 0038 applied to staging and prod, verified
**Requested by:** DevRel, `reports/phase-2/BETHERE-ASKS-2.md`
**Severity:** high — it is the last thing between the campaign and sending

## The ask, and the diagnosis is theirs and correct

`notification_outbox.dedup_key` is `[event, attendee, kind]`. Right for every
kind whose destination is a single event — `reminder`, `deposit_rejected`,
`registration`. Wrong for `survey`, whose destination is `/feedback`, a page
deliberately **not** scoped to an event.

`dispatch()` sends one message per row, so production held:

| | |
|---|---|
| pending survey rows | **425** |
| distinct people | **206** |
| most for one person | **12** |
| people with duplicates | 83 |
| surplus rows | 219 |

The form was collapsed in `.issues/091`; the notification was not. At 45 rows
that was a nuisance. At 425 it is the campaign — and the heaviest send lands on
the regulars, who are the people the programme can least afford to annoy.

One correction to their write-up, because it matters to anyone scripting
against this: the key's first element is **`events.id`, not the slug.** They
diverge on every event created by duplication (`.issues/079`) — RTM #6's id is
still `solana-x-ai-builders-the-road-to-mainnet-5-bangkok-copy`. Irrelevant once
the survey stops using it, but it would have sent a script to the wrong column.

## The fix

Migration **0038** keys the survey per person:

```
survey          ->  ["*", lower(attendees.email), "survey"]
everything else ->  unchanged
```

`dedup_key` is already `UNIQUE`, so `INSERT OR IGNORE` does the collapsing —
exactly as DevRel predicted. Lowercased to match `notification_inbox_visible`,
which compares `lower(email)`.

### The part the ask did not mention, which would have broken it

The repair has to **rewrite the surviving row's key**, not just cancel the
extras. Leave the survivor on its old `[event, attendee, survey]` key and the
next enqueue writes a *second*, differently-keyed row for the same person — the
UNIQUE constraint never sees a collision, and the bug returns silently on the
next event that opens. Both statements ship in the migration for that reason.

Surplus rows are `cancelled`, not deleted: `notification_inbox_visible` already
excludes cancelled, and a record of what was collapsed is worth the rows.

## The consequence DevRel asked us to decide with it

Their note says naming one event becomes wrong. It is worse than that — the
shared template renders **four** event-specific things: the subject
(`"{title} — {event.name}"`), `When:`, `Where:`, and `Your ticket:`. For a
collapsed row the filed event is whichever enrolled the person first, so all
four would describe one of up to twelve sessions, two of them describing a venue
and a time that are months past, and the ticket linking to a finished event.

So the survey now returns its **own message** rather than the shared one with a
different title: no event name, no when/where, no ticket, one link to
`/feedback`, and a line saying the page lists every session they attended. Title
is `How were the sessions?` in both the mail and the inbox.

DevRel has Thai copy drafted on this assumption
(`reports/phase-2/SURVEY-EMAIL.md`); the English above is a placeholder that
says the right things, not a decision about final wording.

## Verified against the real thing

Rehearsed on a **fresh prod export taken after the online backfill** (1.8 MB,
2026-09-14), not on a reconstruction:

| | before | after |
|---|---|---|
| pending survey | 425 | **206** |
| cancelled | 0 | **219** |
| DevRel's `HAVING COUNT(*) > 1` query | 83 rows | **0 rows** |
| distinct emails among pending | 206 | **206** |
| inbox view | 425 / 206 | **206 / 206** |

Survivor keys are `["*","ratchapon.poc@gmail.com","survey"]`. Re-running 0038
leaves the counts unchanged, so it is idempotent. Full migration chain applies
clean from empty.

New test `test_one_survey_per_person_not_one_per_event` builds one human with
two attendee rows and opens both events; A/B reverting the key to the per-event
form fails it `2 != 1`. Worker lib suite 288, SQL suite 28.

### A trap the test walked into first

Writing the fixture as "register, then `UPDATE attendees SET email=…`" silently
produced an empty fixture. `notification_attendee_email` erases the outbox and
the enrolments whenever an address changes — correct PDPA behaviour, and a
reminder that an email correction on a real attendee drops their queued
messages. The fixture now sets the address at INSERT.

## Not changed

Everything DevRel listed as out of scope: `/feedback`, the `post.` prefix, the
eligibility rule, `post_event_registration_open`, `recap_published`, the quiz
flags.

## Applied

Migration 0038 on `bethere-db-staging` and `bethere-db`, 5 commands each. No
separate backfill — the repair travels in the migration. Read back from
production immediately after:

```
cancelled  219 rows   83 distinct emails
pending    206 rows  206 distinct emails
```

DevRel's `HAVING COUNT(*) > 1` verification query returns `"results": []`.

Every number matches the rehearsal against the prod export exactly, including
the 83 — the count of people who had duplicates, which is now the count of
people whose surplus was collapsed.

## Related

- `.issues/097` — the eligibility widening that took this from 90 to 425.
- `.issues/091` — the page that was already per-person.
