# 003 — Worker cpuTime under a small burst on GET /api/deposit/usdc/tx, staging
Status: valid
Rung: `cpuTime` and `outcome` per request when one write-adjacent path is hit back to back and at concurrency 10 (028 M1, the burst run named in `.benchmarks/002`)
Session: event-checkin-7c, 1790546831
Commit: 11036d96 (staging version `aca050b9-f155-4fde-b7c3-f20fc72df54f`, read from each tail event's `scriptVersion`)
Gate: `curl -fsS "https://bethere-staging.solana-thailand.workers.dev/api/deposit/usdc/tx?event_id=agent-demo-meetup-1790482206&attendee_id=burst-probe&wallet=9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"` exit 0 (returns a deposit transaction)
Lanes: single
Load: staging (`bethere-staging`), Helius devnet RPC, no other known traffic; isolate count unknown

## Method

`npx wrangler tail --env staging --format json` ran for the whole burst.
The path is unauthenticated, writes nothing to D1, and builds a full escrow
deposit transaction, so it can be repeated without seeding. Two steps with
`xargs -P … curl`: 20 requests one at a time, then 50 at concurrency 10. Four
more single requests came afterwards for the blockhash A/B in
`.issues/156`. 74 tail events in total, all on this path.

## Result

| Set | n | cpuTime ms (min / median / max) | > 10 ms | outcome |
|---|---|---|---|---|
| all | 74 | 6 / 11 / 17 | 42 | all `ok` |
| status 200 | 68 | 6 / 11 / 17 | 37 | all `ok` |
| status 500 | 6 | 10 / 11 / 13 | 5 | all `ok` |

No `exceededCpu` and no 1102. The six 500s were all
`RPC call failed: HTTP 429 … rate limited` from Helius devnet, in the
concurrency-10 step.

## Notes

- **The burst hit the RPC limit before any CPU limit.** Every build called
  `getLatestBlockhash`, because the KV blockhash cache never stored anything
  (`.issues/156`; every tail event logged the rejected put). The run stopped
  at concurrency 10: going higher would have measured Helius's rate limit,
  not the Worker's CPU.
- 42 of 74 requests were above 10 ms and none was cut off. This is the
  Free-plan tolerance described in `.benchmarks/002`, now over a 74-request
  burst rather than a handful of spaced ones. It still doesn't show where
  "consistently" starts, and it is one path at ≤ 17 ms, not the 31–34 ms
  paths.
- The measured CPU includes the failed KV put and its error logging on every
  request. With the `.issues/156` fix it should drop, but no after-number is
  claimed until the fix is on staging and this rung is rerun.
- n = 74, so there is no "p99". The tail support is the max (17 ms).
