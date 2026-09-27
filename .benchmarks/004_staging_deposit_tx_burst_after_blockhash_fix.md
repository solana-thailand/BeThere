# 004 — The deposit-tx burst from 003, rerun after the blockhash fixes, plus a stub-RPC retry check
Status: valid
Rung: `cpuTime`, `outcome` and status per request on `GET /api/deposit/usdc/tx` at concurrency 1/10/20/40 once the per-isolate blockhash cache and the one-shot retry are live (028 M1, the rerun named in `.benchmarks/003` and `.issues/156`)
Session: event-checkin-62, 1790548470
Commit: a6343840 (staging version `0de8ec15-9acc-43c5-8971-518bff756e84`, read from each tail event's `scriptVersion`)
Gate: `curl -fsS "https://bethere-staging.solana-thailand.workers.dev/api/deposit/usdc/tx?event_id=agent-demo-meetup-1790482206&attendee_id=burst-probe&wallet=9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"` exit 0 (returns a deposit transaction)
Lanes: single
Load: staging (`bethere-staging`), Helius devnet RPC, no other known traffic; one client IP

## Method

Same path, event and query as `.benchmarks/003`, and the same
`xargs -P … curl` steps with `npx wrangler tail --env staging --format json`
running throughout. The steps were 20 requests at concurrency 1 and 50 at 10
(003's two steps), then 100 at 20 and 100 at 40. The tail caught 271 events:
the 270 burst requests plus one follow-up probe. Every event carried
`scriptVersion` `0de8ec15`.

A cache hit logs `using cached blockhash`. A request without that line called
the RPC. `build_config` logs its DEV_MODE warning once per isolate (it sits
behind a `OnceLock`), so that line marks the first request in a new isolate.

## Result

| Step | n | 200 | 429 (own limiter) | 500 | RPC calls (cache misses) |
|---|---|---|---|---|---|
| s1 conc 1 | 20 | 20 | 0 | 0 | 2 |
| s2 conc 10 | 50 | 50 | 0 | 0 | 0 |
| s3 conc 20 | 100 | 52 | 48 | 0 | 4 |
| s4 conc 40 | 100 | 14 | 86 | 0 | 12 |

| Set | n | cpuTime ms (min / median / max) | > 10 ms | outcome |
|---|---|---|---|---|
| all | 271 | 4 / 7 / 94 | 72 | all `ok` |
| 200, cache hit | 118 | 5 / 9 / 17 | 40 | all `ok` |
| 200, cache miss, first request in a new isolate | 16 | 49 / 80 / 94 | 16 | all `ok` |
| 200, cache miss, warm isolate | 2 | 14, 25 | 2 | all `ok` |
| 429, deposit rate limiter | 135 | 4 / 6 / 19 | 14 | all `ok` |

- **No 500s.** In 003, 6 of 50 requests at concurrency 10 were 500s, from a
  Helius 429 on `getLatestBlockhash`. Here the concurrency-10 step went
  50/50 with no RPC call at all: the per-isolate cache (`3a6f69f2`) served
  every build.
- **Helius never returned a 429 in this run.** The 18 RPC calls all
  succeeded and no event logged `getLatestBlockhash failed, retrying once`.
  The retry (`a6343840`) was deployed but never exercised on staging. See
  the stub check below.
- **The 429s are the worker's own limiter, not Helius.** They carry the body
  `{"error":"rate_limit_exceeded","retry_after_secs":60}` and log nothing past
  the middleware. `RATE_LIMIT_DEPOSIT` allows 60 requests a minute per IP, and
  a single client IP sent 270 in about 8 s.
- **No `exceededCpu`, no 1102.** For comparison, 003's all-miss run
  (every build made an RPC call and a failed KV put) was 6 / 11 / 17 ms.

## Stub-RPC retry check (local)

A one-client burst could not trigger a Helius 429 (see Notes), so the retry
was driven locally. `wrangler dev --local` (code at `a6343840`, a copy of
the `.issues/156` local D1/KV state, event `bhprobe`) ran with
`--var HELIUS_RPC_URL:http://127.0.0.1:8898`. A stub answered
`getLatestBlockhash` from a fixed queue. The three requests were 21 s
apart, so each started on a cache miss.

| Scenario | Stub answers | Worker response | Stub hits | Logged retry delay |
|---|---|---|---|---|
| A: one 429 | 429, then OK | **200** in 0.72 s | 2 | 655 ms |
| B: two 429s | 429, 429 | 500 in 0.84 s | 2 | 816 ms |
| C: 400 | 400 | 500 in 0.01 s | 1 | none (not retried) |

The transaction returned in A carries the stub's blockhash
(`4uQeVj5t…`, decoded from the recent-blockhash field), so the success came
from the second attempt. B retries only once. C does not retry a
non-transient status. Both retry delays fall in the designed 500–1,000 ms
window.

## Notes

- **The direct answer: at this load the cache, not the retry, removed the
  500s.** The retry turns a single 429 into a 200 (stub A). It has not yet
  met a real Helius 429.
- **A real Helius 429 is now hard to reach from one client.** A burst needs
  many cache misses at once (new isolates), and the worker's own deposit
  limiter caps one IP at 60 requests a minute. I did not drive Helius
  directly to force a 429: the key may be shared with prod.
- **A new isolate's first request cost 49–94 ms CPU**, 5–9× the Free-plan
  10 ms. There were 16 of them, all `ok`. The number includes the isolate's
  first-request work (at least `build_config`) plus the RPC fetch. I did not
  split the two. The two warm-isolate misses cost 14 and 25 ms. It is the largest per-request CPU seen on this path so far. It is
  still not a cut-off, and the question of where "consistently over" starts
  remains open (028 M1).
- Cache-hit CPU (median 9, max 17) is close to 003's all-miss numbers
  (median 11, max 17). The failed KV put that 003 paid on every request did
  not show up as a large CPU saving. n is small, and there is only one lane.
- n = 271, and the sets are uneven. There is no "p99"; the tail support is
  each set's max.
