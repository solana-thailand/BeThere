# 002 — Worker cpuTime on the escrow/deposit hot paths, staging
Status: valid
Rung: `cpuTime` per request for escrow init, deposit, escrow check-in and refund (028 M1, 010 P0.2)
Session: event-checkin-6b, 1790538202
Commit: 11036d96 (staging version `aca050b9-f155-4fde-b7c3-f20fc72df54f`, read from each tail event's `scriptVersion`)
Gate: `PATH=/tmp/bethere-e2e-venv/bin:$PATH bash scripts/e2e_devnet_test.sh --skip-setup --non-interactive` exit 0
Lanes: single
Load: staging (`bethere-staging`), devnet RPC, one e2e run, no other known traffic; isolate warmth unknown

## Method

`npx wrangler tail --env staging --format json` ran for the whole of one
`scripts/e2e_devnet_test.sh` run (event `e2e-escrow-1790543188`, full
escrow cycle: init, confirm-init, USDC deposit, webhook verify, on-chain
mark-checked-in, refund, PDA closed). Each tail event carries `cpuTime` and
`wallTime` in whole milliseconds. Events were grouped by method and path.
11 events in total.

## Result

| Path | n | cpuTime ms (min / max) | wallTime ms | Status |
|---|---|---|---|---|
| `POST /api/deposit/usdc/webhook` | 2 | 8 / 34 | 45 / 2465 | 200 |
| `POST /api/escrow/init` | 1 | 31 | 2184 | 200 |
| `POST /api/events` | 1 | 24 | 4490 | 200 |
| `POST /api/escrow/confirm-init` | 1 | 19 | 1925 | 200 |
| `POST /api/deposit/usdc` | 1 | 19 | 746 | 200 |
| `POST /api/escrow/mark-checked-in` | 1 | 15 | 439 | 200 |
| `POST /api/escrow/refund` | 1 | 15 | 931 | 200 |
| `GET /api/deposit/usdc/tx` | 1 | 14 | 454 | 200 |
| `GET /api/health` | 1 | 7 | 49 | 200 |
| `GET /api/auth/me` | 1 | 5 | 5 | 200 |

Every event's `outcome` was `ok`. A separate read-only probe the same
session (6 requests: `GET /api/health`, `GET /api/public/events`) gave
6–10 ms each.

## Notes

- **Every write path measured is above the 10 ms figure the plans treat as
  binding** ([[free-plan-cpu-cap-is-binding]]), and none was cut off: all
  `ok`. **Why (partly settled 2026-09-28):** Cloudflare's limits page
  (developers.cloudflare.com/workers/platform/limits, read 2026-09-28) says
  the Free plan's 10 ms is "CPU time per HTTP request", but "each isolate has
  some built-in flexibility to allow for cases where your Worker infrequently
  runs over the configured limit. If your Worker starts hitting the limit
  consistently, its execution will be terminated". Global-scope start-up has
  its own 1 s limit. A cut-off shows as error 1102 and outcome `exceededCpu`.
  So `ok` on a handful of spaced requests is what that rule predicts and says
  nothing about a sustained burst (event-day check-in). `worker/wrangler.toml`
  sets no `[limits]`. Still unchecked: the account's actual plan. Until that
  and a burst run settle it, treat 14–34 ms write paths as at risk under
  load, not as safe.
- n is 1 or 2 per path, so there is no tail. Cold and warm isolates are mixed
  and not identified.
- Not covered: the staff scan `POST /api/checkin` (off-chain check-in) and
  `POST /api/claim/{token}` (staging has no Crossmint key, so a claim fails
  before the mint). Both need their own rung.
- The run left event `e2e-escrow-1790543188` on staging, as every e2e run
  does, and spent devnet SOL only.
