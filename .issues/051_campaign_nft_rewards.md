# Issue 051: Campaign NFT Rewards + On-Chain Developer Dashboard

## Summary

Complete the campaign NFT reward system with layered rewards (per-event + campaign completion), auto-progress tracking on check-in, on-chain verification via Helius DAS API, and a public developer ranking dashboard.

## Motivation

- Per-event NFT badges exist but campaign completion has no actual NFT mint
- Campaign progress tracking requires manual updates — should auto-update on check-in
- No on-chain verification or public ranking exists
- NFTs as the score system (no separate staking needed) — simple, verifiable, gamified

## Architecture

### Reward Flow
```
Event Check-in → Per-event cNFT (already working via claim.rs)
Campaign Complete → Campaign cNFT (new — premium collection)
Public Dashboard → Read wallet cNFTs via Helius DAS getAssetsByOwner
Ranking → Aggregate NFT counts, weighted scoring
```

### Auto-Progress Flow
```
Check-in → D1 write → check if event belongs to campaign → upsert developer_campaign_progress
```

### NFT Classification
```
Campaigns with reward_type = "nft_certificate" store collection_mint in reward_config JSON.
Leaderboard + dashboard match each NFT's grouping.collection_mint against campaign mints.
Match → campaign NFT (3pts). No match → event NFT (1pt).
```

## Phases

### Phase 1: Auto-Progress on Check-In
- [x] After check-in D1 write, query `campaign_events` for the event_id
- [x] If found, upsert `developer_campaign_progress` with incremented events_completed
- [x] Non-blocking — failures are logged but don't block check-in

### Phase 2: Campaign NFT Mint on Reward Claim
- [x] `claim_campaign_reward` mints cNFT via existing `mint_compressed_nft()`
- [x] Uses campaign-specific collection/metadata (from `reward_config` JSON)
- [x] Stores asset_id and signature in progress row (new columns)
- [x] Migration: add `reward_asset_id` and `reward_signature` to developer_campaign_progress

### Phase 3: On-Chain Verification API
- [x] New `worker/src/solana.rs` — `get_assets_by_owner()` via Helius DAS API
- [x] New `worker/src/handlers/wallet.rs` — wallet lookup + NFT inventory endpoints
- [x] GET /wallet/{address}/nfts — returns all cNFTs owned by wallet + campaign_mints
- [x] GET /wallet/leaderboard — ranking by NFT count (event + campaign weighted)

### Phase 4: Public Developer Dashboard
- [x] Frontend page: connect wallet, show NFTs, show ranking
- [x] Tier system: Collector (3+ NFTs), Dedicated (5+), Legend (10+)
- [x] Campaign NFT = 3x weight in scoring
- [x] NFT classification via collection_mint matching against active campaigns

## Database Changes

### Migration 0008: Campaign Reward Columns
```sql
ALTER TABLE developer_campaign_progress
  ADD COLUMN reward_asset_id TEXT;

ALTER TABLE developer_campaign_progress
  ADD COLUMN reward_signature TEXT;
```

## Key Files

| File | Purpose |
|------|---------|
| `worker/src/handlers/wallet.rs` | Wallet NFT lookup + leaderboard with KV caching + NFT classification |
| `worker/src/handlers/campaigns.rs` | Campaign CRUD + claim_campaign_reward (mints cNFT) |
| `worker/src/db/campaigns.rs` | Campaign DB queries + `campaign_collection_mints()` |
| `worker/src/db/developers.rs` | `list_developers_with_wallets()` for leaderboard |
| `worker/src/solana.rs` | DAS API + mint_compressed_nft + wallet validation |
| `frontend-leptos/src/pages/dev_dashboard.rs` | Developer dashboard with NFT classification |
| `frontend-leptos/src/api/wallet.rs` | Wallet API client types + tier helpers |
| `frontend-leptos/src/pages/campaigns_page.rs` | Admin campaign form with structured reward_config |

## Status
- [x] Phase 1: Auto-Progress on Check-In
- [x] Phase 2: Campaign NFT Mint
- [x] Phase 3: On-Chain Verification API
- [x] Phase 4: Public Developer Dashboard
