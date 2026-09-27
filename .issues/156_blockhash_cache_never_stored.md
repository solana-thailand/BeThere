# 156: The Solana blockhash cache never stored anything

**Status:** fixed on develop 2026-09-28 (session `event-checkin-7c`). Not deployed. Staging and prod still run the old code. Found during the plan 028 M1 burst run on staging.

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

## Not covered

- Per-isolate means each isolate makes its own first fetch. A burst that
  lands on many cold isolates at once still makes one RPC call per isolate.
  Far fewer calls than before, but not zero.
- The burst run stopped at concurrency 10. CPU was 6–17 ms per request, all
  `ok`, and no `exceededCpu`. Going higher would have only measured Helius's
  rate limit. Rerun it after this deploys (plan 028 M1).
