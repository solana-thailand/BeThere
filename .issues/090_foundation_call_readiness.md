# 090 — What is not in production before the 15 Sep Foundation call

**Status:** deployed 2026-09-13 (prod `253717a5`) — two follow-ups remain, see the outcome section
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
2. **`post_event_registration_until_ms` is NULL on all twelve** — the form
   never closes. ~~Currently an accident rather than a choice.~~ **Wrong, and
   corrected 2026-09-13:** NULL is the designed value.
   `post_event_registration_deadline_passed` documents it as *"`None` = open
   indefinitely, so it never passes. Plan 008 — Phase 3"*
   (`domain/src/models/event/config.rs:369`). Setting a value only schedules a
   close date in advance. For a retrospective backfill across twelve past
   events, NULL is correct and nothing needs changing. See `.issues/091`.
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

---

## Deployed 2026-09-13 — outcome

**Status: closed for the deploy; two follow-ups below.**

| | |
|---|---|
| Prod version | `253717a5-9334-484c-9ac5-85ef7675fd13` (was `20499a78`) |
| Prod commit | `0c56c41` |
| Frontend bundle | `event-checkin-frontend-61180dc45a1b247a` (was `d954a4af68c66138`) |
| Migration `0035` | applied to `bethere-db-staging` **and** `bethere-db`, 27 commands each |

### The pass condition was met

The check in the section above was a string search against the served wasm.
The stronger form was available once the artifact existed locally, so that is
what was run: `cmp` between the downloaded prod object and the local
`dist/` build — **byte-identical**. That build was verified before deploying to
contain `post.satisfaction.overall` (1) and `post.would_return` (1), against
**0** in the outgoing prod bundle. Staging served the same bytes first.

Effect of the migration verified on both databases by reading the rebuilt
constraint back, not by trusting the exit code:

```
kind IN ('registration','reminder','deposit_confirmed','deposit_rejected','survey')
```

Prod `/api/health` after: `status: ok`, `d1.connected: true`, `attendees 477`,
`events 16` — unchanged by the table rebuild. `deploy.sh`'s own Content-Type
smoke test passed on both environments.

### The preflight gate was bypassed, again

`flow-harness/results/.last-green` still does not exist, so prod was deployed
with `--force --reason`. That entry is the **fifth bypass today**;
`worker/scripts/.preflight-bypass.log` now has five lines and zero green runs.

`.issues/084` predicted exactly this — *"either prod cannot be deployed, or the
gate gets bypassed routinely and stops meaning anything."* The second branch is
now the observed behaviour. A gate that is bypassed on every deploy is not a
gate, and the honest options are to finish 084 or to remove it. Continuing to
`--force` past it is the one option that costs something and buys nothing.

### Correction: completing the two draft events buys nothing today

The section above lists it as a gap because their post-event form cannot open.
That is true and incomplete — checked afterwards, both have **0 attendees and
0 check-ins**:

| slug | status | attendees | checked in |
|---|---|---|---|
| `solana-in-latent-space-part-7` | draft | 0 | 0 |
| `comfyui-thailand-1st-meetup` | draft | 0 | 0 |

So there is nobody who could fill the form, and `.issues/080`'s survey enqueue
trigger — which fires only for enrolled attendees with a non-null
`checked_in_at` — would select nobody. Completing them is state hygiene, not a
fix for a reachable dead end. It should not be presented as one on the call.

It is also **blocked from this session regardless**: the transition goes through
`PUT /api/events/{id}` in the authed router, and prod runs `DEV_MODE=0` with
`JWT_SECRET` held as a Cloudflare secret (`state.rs`, `get_secret`), so no token
can be minted here. It needs an organizer login in the UI — and the decision of
whether two events with no attendees should be completed at all.

### What is now live and has still never been used

`registration_responses` has **0** rows with a `post.%` key and `attendees` has
**0** rows with `participation_type = 'retrospective'`. The questions are
reachable in production as of this deploy; the first person to answer one will
be the first end-to-end exercise of the path. Worth saying plainly on the 15th
rather than presenting the feature as proven.
