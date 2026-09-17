# 095 — The notification inbox was invisible to half the people who attended

**Status:** fixed 2026-09-13 (migration 0036 + corrective backfill); **not deployed**
**Found:** 2026-09-13, by the organizer checking his own account
**Severity:** high — it silently halved every notification the platform has ever
been able to show, and it was invisible from the outside

## How it surfaced

*"ผมเป็นคนจัดและลงทะเบียนในทุก event ที่ผ่านมา แต่ตัวเองเห็นแค่ 4 ที่ขึ้นมาใน survey"*

The right instinct, and the numbers back it. `ratchapon.poc@gmail.com` has 11
registrations and **5** check-ins on events whose post-event form is open — but
the feedback page offered 4.

```
slug                                         status      per  approval_status
intro-to-vibing-on-solana                    completed   1    approved
solana-x-ai-builders-the-road-to-mainnet-2   completed   1    checked_in   ← missing
solana-x-ai-builders-the-road-to-mainnet-3   completed   1    approved
solana-x-ai-builders-the-road-to-mainnet-4   completed   1    approved
solana-x-ai-builders-the-road-to-mainnet-5   completed   1    approved
```

One row out of five reads `checked_in`. That was the whole difference.

## What it is

`attendees.approval_status` is a **progression**, not a set of alternatives
(`CheckInStatus`, `domain/src/models/attendee/status.rs`):

```
PendingApproval → Approved → Invited → CheckedIn
```

Someone scanned at the door ends at `checked_in`, which is *past* `approved`,
not instead of it. Three places filtered `approval_status = 'approved'`:

| where | since | effect |
|---|---|---|
| `notification_inbox_visible` (view) | migration **0030** | **every** notification kind hidden from anyone at `checked_in` or `invited` |
| `notification_post_event_survey` (trigger) | migration 0035 | survey never queued for them |
| `.issues/091_backfill.sql` | today | enrolled only half the people |

The view is the serious one. It has been wrong since notifications were built,
and nobody could see it because `notification_outbox` was empty until today.

## The size of it, from production

```sql
SELECT a.approval_status, COUNT(*) FROM attendees a JOIN events e ON e.id=a.event_id
WHERE a.checked_in_at IS NOT NULL AND a.checked_in_at<>'' AND e.post_event_registration_open=1
GROUP BY a.approval_status;
-- checked_in  45
-- approved    45
```

**Exactly half.** The campaign reported as "45 rows / 30 people" is really
**90 rows / 55 people**. And for a post-event survey the excluded half is the
worst possible half — `checked_in` is the strongest evidence a person was
actually in the room.

## The fix

Migration **0036** rebuilds the view and the trigger with
`approval_status <> 'pending_approval'`.

**Why an inequality and not an `IN` list.** An `IN ('approved','invited',
'checked_in')` fails *closed* when a new status appears — it silently drops
people, which is precisely this bug. The inequality fails *open*: at worst a
pending registration sees a message, which is visible and recoverable. For a
notification system, visible-and-wrong beats silent-and-missing.

Everything else in the view is byte-identical to 0035. The first draft of this
migration also tightened the survey clause and dropped its `checked_in_at`
condition; that was an unintended behaviour change and was reverted before
commit. A migration that fixes one predicate should touch one predicate.

## Verified

Rehearsed against the prod export with 0035 + 0036 + the corrective backfill:

| | before | after |
|---|---|---|
| inbox rows | 45 | **90** |
| distinct people | 30 | **55** |
| the reporter's own blocks | 4 | **5**, RTM #2 restored |
| outbox | survey/pending 45 | survey/pending 90, no `registration` rows |

New test `test_inbox_is_visible_to_every_status_past_pending` walks `approved`,
`invited` and `checked_in` and asserts each sees their inbox, and that
`pending_approval` is the one state that stays out. A/B: reverting the predicate
to `= 'approved'` fails it twice (`invited must see their inbox`,
`checked_in must see their inbox`); restored to green. 26 tests in that suite,
and it executes every production migration, so 0036 is exercised on every CI run.

## To apply

Deploy, then:

```bash
cd worker
mv ~/.pnp.cjs ~/.pnp.cjs.bak
npx wrangler d1 migrations apply DB --env staging --remote
npx wrangler d1 migrations apply DB --remote
npx wrangler d1 execute DB --remote --file ../.issues/095_backfill_checked_in.sql
mv ~/.pnp.cjs.bak ~/.pnp.cjs
```

`.issues/095_backfill_checked_in.sql` is safe to run after the first pass: every
statement is `INSERT OR IGNORE` against a unique key, so already-enrolled rows
are untouched and no survey is queued twice.

Expect afterwards: `survey | pending | 90`, no `registration` rows, and
`notification_inbox_visible` at 90 rows / 55 people.

## The pattern, again

Three copies of one predicate, two of them wrong. This is the fourth time today:
`.issues/080` (the cancel rule in two places), `.issues/086`, `.issues/095`
(this), and `.issues/095`'s sibling `.issues/094`/`#102`, where the same payload
shape existed in four builders and I fixed the wrong one first.

The lesson that keeps not sticking: **when a rule appears once, look for the
second copy before fixing the first.**

## Related

- `.issues/091` — the campaign whose reach this doubles.
- `.issues/094` — the recognition work on the same page.
- Memory `duplicated-state-transition-paths`.
