# 180: A slow Crossmint mainnet mint is shown as "Minting Failed" (502)

**Status:** open (2026-09-30). Found by the owner on prod `438c392d` while claiming a badge; diagnosed from `wrangler tail` by session `event-checkin-b5`.

## What happened

The owner claimed a badge on prod. The first attempt showed "Minting Failed —
API error (502): external service error: crossmint returned 502". A retry a
minute later succeeded.

The retry's log says `crossmint: resuming in-flight mint (idempotent retry)`,
with the same `nft_id` as the first attempt, then `claim fulfilled`. So
Crossmint had accepted the first request, and the mint was still running when
our poll budget ran out. One NFT was minted, not two: the resume path
(`solana.rs:158`) worked as designed.

## Why it shows as a failure

- `mint_compressed_nft` polls `CROSSMINT_MAX_POLLS = 12` times, every
  `CROSSMINT_POLL_DELAY_MS = 1200` ms (about 14 s), then returns
  `Err("crossmint mint still pending after 12 polls …")` (`solana.rs:261`).
- `claim/mint/execute.rs:410` (and `walkin.rs:137`) map **every** mint error to
  `AppError::External { service: "crossmint", status: 502 }`. A pending mint
  and a rejected mint look the same to the page.
- The page shows the `Minting Failed` title (`locales/en/claim.json:117`).

## Why it matters now

`.plans/033`: the badge clip for the 8 Oct take is filmed on prod, which mints on
Crossmint mainnet. A "Minting Failed" screen on camera, when the mint is only
slow, is the likely outcome if mainnet takes more than about 14 s again.

## Options

1. Return a distinct "still minting" result for the pending case (for example
   202 with a `pending` flag). The page says "still minting, this can take up to
   a minute" and retries on its own. The pending marker is already kept, so a
   retry resumes and does not double-mint.
2. Raise the poll budget (for example 25 polls, about 30 s). Waiting uses wall
   time, not CPU, so the free-plan 10 ms CPU cap is not the limit. This is a
   smaller change, but a slow mint still ends on the failure screen.
3. Both.

Recommended: 1, with 2 as the one-line stopgap if 1 cannot land and be
rehearsed before the 6 Oct freeze.
