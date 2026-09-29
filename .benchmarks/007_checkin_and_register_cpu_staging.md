# 007 — Worker cpuTime for the staff scan and agent registration, staging
Status: valid
Rung: `cpuTime` per request for `POST /api/checkin/{id}` (staff scan) and `POST /api/public/register`, plus a second sample of the deposit/escrow paths (028 M1, 010 P0.2)
Session: event-checkin-83, 1790615094
Commit: a3d4e7a9 (staging version `814738a8-98e6-4c63-b28f-b66cffaa3029`, read from each tail event's `scriptVersion` and matched in `wrangler deployments list --env staging`)
Gate: `/tmp/bethere-e2e-venv/bin/python /tmp/gap2/run.py` exit 0 (the `.issues/164` gap-2 rehearsal: fixture, register, pay, scan, on-chain check-in, refund, all asserted by the driver; not in the repo)
Lanes: single
Load: staging (`bethere-staging`), devnet RPC, one rehearsal, no other known traffic; isolate warmth unknown

## Method

`npx wrangler tail --env staging --format json` ran for the whole of one
`.issues/164` gap-2 rehearsal on 2026-09-28 (event
`agent-demo-meetup-1790614521`, made by `bethere-mcp/examples/demo_fixture.rs`
with `BETHERE_DEMO_END_MIN=8`). The attendee was registered and paid by the
`bethere-mcp` server over stdio (agent wallet `54GK…`), so it is a real,
unchecked D1 attendee: the fixture that `.plans/028` M1 blocker (1) was
missing. The staff scan used `dev-token` auth. Tail events were filtered to
this event, its attendee and the escrow/deposit/auth/register paths, and
grouped by method and path. 20 events.

## Result

| Path | n | cpuTime ms (min / max) | wallTime ms | Status |
|---|---|---|---|---|
| `POST /api/checkin/{id}` (first scan) | 1 | 13 | 754 | 200 |
| `POST /api/checkin/{id}` (repeat scan) | 1 | 8 | 50 | 400 "already checked in" |
| `POST /api/public/register` | 2 | 18 / 23 | 1200 / 1966 | 200 |
| `POST /api/auth/wallet/nonce` | 1 | 7 | 457 | 200 |
| `POST /api/auth/wallet/verify` | 1 | 9 | 588 | 200 |
| `POST /api/deposit/usdc/webhook` | 1 | 27 | 1785 | 200 |
| `POST /api/escrow/mark-checked-in` | 1 | 25 | 695 | 200 |
| `POST /api/escrow/refund` | 2 | 8 / 26 | 240 / 875 | 200 |
| `GET /api/public/ticket/{id}` | 1 | 25 | 2064 | 200 |
| `GET /api/deposit/usdc/confirm` | 1 | 18 | 1819 | 200 |
| `POST /api/deposit/usdc` | 1 | 15 | 534 | 200 |
| `POST /api/escrow/confirm-init` | 1 | 14 | 1269 | 200 |
| `PUT /api/events/{id}` | 1 | 11 | 961 | 200 |
| `POST /api/escrow/init` | 1 | 11 | 692 | 200 |
| `GET /api/deposit/status/{id}` | 2 | 8 / 11 | 83 / 275 | 200 |
| `GET /api/deposit/usdc/tx` | 1 | 9 | 53 | 200 |
| `GET /api/public/event/{slug}` | 1 | 8 | 89 | 200 |

Every outcome was `ok`. n is 1–2 per path, so these are single samples, not
a distribution; no tail claim is made.

## Notes

- The staff scan, the one path in M1 blocker (1), costs 13 ms on a first
  scan. That is above the plan's 10 ms figure, like the other write paths in
  `.benchmarks/002`, and it was not cut off.
- Registration (18–23 ms) and `GET /api/public/ticket/{id}` (25 ms) are also
  above 10 ms. Neither was in `.benchmarks/002`.
- `escrow/init` read 11 ms here and 31 ms in `.benchmarks/002`, and
  `escrow/refund` read 8 ms (a pre-end build) and 26 ms (the real one). With
  n = 1 and unknown isolate warmth, those gaps are not a trend. The three
  earlier fixture runs in the same tail, aborted by driver bugs before any
  attendee existed, put `escrow/init` at 10–35 ms and `confirm-init` at
  11–47 ms.
- Still unmeasured: `POST /api/claim/{token}`. Staging has no Crossmint key.
