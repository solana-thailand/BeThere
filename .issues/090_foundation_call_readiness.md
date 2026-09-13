# 090 — What is not in production before the 15 Sep Foundation call

**Status:** open — the gap is a deploy, not code
**Found:** 2026-09-13, checking BeThere against `solana-thailand-devrel-helper`
**Severity:** high — DevRel's top-priority ask is merged and invisible

## The one-line version

Everything DevRel asked for in `reports/phase-2/BETHERE-ASKS.md` item 2 is
written, reviewed, merged and **not deployed**. Production is five merges
behind `develop`, and the missing piece is exactly the satisfaction question
set they named as their single highest-value item.

## Verified, not remembered

Production runs version `20499a78-fda8-4b37-878c-48500d20ab3f`, deployed
2026-09-13 08:05 UTC. `develop` is at `3df7e26`, five merge commits later.

The check that settles it is not the timestamp — it is the shipped binary.
Prod serves `event-checkin-frontend-d954a4af68c66138_bg.wasm` (5,095,660 bytes).
Searching it:

| String | Hits in prod wasm |
|---|---|
| `post-event-register` | 3 |
| `post.satisfaction.overall` | **0** |

So the post-event **route** is live and the **questions are not**. A person who
scans a recap QR code today reaches a working form that asks them nothing about
the event.

## Undeployed, in merge order

| PR | Issue | What is missing from prod |
|---|---|---|
| #88 | 084 | docs only |
| #90 | **087** | **the four `post.*` satisfaction questions in the form** |
| #91 | 088 | claim-flow test contract |
| #92 | 089 | event invariant audit |
| #93 | **080** | per-kind notification delivery windows + **migration 0035** |

Migration 0035 has not been applied to prod; `wrangler d1 migrations list DB
--remote` showed prod at 0034 with 0035 the only pending one. It rebuilds
`notification_outbox`, which has **0 rows** in prod — so the migration is being
applied to an empty table, which is the cheapest it will ever be.

## DevRel's picture of prod is stale, in our favour

`BETHERE-ASKS.md` was written 2026-09-13 and reports
`per_open=0 ... of 16` with only RTM #1 completed. Read from a prod D1 export
taken the same afternoon, that is no longer true:

- **12 events** are `completed` with `post_event_registration_open = 1`.
- `islanddao-v4-demo` is completed with the flag off (not a Solana Thailand
  event — leaving it is the right default, but it should be a decision).
- RTM #6 is correctly still `active`, ending 2026-09-27 09:00 UTC.

So item 1 is done and the reply's "tell us which events" question has been
answered by action. Their doc should be corrected before the call rather than
read from.

## What is still zero, and why that matters

```
attendees=477  retrospective=0  registration_responses=2103
post.*_answers=0  notification_outbox=0  notification_enrollments=0
```

The retrospective path has never been exercised end to end in production. Not
a defect — nothing has been deployed that could exercise it. But it means the
first real use will be the first test, and that is worth saying out loud on the
call rather than presenting the feature as proven.

## Remaining data gaps, smaller than the deploy

1. **Two past events are still `draft`**, so their post-event form cannot open
   at all: `solana-in-latent-space-part-7` (ended 2026-07-29) and
   `comfyui-thailand-1st-meetup` (ended 2026-09-12). Both need the same
   Completed transition the other twelve got.
2. **`post_event_registration_until_ms` is NULL on all twelve.** No deadline
   means the form never closes. That is a defensible default for a
   retrospective backfill, but it is currently an accident rather than a
   choice, and `.issues/080`'s survey withdrawal logic keys off it.
3. **Six events have no `poster_url`** — all six Latent Space sessions plus
   `intro-to-vibing-on-solana`.

## Why this is not fixed in this session

`bash build.sh` and `bash deploy.sh` are both refused by the Claude Code auto
mode permission classifier, so the frontend cannot be rebuilt and neither
`staging` nor `prod` can be deployed from here. A prod D1 backup was taken
first and is at `~/bethere-backups/bethere-db-20260913.sql` (1.6 MB, outside
the repo — it carries PII).

## The order to do it in

1. `cd frontend-leptos && bash build.sh`
2. `cd worker && bash deploy.sh staging` — verify 0035 applies to an empty
   `notification_outbox`, then smoke-test Content-Type on the wasm asset.
3. `cd worker && bash deploy.sh` — prod.
4. Re-run the wasm string check above; `post.satisfaction.overall` must now
   return a non-zero count. That is the pass condition, not HTTP 200.
5. Complete the two draft events; decide the deadline question.
