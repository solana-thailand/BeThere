# 156: The Solana blockhash cache never stored anything

**Status:** deployed 2026-09-28 — prod version `13b1b7eb` (`main` `0121ece2`, tag `deploy/production/20260928T002515Z`), shipping `3a6f69f2` and `a6343840` (session `event-checkin-af`). Staging ran it from 2026-09-27 as `0de8ec15`. Verified in `.benchmarks/004`: no 500s at concurrency 10–40; the one-shot retry was proven against a local stub RPC. A real Helius 429 being retried has not been seen in prod yet. Found during the plan 028 M1 burst run on staging.

## What happens

Every escrow transaction builder (deposit, init, mark-checked-in, refund,
close, claim-forfeited, rollover) gets a recent blockhash through
`solana_escrow::wire::get_latest_blockhash`. That function read a KV entry
`cache:blockhash` and, on a miss, fetched from the RPC and put the result back
with `expiration_ttl(30)`.

KV rejects any TTL under 60. Every put failed with:

```
KV PUT failed: 400 Invalid expiration_ttl of 30. Expiration TTL must be at least 60.
```

The failure was logged as a warning and the request carried on, so the cache
was always empty. Every transaction build made one `getLatestBlockhash` call
and one failed KV write. Nothing ever returned an error because of it, so it
never showed up.

## Evidence

- `wrangler tail --env staging`, 70 requests to `GET /api/deposit/usdc/tx`
  (event `agent-demo-meetup-1790482206`): 64 of 64 successful builds logged
  `blockhash cache write failed … Invalid expiration_ttl of 30`.
- At concurrency 10 (50 requests), 6 returned 500:
  `failed to build deposit TX: RPC call failed: HTTP 429 … rate limited`
  from Helius devnet. A cache hit would not have called the RPC at all.
- A/B on the decoded transaction blockhash: 4 staging requests 2.5 s apart
  (old code) carried 4 different hashes. Local `wrangler dev` with the fix:
  14 requests 2.5 s apart carried one hash for 0.9–18.5 s and a new one from
  21.7 s.

## Impact

- **RPC rate limit is the real risk.** Solana Pay wallets fetch the deposit
  transaction on scan. At a registration desk, a few simultaneous scans are
  enough to hit a 429, and the attendee sees a failed deposit. Prod escrow
  runs on devnet (deliberate), with the same Helius tier limits.
- A wasted KV operation on every build. The Free plan caps KV writes per day.
  I did not check whether a rejected put counts toward that cap.

## Fix

- `get_latest_blockhash` keeps the blockhash per isolate
  (`isolate_cache::Expiring`, keyed by RPC URL) for 20 s. KV is gone from the
  path, and so is the `kv` parameter on every builder and its call sites.
- KV at 60 s would be the wrong fix. An edge read can lag a write by up to
  60 s, so a served hash could be about two minutes old. That is past the
  150-block validity window (about 60–90 s), and `finalized` commitment
  already costs about 13 s of it. The 20 s budget is in
  `.plans/014_negative_results.md` §9.
- Guard: `worker/tests/kv_ttl_floor_guard.rs` resolves every
  `.expiration_ttl(…)` argument under `worker/src` (a literal, or a `const`
  that is a product of literals) and fails below 60. Unresolvable arguments
  must be on an allowlist with their clamp (today only `auth.rs`'s
  `.max(KV_MIN_TTL)`). A planted `const … = 30` fails it. Every other TTL in
  the tree is already ≥ 60.

## Follow-up: one retry on a transient failure (session `event-checkin-62`)

Per-isolate means each isolate makes its own first fetch. A burst that lands
on many cold isolates at once, or on one warm isolate right after its copy
expires, still makes one RPC call per request that misses. A 429 on that call
was a 500 to the wallet.

- `solana_escrow/blockhash.rs` (the blockhash code moved out of `wire.rs`,
  which was 1,021 lines after the change) retries `getLatestBlockhash` once
  when the first attempt gets 429, 5xx or no response. Other statuses and
  malformed bodies fail at once.
- The pause is 500–1,000 ms: `rpc_retry::retry_delay_ms` adds the failure's
  millisecond mod 501. Requests rejected in one burst fan out instead of
  retrying together. It is not random because `solana_escrow` is under the
  no-RNG guard (`tests/deterministic_monetary_code.rs`), which caught a
  `Math::random` draft.
- Tests: `worker/tests/rpc_retry.rs` (5). Dropping `429` from the transient
  set fails them.
- Measured in `.benchmarks/004`. On staging, Helius sent no 429, so the
  retry never ran there. The cache alone took concurrency 10 from six 500s to
  none. Against a local stub, one 429 became a 200, two 429s a 500, and a 400
  failed without a retry.

Declined, with reasons:
- **Share the hash across isolates through KV, stamped with its fetch time.**
  A KV read at a PoP is itself cached for at least 60 s (`cacheTtl` default
  and floor), so under a burst most readers would get a copy past the 20 s
  budget, reject it, fetch, and write again. On the Free plan, KV writes are
  capped per day. It would add writes and save little.
- **Single-flight within an isolate** (concurrent misses await one fetch).
  Workers do not let one request await I/O started by another request's
  context, and a waiter would fail if the owning request is cancelled.

## Not covered

- A real Helius 429 has not been met since the fix. From one client IP, the
  worker's own deposit limiter (60/min) answers first (`.benchmarks/004`).
- Prod: ships with the 3 Oct deploy (owner-gated).
