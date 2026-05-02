# 026 — cNFT Mint Fix & Full E2E Test

## What Happened

The E2E browser test suite (from handover 025) was extended to 10 steps. The final mint step was failing due to three distinct root causes, all discovered and fixed in this session.

## Branch

- `main` (`68087b1`) — fix ahead of `develop` by 1 commit

## Root Causes & Fixes

### 1. Helius `null` Field Deserialization
- Helius `mintCompressedNft` API returns `null` for `signature`, `assetId`, and `minted` when on-chain transaction fails
- Original `HeliusMintResult` had `signature: String` — no `Option`, no `#[serde(default)]`
- Caused `serde_json` error: `"invalid type: null, expected a string"`
- **Fix:** Changed all fields to `Option<String>` / `bool` with `#[serde(default)]`

### 2. NFT Name Exceeds 32-Character Limit
- Resolved NFT name was 65 chars: `"BeThere - Solana x AI Builders: The Road to Mainnet #1 (Bangkok)"`
- Bubblegum/Metaplex enforces 32-char max → on-chain error 6012 `MetadataNameTooLong`
- **Fix:** `EventConfig::nft_name()` now truncates to 29 chars + `"..."` (total 32)

### 3. Localhost URLs in NFT Metadata
- `NFT_IMAGE_URL` and `NFT_METADATA_URI` in `.dev.vars` pointed to `http://localhost:8787/...`
- Helius servers cannot resolve localhost → on-chain mint fails
- Wrangler caches `.dev.vars` at startup, KV persists event config across restarts
- **Fix:** Cleared `.dev.vars`, restarted Wrangler, used `PUT /api/events/default` to clear cached KV

### 4. `response.json()` Silent Failures
- `worker::Response::json()` silently fails on Workers runtime when serde deserialization fails
- **Fix:** Changed to `response.text()` + `serde_json::from_str()` for better error diagnostics

## E2E Test Result

```
Step  1: Server Health                          ✅ PASS
Step  2: Authentication (HMAC-SHA256 JWT)       ✅ PASS
Step  3: List Attendees (61 approved, 31 in)    ✅ PASS
Step  4: Check-in Attendee                      ⏭️ SKIP (all checked in)
Step  5: Claim Token Lookup                     ✅ PASS
Step  6: Submit Quiz (5/5 correct, 100%)        ✅ PASS
Step  7: Configure Adventure (admin)            ✅ PASS
Step  8: Complete Adventure (level_01)          ✅ PASS
Step  9: Mint Compressed NFT                    ✅ PASS
Step 10: On-chain Verification & Cost Analysis  ✅ PASS
```

Result: **9 PASS, 0 FAIL, 1 SKIP** ✅

## Files Changed

| File | Change |
|------|--------|
| `worker/src/solana.rs` | Null-safe deserialization, raw body parsing, better error messages |
| `domain/src/models/event.rs` | NFT name 32-char truncation in `nft_name()` |
| `scripts/e2e/test_full_e2e.sh` | Full 10-step E2E browser test script |

## On-Chain Assets Verified

| Asset ID | Owner | Network |
|----------|-------|---------|
| `6F4Cb7hyqWWXaJtjoPEUY99fzCgxpovLvH6EdA9Q1aUz` | `BBc7Vvbe...7L6W6` | devnet |
| `DehVNwud4wD1tN8NzmTouiya6zLNEvreTH6A8UcVDcwV` | `FKmUzuZQ...Eq12` | devnet |
| `9h2EPb6sW5s4dL3wYhKYM2SQFuon71pQiGGhcDKtAHHk` | `3uhUFUq5...pREM` | devnet |

## Mint Cost

| Metric | Value |
|--------|-------|
| Network Fee | 5,051 lamports (~0.000005051 SOL) |
| Compute Units | ~41,000–49,000 |
| USD Cost | ~$0.000869 |

## Architectural Insight

Wrangler dev server caches `.dev.vars` at startup and persists KV across restarts:
- Env var changes require **Wrangler restart**
- Event config changes require **KV wipe** or **API update call**
- For production, NFT URLs **must** be publicly accessible

## Remaining Work

### 🔴 Blocking
- [ ] Merge `main` → `develop` (1 commit behind)
- [ ] Upload NFT badge image to Arweave (waiting on design)
- [ ] Set public `NFT_IMAGE_URL` / `NFT_METADATA_URI`

### 🟡 High Priority
- [ ] Fix cost analysis parsing in Step 10 (getSignaturesForAsset format mismatch)
- [ ] Handle "all checked in" in E2E script
- [ ] Add `nft_name` > 32 char warning in admin UI
- [ ] Shorten NFT name template (use slug/abbreviation)

### 🟢 Future
- [ ] Add E2E test to CI
- [ ] Create dedicated test Google Sheet
- [ ] Browser automation test (Playwright/Cypress)
- [ ] Investigate Helius tree capacity for 1000+ attendee events

## How to Dev/Test

```bash
# Start dev server
cd worker && ./deploy.sh dev

# Run full 10-step E2E test
./scripts/e2e/test_full_e2e.sh

# Run devnet test suite
./scripts/e2e/test_devnet.sh
```

## Issues Ref

- `.issues/007_devnet_e2e_test.md` — E2E test tracking
- `.issues/008_nft_config_and_production_readiness.md` — NFT config + production checklist
