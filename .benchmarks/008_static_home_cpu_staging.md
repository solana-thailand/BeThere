# 008 — Worker cpuTime for the static home `GET /`, staging
Status: valid
Rung: `cpuTime` per request for `GET /` served by the Worker with the server-rendered opening (.plans/045 R4.9; bar: under 5 ms)
Session: event-checkin-d3, 1791532595
Commit: 1d726e50 (staging version `94c22465-11f2-4718-8edf-1511e50b851c`, read from each tail event's `scriptVersion`)
Gate: `cargo test -p event-checkin-worker --locked` exit 0 (incl. `tests/static_home.rs`)
Lanes: single
Load: staging (`bethere-staging`), 16 open public events in D1, one client, 30 requests 1 s apart, no other known traffic; isolate warmth unknown (the first request may have been cold)

## Method

`wrangler tail --env staging --format json` (wrangler 4.149.0) ran while
`curl` fetched `https://bethere-staging.solana-thailand.workers.dev/?b=N` for
N = 1..30, one a second. Tail events were filtered to those 30 URLs; 28 of
the 30 arrived in the tail.

## Result

| Path | n | cpuTime ms (min / median / p90 / max) | wallTime ms (median) | Outcome |
|---|---|---|---|---|
| `GET /` | 28 | 1 / 1 / 2 / 6 | 40 | 28 × ok |

The page is 17,288 bytes (local build, same tree), under the 20 KB budget.

## Reading

The bar (5 ms) holds at the median and p90; one request took 6 ms. The wall
time is the D1 read of the open events. What it costs that the asset path did
not: every home view is now a Worker request on the free plan's daily
allowance, where the static `index.html` was not.
