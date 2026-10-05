# 181: F7 sponsors follow-ups: prod migration order, logo upload, new baselines

**Status:** open (2026-10-01). Filed by session `event-checkin-b5` when it committed F7 (`f5f5129f`, `4bc9fece`, `ec458a89`), from the handoff notes of session `event-checkin-f7`.

F7 (sponsors plus the organizer line) is committed on `develop` and is not
pushed or deployed. Three things remain.

## 1. Apply migration 0057 to prod before the code (blocking the F7 deploy)

`upsert_event` now always writes `events.sponsors_json`. If code from
`f5f5129f` onward reaches prod before
`worker/migrations/0057_event_sponsors.sql` is applied there, every event
create and update fails with a missing column. Staging already has 0057
(applied 2026-09-29, `.git/deploy-guard.log`).

Order, per `deploy-guard` step 5:

1. Back up prod D1.
2. `npx wrangler d1 migrations apply bethere-db --remote`.
3. Read the schema back with `PRAGMA table_info(events)`: `sponsors_json`
   must be TEXT, NOT NULL, DEFAULT '[]'.
4. Deploy the code.

The column is additive with a default, so applying it early is safe for the
code already on prod.

## 2. Re-take the event visual baselines from CI

`ec458a89` removed `e2e/__screenshots__/event-{mobile,desktop}-linux.png`,
because the seed now renders an organizer line and a sponsor row. After the
next CI run on the pushed branch, commit the two PNGs from the
`visual-baselines` artifact. No other baseline renders the organization.

## 3. Logo upload (feature follow-up, not blocking)

The editor takes an `https://` logo URL only. An upload to R2, like the poster
upload, would save organizers from hosting the image themselves. Scope and
priority are the owner's call; it is not demo-facing.

Also tracked elsewhere: `worker/src/db/events.rs` is 1,125 lines, over the
1,024 rule. Its split is in `.issues/052` and was waiting for F7 to land, which
it now has.
