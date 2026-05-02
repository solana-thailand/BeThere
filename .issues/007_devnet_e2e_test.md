# Issue 007: Devnet E2E Test + Helius API Bug Fix

## Status: ✅ Automated Tests Pass — Manual Tests Pending

## Critical Bug Found & Fixed

The Helius `mintCompressedNft` RPC was **failing silently** in production because our worker sent two undocumented parameters:

| Param | Result | Fix |
|-------|--------|-----|
| `priorityFee` | `Invalid request params` (-32602) | **Removed** — not in [Helius API docs](https://www.helius.dev/docs/api-reference/mint/mintcompressednft) |
| `tree` | `Invalid request params` (-32602) | **Removed** — Helius always mints to its own managed tree |

### Verified Working
- ✅ `mintCompressedNft` without `priorityFee` or `tree` → mint succeeds (~2s)
- ✅ 5 devnet mints confirmed via `getAsset` DAS API
- ✅ Assets visible with correct name, symbol, owner on Helius indexer

### Files Changed
- `worker/src/solana.rs` — Removed `priorityFee` and `tree` params from RPC call
- `frontend-leptos/src/pages/events_page.rs` — Updated merkle_tree hint text

## Devnet Assets Created

| Asset ID | Name | Tx Signature |
|----------|------|-------------|
| `G1PhU9wTgMGcZiBWgeMk5Tj2p79YQ4sv4P5p8DuxCvUU` | BeThere Devnet Test | `44n2FKbA...` |
| `GBKR4Q2eoFoBgZQuwxdxPw6tacjnsnT6QNnV78kh3Tff` | BeThere Badge - Devnet Test | `wAtPvPwh...` |
| `GNF4mp3fR6x9jjcPHpj3R9Dxf1FqmbsFCMpz8sNXxsaJ` | No Priority Test | `2ZdSVTWQ...` |

## Devnet E2E Demo Checklist

### Prerequisites
- [x] Helius API key configured in `worker/.dev.vars`
- [x] Devnet SOL funded (1.77 SOL on payer keypair)
- [x] CLI tree created and verified (3 mints, 16,381 remaining)
- [x] Critical Helius API bug fixed (`priorityFee` / `tree` params)

### Local Server Test
- [x] `wrangler dev` starts successfully
- [x] `GET /api/health` returns `{"status": "ok"}`
- [x] `GET /api/claim/{fake-token}` returns `{"success": false, "error": "claim token not found"}`

### Full Flow (requires Google Sheets data)
- [ ] **Login** — Admin logs in via Google OAuth
- [ ] **Seed event** — `POST /api/events/seed` creates event from env config
- [ ] **Configure quiz** — Admin adds quiz questions via UI
- [ ] **Check-in** — Staff scans QR code → attendee marked in Sheet
- [ ] **Claim link** — Attendee receives `https://...claim/{token}`
- [ ] **Quiz** — Attendee passes quiz
- [ ] **Adventure** — Attendee completes required adventure levels
- [ ] **Wallet connect** — Attendee enters Solana wallet address
- [ ] **Mint** — Worker calls Helius `mintCompressedNft` → cNFT minted
- [ ] **Verify** — Asset appears on [Solana Explorer (devnet)](https://explorer.solana.com?cluster=devnet)

### Automated E2E Script (`scripts/e2e/test_devnet.sh`)
All 7 automated tests pass as of `a0d7cba`. See handover 025 for details.

## Related
- Handover 025 — Security audit + E2E test script
- Issue 008 — NFT config + production readiness

### Demo Script for CTO

```bash
# Terminal 1: Start worker
cd worker && npx wrangler dev --port 8787

# Terminal 2: Test Helius mint directly (proves minting works)
curl -s "https://devnet.helius-rpc.com/?api-key=$HELIUS_API_KEY" \
  -X POST -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "cto-demo",
    "method": "mintCompressedNft",
    "params": {
      "name": "BeThere Attendance Badge",
      "symbol": "BETHERE",
      "description": "Proof of attendance at Solana x AI Builders event",
      "owner": "<WALLET_ADDRESS>",
      "imageUrl": "https://bethere.solana-thailand.workers.dev/badge.png",
      "externalUrl": "https://bethere.solana-thailand.workers.dev",
      "sellerFeeBasisPoints": 0,
      "confirmTransaction": true
    }
  }'

# Verify on explorer
open "https://explorer.solana.com/address/<ASSET_ID>?cluster=devnet"
```

## Architecture Notes

### Helius Tree vs Own Tree
| Aspect | Helius Managed Tree | Own Tree (CLI-created) |
|--------|-------------------|----------------------|
| Minting | ✅ Via `mintCompressedNft` RPC | ✅ Via CLI `mint` command |
| Cost per mint | Free (included in Helius plan) | Free (you paid for the tree) |
| Control | Limited (Helius manages) | Full (you are tree authority) |
| Data ownership | Helius indexes automatically | Helius indexes automatically |
| **Production** | ✅ Recommended for now | 🔜 Future migration path |

### Why Keep `merkle_tree` Field
The `merkle_tree` field in `EventConfig` is **reserved for future use**:
- When we implement direct Bubblegum `mint_v2` calls (bypassing Helius)
- When Helius adds custom tree support
- For tree address reference/tracking in the admin UI

## Next Steps
- [ ] Complete full E2E with real Google Sheet data
- [ ] Present to CTO
- [ ] Plan mainnet deployment (SOL funding, Helius plan tier)
