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

### Option A: Self-Hosted (Recommended — No External Upload Needed)

The worker serves its own badge and metadata. No Arweave/IPFS required.

**Badge image** (production 1000x1000 SVG):
```
https://bethere.solana-thailand.workers.dev/api/badge-hd.svg
```

**Metadata JSON** (dynamic per-event):
```
https://bethere.solana-thailand.workers.dev/api/metadata/{event_id}
```

The metadata endpoint loads per-event fields from KV (name, symbol, description, image)
and falls back to global config. It automatically includes Event name and Date traits.

**Admin UI configuration:**
1. Login as SuperAdmin at `/admin`
2. Go to **Events** → Edit event
3. Fill in:
   - **NFT Image URL** — `https://bethere.solana-thailand.workers.dev/api/badge-hd.svg`
   - **NFT Metadata URI** — `https://bethere.solana-thailand.workers.dev/api/metadata/{event_id}`
   - **NFT Name Template** — `BeThere - {event_name}`
   - **NFT Symbol** — `BETHERE`
   - **NFT Description Template** — `Proof of attendance at {event_name}`
   - **NFT Collection Mint** — leave empty (Helius managed)

- [x] Production badge SVG (1000x1000) — `worker/src/badge_production.svg`
- [x] Dynamic metadata endpoint — `worker/src/handlers/metadata.rs`
- [x] HD badge route — `GET /api/badge-hd.svg`
- [ ] Configure `HELIUS_API_KEY` worker secret
- [ ] Set NFT fields in admin UI for first event

### Option B: Custom Arweave/IPFS Upload

For permanent, immutable storage (survives even if the platform goes down):

- [ ] Design attendance badge image (PNG, recommended 1000x1000)
- [ ] Upload to Arweave (permanent) or IPFS/CDN
- [ ] Record the URL as `nft_image_url`

**Options for Arweave upload:**
- [ardrive.io](https://ardrive.io) — web UI, pay with AR tokens
- `npx arweave-deploy` — CLI upload
- IRYS (formerly Bundlr) — `npx @irys/sdk upload <file>`

Create a Metaplex-compliant metadata JSON, upload to Arweave/IPFS, record as `nft_metadata_uri`.

### Collection Mint (Optional)

Helius `mintCompressedNft` can mint without a collection mint. If you want NFTs grouped under a collection:

```bash
# Create collection mint on devnet
spl-token create-token --url devnet
# Use the resulting mint address as nft_collection_mint
```

- [ ] Decide: use Helius managed (no collection) or own collection mint
- [ ] If own collection: create and record as `nft_collection_mint`

### Configure Worker Secrets

```bash
cd worker

# Required for NFT minting
npx wrangler secret put HELIUS_API_KEY

# Already configured (verify)
npx wrangler secret list
```

### Verify on Devnet

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
- [x] KV-based claim dedup lock (prevent double-claim race condition) — implemented in `worker/src/handlers/claim.rs`
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
| `GOOGLE_SHEET_NAME` | `Attendees` | Attendee sheet tab |
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

## Future Consideration: `solana-keychain`

[`solana-keychain`](https://github.com/solana-foundation/solana-keychain) is a unified Solana transaction signing library (Rust + TypeScript) with backends for AWS KMS, GCP KMS, Vault, Fireblocks, Turnkey, Privy, etc. **Audited by Accretion.**

**Not needed today** — BeThere delegates all signing to Helius (`mintCompressedNft` JSON-RPC). BeThere never signs transactions itself.

**When to consider adding it:**

| Scenario | Needed? |
|----------|----------|
| Keep using Helius `mintCompressedNft` | ❌ No — Helius signs for you |
| Platform-managed refund wallet (sign SOL/USDC transfers) | ✅ Yes — need to sign transfers |
| Self-hosted Bubblegum `mint_v2` (cut Helius dependency) | ✅ Yes — you'd sign `mint_v2` yourself |
| Organizer deposits held in platform vault | ✅ Yes — AWS KMS or Vault backend for treasury key |
| Enterprise organizer key management | ✅ Yes — multiple KMS backends |

Rust crate: `solana-keychain` (v1.0.1, feature-gated, async, `wasm32` compatible).

## Related

- Handover 025 — Security audit + E2E test
- Handover 026 — cNFT mint fix + E2E cost analysis
- Issue 007 — Devnet E2E test
- Issue 006 — Rust Adventures design
- Handover 014 — Solana integration plan
