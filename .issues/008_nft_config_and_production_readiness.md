# Issue 008: NFT Config Setup & Production Readiness

## Status: In Progress

## Production cNFT Strategy

> **Decision:** The production system can mint cNFTs on **devnet** while the main check-in/refund system runs on mainnet. This means:
> - Attendees get real Solana devnet cNFT badges (free, no real SOL cost)
> - The platform itself is production-grade (real auth, real Sheets, real QR scanning)
> - Mainnet cNFT minting is deferred to a future phase
> - `HELIUS_API_KEY` should point to devnet RPC (`https://devnet.helius-rpc.com`) for now
> - NFT image/metadata URLs must be publicly accessible (not localhost)

## Overview

Before the BeThere event platform can go to production, the NFT config must be set up and the remaining security items from the audit (handover 025) must be addressed.

## NFT Config Checklist

### 1. Design & Upload NFT Badge Image

- [ ] Design attendance badge image (PNG, recommended 400x400 or 1000x1000)
- [ ] Upload to Arweave (permanent) or IPFS/CDN
- [ ] Record the URL as `nft_image_url`

**Options for Arweave upload:**
- [ardrive.io](https://ardrive.io) — web UI, pay with AR tokens
- `npx arweave-deploy` — CLI upload
- IRYS (formerly Bundlr) — `npx @irys/sdk upload <file>`

### 2. Create Metadata JSON

Create a Metaplex-compliant metadata JSON:

```json
{
  "name": "BeThere - {Event Name}",
  "symbol": "BETHERE",
  "description": "Proof of attendance at {Event Name}",
  "image": "<nft_image_url from step 1>",
  "external_url": "https://bethere.solana-thailand.workers.dev",
  "attributes": [
    { "trait_type": "Event", "value": "{Event Name}" },
    { "trait_type": "Type", "value": "Attendance Badge" },
    { "trait_type": "Date", "value": "{Event Date}" }
  ],
  "properties": {
    "category": "image",
    "files": [{ "uri": "<nft_image_url>", "type": "image/png" }]
  }
}
```

- [ ] Upload metadata JSON to Arweave/IPFS
- [ ] Record the URL as `nft_metadata_uri`

### 3. Collection Mint (Optional)

Helius `mintCompressedNft` can mint without a collection mint. If you want NFTs grouped under a collection:

```bash
# Create collection mint on devnet
spl-token create-token --url devnet
# Use the resulting mint address as nft_collection_mint
```

- [ ] Decide: use Helius managed (no collection) or own collection mint
- [ ] If own collection: create and record as `nft_collection_mint`

### 4. Configure Worker Secrets

```bash
cd worker

# Required for NFT minting
npx wrangler secret put HELIUS_API_KEY

# Already configured (verify)
npx wrangler secret list
```

### 5. Configure Event via Admin UI

1. Login as SuperAdmin at `/admin`
2. Go to **Events** → Edit event
3. Fill in:
   - **NFT Collection Mint** — from step 3 (or leave empty)
   - **NFT Metadata URI** — from step 2
   - **NFT Image URL** — from step 1
   - **NFT Name Template** — e.g. `BeThere - {event_name}`
   - **NFT Symbol** — e.g. `BETHERE`
4. Save

### 6. Verify on Devnet

```bash
# Start dev server
cd worker && ./deploy.sh dev

# Run E2E test with mint
./scripts/e2e/test_devnet.sh --mint-only

# Or test manually via API
curl -s "http://localhost:8787/api/claim/{token}" | jq .
```

The response should include `"nft_available": true` when all config is set.

## Production Deployment Checklist

### Security (from handover 025 audit)

- [x] Adventure attendee validation
- [x] `required_levels` logic fix
- [x] Cookie `Secure` flag
- [x] Secret redaction in Debug output
- [ ] KV-based claim dedup lock (prevent double-claim race condition)
- [ ] JWT revocation mechanism (optional, phase 2)

### Infrastructure

- [ ] `HELIUS_API_KEY` set as production wrangler secret
- [ ] All other secrets verified in production (`JWT_SECRET`, Google OAuth, etc.)
- [ ] KV namespace IDs match production (not dev)
- [ ] Frontend built and deployed (`trunk build` + `wrangler deploy`)

### Functional Testing

- [ ] Full browser E2E: login → enable quiz+adventure → check-in → quiz → adventure → claim → mint
- [ ] Verify cNFT appears on Solana Explorer (mainnet)
- [ ] Test with mobile device (D-pad, wallet connect)
- [ ] Test error states: invalid token, already claimed, wrong wallet

### Pre-Launch

- [ ] Merge `develop` → `main` (done — a0d7cba)
- [ ] Deploy to production (`cd worker && ./deploy.sh`)
- [ ] Verify production health (`GET /api/health`)
- [ ] Smoke test production claim flow
- [ ] Monitor wrangler logs for errors

## Environment Variables Reference

### Worker Secrets (`wrangler secret put`)

| Secret | Required | Purpose |
|--------|----------|---------|
| `JWT_SECRET` | ✅ | HMAC-SHA256 signing key |
| `GOOGLE_CLIENT_ID` | ✅ | OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | ✅ | OAuth client secret |
| `GOOGLE_REDIRECT_URI` | ✅ | OAuth callback URL |
| `GOOGLE_SERVICE_ACCOUNT_EMAIL` | ✅ | Sheets API access |
| `GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY` | ✅ | Sheets API auth |
| `GOOGLE_SERVICE_ACCOUNT_TOKEN_URI` | ✅ | Token endpoint |
| `HELIUS_API_KEY` | ✅ for NFT | Helius RPC API key |
| `STAFF_EMAILS` | ✅ | Staff email allowlist |
| `SUPER_ADMIN_EMAILS` | ✅ | Admin email allowlist |

### Worker Vars (`wrangler.toml [vars]`)

| Var | Default | Purpose |
|-----|---------|---------|
| `SERVER_URL` | `https://event-checkin.workers.dev` | Public URL |
| `HELIUS_RPC_URL` | `https://mainnet.helius-rpc.com` | Solana RPC |
| `GOOGLE_SHEET_NAME` | `checkin` | Attendee sheet tab |
| `GOOGLE_STAFF_SHEET_NAME` | `staff` | Staff sheet tab |

### Per-Event Config (Admin UI / KV)

| Field | Where | Purpose |
|-------|-------|---------|
| `nft_metadata_uri` | Admin → Edit Event | Metaplex metadata JSON URL |
| `nft_image_url` | Admin → Edit Event | Badge image URL |
| `nft_collection_mint` | Admin → Edit Event | Collection address (optional) |
| `nft_name_template` | Admin → Edit Event | Name with `{event_name}` placeholder |
| `nft_symbol` | Admin → Edit Event | Token symbol |
| `quiz_enabled` | Admin → Edit Event | Toggle quiz gate |
| `adventure_enabled` | Admin → Adventure Config | Toggle adventure gate |
| `required_levels` | Admin → Adventure Config | Levels to complete for gate |

## Related

- Handover 025 — Security audit + E2E test
- Issue 007 — Devnet E2E test
- Issue 006 — Rust Adventures design
- Handover 014 — Solana integration plan
