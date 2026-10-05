# 180: A slow Crossmint mainnet mint is shown as "Minting Failed" (502)

**Status:** deployed to prod `dbd342c9` (2026-09-30, `deploy/production/20260930T064022Z`, version `60bcd02d`, session `event-checkin-b5`; staging ran the same tree first, and the claim 504/502 A/B was re-run on it). The served wasm carries `claim-pending` and "Still minting". Was: fixed on develop (`b99ee8f4` merge of `8a777b36`, plus `1598dd0f`; 2026-09-30, session `event-checkin-b5`). On staging `1598dd0f`; not on prod. Was: open (2026-09-30). Found by the owner on prod `438c392d` while claiming a badge; diagnosed from `wrangler tail` by session `event-checkin-b5`.

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

## Fix (option 1), 2026-09-30

- **Worker.** New `AppError::UpstreamPending` (504, fixed public body) and
  a typed `MintError { Pending, Failed }` in `domain/src/models/error.rs`. A
  poll timeout is `Pending` only when the mint can resume (provider id or KV
  marker). Campaign reward mints have neither, so their timeout stays a
  failure. `execute.rs` and `walkin.rs` map `Pending` to 504. The poll budget
  is unchanged (option 2 not taken).
- **Page.** `pages/claim/mint_retry.rs` retries a 504 up to 4 times, 3 s
  apart, with the spinner still up. After that it shows "Still minting" (EN/TH)
  on an amber card with a Pending badge. A 502 is never retried automatically.
- **Tests.** `domain/tests/mint_error.rs` (4), `worker/tests/mint_pending_guard.rs`
  (2), `frontend-leptos/tests/claim_mint_retry.rs` (3), all floored. Two
  mutants go red. The `log_pii_guard` pin follows the typed scrub.
- **Browser A/B on staging** (headless Chrome, 390 px). The claim POST was
  faked in the browser, and `nft_available` was flipped client-side because
  staging has no Crossmint key.
  - A 504 gave 5 POSTs (1 + 4 retries), the spinner during the retries, then
    "Still minting" (EN and TH, amber).
  - A 502 gave 1 POST, then "Minting Failed" (red), as before.
- **Not run:** a real slow mainnet mint. It cannot be triggered on demand.
  With the real Worker each attempt waits up to about 14 s, so the pending
  screen appears after about 75 s of retries.
