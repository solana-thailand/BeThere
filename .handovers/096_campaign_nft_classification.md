# Handover 096: Campaign NFT Classification + Test Fix

## What Happened

Completed the campaign vs event NFT classification feature and fixed a pre-existing test failure.

### 1. Campaign NFT Classification (Issue #051 final phase)

**Problem:** The leaderboard and dev dashboard treated ALL NFTs as `event_nfts` with `campaign_nfts = 0`. Campaign NFTs should be worth 3x (3pts vs 1pt) but were never distinguished.

**Solution:**
- Added `campaign_collection_mints()` DB function in `worker/src/db/campaigns.rs` — queries active campaigns with `reward_type = 'nft_certificate'` and extracts `collection_mint` from `reward_config` JSON
- Added `classify_nfts()` helper in `wallet.rs` — matches each NFT's `grouping.collection_mint` against the campaign mint set
- Leaderboard now classifies NFTs before scoring (step 3 of the pipeline)
- Wallet NFTs endpoint returns `campaign_mints` in response so frontend can also classify
- Frontend `dev_dashboard.rs` uses `campaign_mints` to classify NFTs client-side

**Data flow:**
```
D1 campaigns → extract collection_mints → HashSet
DAS API items → check grouping.collection_mint against set
  Match → campaign_nfts (3pts)
  No match → event_nfts (1pt)
```

### 2. Pre-existing Test Fix

**Problem:** `test_last_column_letter_hardcoded` expected "AF" but `ColumnMapping::hardcoded()` has 33 columns (added `consent_marketing` at index 32 = "AG").

**Fix:** Updated assertion from "AF" to "AG" to match the actual 33-column mapping.

## Code Changes

| File | Change |
|------|--------|
| `worker/src/db/campaigns.rs` | Added `campaign_collection_mints()` |
| `worker/src/handlers/wallet.rs` | Added `classify_nfts()`, `HashSet` import, `campaign_mints` field in response, classification in leaderboard |
| `frontend-leptos/src/api/wallet.rs` | Added `campaign_mints: Vec<String>` to `WalletNftsResponse` |
| `frontend-leptos/src/pages/dev_dashboard.rs` | Replaced `campaign_nfts = 0` with real classification |
| `domain/src/models/attendee.rs` | Fixed test assertion "AF" → "AG" |
| `.issues/051_campaign_nft_rewards.md` | Updated all phases to [x], added NFT classification section, key files table |

## Build & Test

- `cargo check -p event-checkin-worker` — 0 errors, 0 warnings
- `cargo check` (frontend) — 0 errors, 0 warnings
- `cargo clippy -p event-checkin-worker` — 0 warnings
- `cargo test -p event-checkin-worker` — 117/117 passed
- `cargo test -p event-checkin-domain test_last_column` — 3/3 passed

## Refactoring Observation

21 files exceed the 1024-line guideline. Top offenders:
- `scanner.rs` (2108), `claim.rs` (1991), `event_form.rs` (1887), `admin.rs` (1828) — frontend
- `thb/handlers.rs` (1485), `write.rs` (1358), `events.rs` (1349) — backend
- `event.rs` (1307), `attendee.rs` (1239) — domain

This is a separate task — not blocking current work.

## Remain Work

- Deploy frontend with `trunk build --release` + upload
- Issue #050 DO deployment still blocked by CF API error 10013
