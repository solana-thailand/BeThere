# Handover 016: Phase 2 — Integration Testing with Helius Devnet

> **Date**: 2025-04-24
> **Branch**: `feature/014_solana_integration` (branched from `develop`)
> **Status**: Phase 2 integration tested end-to-end, cNFT minting works on devnet
> **Depends on**: Handover 014, Handover 015

## What Happened

Continued from Handover 015 where Phase 2 code was written but untested. This session:

1. **Tested Helius `mintCompressedNft` API** — confirmed it works without a Merkle tree, collection, or wallet funding
2. **Made collection/metadata optional** — simplified setup from 5 required secrets to just 1 (`HELIUS_API_KEY`)
3. **Discovered column mapping bug** — Google Sheet columns P-S are our custom fields, not L-M as assumed
4. **Fixed column mapping** — updated code to use correct indices (P=15, Q=16, R=17, S=18)
5. **Full integration test passed** — check-in → claim lookup → cNFT mint → duplicate block, all working

### Key Discovery: Helius Manages the Tree

The Helius deprecated `mintCompressedNft` API handles everything server-side:
- No Merkle tree creation needed (~$50 saved)
- No collection NFT needed (~$3 saved)
- No wallet funding needed for minting
- No tree authority keypair to manage
- Helius uses its own tree (`4rnLzDBgK3BqDaFGJVzvpYDKGH8mnByMxxrf4HzjhYLq`)

The only required secret is `HELIUS_API_KEY`. Everything else is optional.

### Key Discovery: Sheet Column Mapping Was Wrong

Debug logging revealed the actual Google Sheet layout differs from what the code assumed:

```
Expected (WRONG):                     Actual (CORRECT):
  G[6]  = solana_address               G[6]  = registration_date
  K[10] = qr_code_url                  K[10] = luma_url
  L[11] = claim_token                  L[11] = payment_amount ($0.00)
  M[12] = claimed_at                   M[12] = payment_fee ($0.00)
```

The Google Form populates columns A through O with form response data. Our custom columns must go in the gap at P-S.

## Current Codebase State

### Git Status

```
M domain/src/config/types.rs          (+10 Helius/NFT config fields)
M domain/src/models/api.rs            (+20 ClaimLookupResponse, ClaimResponse)
M domain/src/models/attendee.rs       (+15 claim_token, claimed_at fields, column mapping fix)
M domain/src/qr/generator.rs          (+2 test fixture)
M frontend-leptos/src/api.rs          (+110 claim API functions + types)
M frontend-leptos/src/lib.rs          (+2 claim route)
M frontend-leptos/src/pages/mod.rs    (+1 pub mod claim)
M worker/src/auth.rs                  (+5 test fixture)
M worker/src/handlers/mod.rs          (+12 claim routes, public/protected split)
M worker/src/lib.rs                   (+1 mod solana)
M worker/src/sheets.rs                (+59 get_attendee_by_claim_token, mark_claimed, column fix)
M worker/src/state.rs                 (+13 Helius/NFT secrets, made optional)
M worker/wrangler.toml                (+7 Phase 2 secret comments)
?? frontend-leptos/src/pages/claim.rs (NEW — claim page component)
?? worker/src/handlers/claim.rs       (NEW — claim API endpoints)
?? worker/src/solana.rs               (NEW — Helius JSON-RPC client)
?? .handovers/016_phase2_integration_testing.md
```

### Changes This Session (on top of Handover 015)

| File | Change |
|------|--------|
| `domain/src/models/attendee.rs` | Fixed column mapping: solana_address→P[15], qr_code_url→Q[16], claim_token→R[17], claimed_at→S[18] |
| `worker/src/sheets.rs` | Updated column refs: claim_token→R, wallet→P, claimed_at→S, qr_url→Q |
| `worker/src/state.rs` | Made NFT_COLLECTION_MINT, NFT_METADATA_URI, NFT_IMAGE_URL optional (empty string default) |
| `worker/src/solana.rs` | Build mint params conditionally — only include collection/uri/imageUrl when non-empty |
| `worker/wrangler.toml` | Updated Phase 2 secret comments to reflect optional status |
| `worker/.dev.vars` | Added HELIUS_API_KEY (local dev secrets, gitignored) |

### Google Sheet Column Layout (Final)

```
Column  Index  Field                Source
──────  ─────  ───────────────────  ──────────────────
A       0      api_id               Google Form
B       1      name                 Google Form
C       2      first_name           Google Form
D       3      last_name            Google Form
E       4      email                Google Form
F       5      ticket_name          Google Form
G       6      registration_date    Google Form
H       7      approval_status      Google Form
I       8      checked_in_at        BeThere Worker
J       9      checked_in_by        BeThere Worker
K       10     luma_url             Google Form
L       11     payment_amount       Google Form
M       12     payment_fee          Google Form
N       13     payment_total        Google Form
O       14     currency             Google Form
P       15     solana_address       BeThere Worker (claim)
Q       16     qr_code_url          BeThere Worker (generate-qrs)
R       17     claim_token          BeThere Worker (check-in)
S       18     claimed_at           BeThere Worker (claim)
...     ...    ...                  ...
Y       24     participation_type   Google Form
```

## Test Results

### Unit Tests

| Suite | Count | Status |
|-------|-------|--------|
| Domain (unit) | 14 | ✅ Pass |
| Worker (unit) | 20 | ✅ Pass |
| Clippy | — | ✅ Clean |
| WASM check (worker) | — | ✅ Clean |
| WASM check (frontend) | — | ✅ Clean |

### Integration Tests (wrangler dev → localhost:8787)

| Test | Result |
|------|--------|
| `GET /api/health` | ✅ `{"status":"ok"}` |
| `POST /api/checkin/gst-rMnFRSlpGXTdQmw` | ✅ Returns claim URL with UUID v7 |
| `GET /api/claim/{token}` | ✅ Returns name, `claimed: false`, `claimed_at: null` |
| `POST /api/claim/{token}` with wallet | ✅ Mints cNFT, returns asset_id + signature |
| `GET /api/claim/{token}` (after claim) | ✅ Returns `claimed: true`, `claimed_at: "2026-04-24T09:24:17.651+00:00"` |
| `POST /api/claim/{token}` (duplicate) | ✅ Blocked: "NFT has already been claimed" |
| DAS `getAsset` verification | ✅ Compressed V1_NFT on devnet |

### cNFTs Minted During Testing

| # | Attendee | Asset ID | Signature |
|---|----------|----------|-----------|
| 1 | Test (curl) | `FPkJmKtKEQBECuLJ8gpKZDJPsWBrsbEysdyHw9LDjjPE` | `s5y2WqdTsua5CUDdR8zNzZRBKxm2peUc3o63MmCoBLFMjvpWTs8v5cdZVLfVY36i1DAc5WhpcTWmM2exw7Ngsyf` |
| 2 | armariya | `5MytDi4AgsbSeD6aFUezRFZXsXAfexbKPQJkgs8sjW4E` | `4z1eueQnDcANeTHwNoVAeR5itSejVEmqpuAtaVKifev3KTZfdqx59oWMVpNEg6pbehEa35mD4YMtcrorKxxKzTPv` |

Both are on Helius tree `4rnLzDBgK3BqDaFGJVzvpYDKGH8mnByMxxrf4HzjhYLq`.

## Architecture Decisions

### 1. Helius `mintCompressedNft` Without Collection

The Helius API works perfectly without a collection. Benefits:
- **Zero setup cost** — no tree creation (~$50 saved), no collection mint (~$3 saved)
- **Simpler code** — no tree authority keypair, no collection management
- **Same output** — still creates valid compressed V1_NFTs on-chain

Trade-off: NFTs won't be grouped under a verified collection on wallets/marketplaces. For attendance badges (POAP-style), this is acceptable.

### 2. Optional NFT Config Secrets

Made `NFT_COLLECTION_MINT`, `NFT_METADATA_URI`, `NFT_IMAGE_URL` optional with empty-string defaults:
- `HELIUS_API_KEY` is the **only required** new secret
- Without metadata URI, Helius uses the name/symbol/description from the RPC params
- Without image URL, wallets show a default placeholder

### 3. Column Mapping at P-S Instead of G/K/L/M

The Google Form generates columns A through O. Our custom fields must go in unused columns:
- **P (15)** = solana_address — filled when attendee claims NFT
- **Q (16)** = qr_code_url — filled by `POST /api/generate-qrs`
- **R (17)** = claim_token — filled at check-in (UUID v7)
- **S (18)** = claimed_at — filled when NFT is claimed (ISO 8601)

## Secrets Configuration

### Required (1 new)

| Secret | Purpose | How to Set |
|--------|---------|-----------|
| `HELIUS_API_KEY` | Helius RPC authentication | `npx wrangler secret put HELIUS_API_KEY` |

### Optional (4 new)

| Secret | Purpose | Default |
|--------|---------|---------|
| `HELIUS_RPC_URL` | RPC endpoint | `https://devnet.helius-rpc.com` |
| `NFT_COLLECTION_MINT` | Collection address | empty (no collection) |
| `NFT_METADATA_URI` | Metadata JSON on Arweave/IPFS | empty (uses inline params) |
| `NFT_IMAGE_URL` | NFT badge image | empty (no image) |

### Local Development

For `wrangler dev`, add secrets to `worker/.dev.vars`:

```
HELIUS_API_KEY=your-api-key-here
```

## Google Sheet Setup Required

You need to add column headers in row 1 for columns P, Q, R, S:

| Column | Header | Example Value |
|--------|--------|--------------|
| P1 | `solana_address` | (empty until claimed) |
| Q1 | `qr_code_url` | `https://bethere.solana-thailand.workers.dev/staff/?scan=gst-...` |
| R1 | `claim_token` | `019dbecd-2549-7190-9036-f0c62db7ef0f` |
| S1 | `claimed_at` | `2026-04-24T09:24:17.651+00:00` |

Then regenerate QR URLs: `POST /api/generate-qrs?force=true`

## Action Items & Next Steps

### Before Next Real Event

- [ ] **Add column headers P, Q, R, S** to the Google Sheet (row 1)
- [ ] **Set `HELIUS_API_KEY`** production secret: `npx wrangler secret put HELIUS_API_KEY`
- [ ] **Regenerate QR URLs**: `POST /api/generate-qrs?force=true` (writes to column Q)
- [ ] **Decide**: mainnet or stay on devnet for first event?
- [ ] **Design NFT artwork** (optional — works without it)
- [ ] **Upload metadata JSON** to Arweave (optional — works without it)
- [ ] **Set `HELIUS_RPC_URL`** to mainnet if going production

### Phase 3 (Refund)

- [ ] SOL airdrop for gas (0.01 SOL)
- [ ] USDC refund transfer (~$13)
- [ ] Fund treasury wallet with SOL + USDC
- [ ] Add refund logic to claim handler

### Future Improvements

- [ ] Migrate from deprecated `mintCompressedNft` to Bubblegum V1 SDK (build instructions locally)
- [ ] Create verified NFT collection for better wallet display (~$3 one-time)
- [ ] Evaluate Bubblegum V2 for soulbound NFTs (non-transferable)
- [ ] Add DAS API integration for reading/querying cNFTs
- [ ] Design proper NFT artwork and upload to Arweave

## Issues Ref

- N/A (no issues created)

## How to Dev/Test

```bash
# Branch setup
git checkout feature/014_solana_integration

# Run existing tests
cargo test

# Check WASM builds
cargo check --target wasm32-unknown-unknown -p event-checkin-worker
cd frontend-leptos && cargo check --target wasm32-unknown-unknown

# Local dev server (requires HELIUS_API_KEY in worker/.dev.vars)
cd worker && ./deploy.sh dev

# Test claim flow manually:
# 1. Generate JWT:
python3 -c "
import hmac, hashlib, base64, json, time
secret = b'YOUR_JWT_SECRET'
header = base64.urlsafe_b64encode(json.dumps({'alg':'HS256','typ':'JWT'}, separators=(',',':')).encode()).rstrip(b'=').decode()
now = int(time.time())
payload = base64.urlsafe_b64encode(json.dumps({'email':'admin@example.com','sub':'admin','is_staff':True,'iat':now,'exp':now+3600}, separators=(',',':')).encode()).rstrip(b'=').decode()
sig = base64.urlsafe_b64encode(hmac.new(secret, f'{header}.{payload}'.encode(), hashlib.sha256).digest()).rstrip(b'=').decode()
print(f'{header}.{payload}.{sig}')
"

# 2. Check in an attendee:
curl -X POST -H "Authorization: Bearer $JWT" http://localhost:8787/api/checkin/{api_id}

# 3. Look up claim:
curl http://localhost:8787/api/claim/{token}

# 4. Mint NFT:
curl -X POST http://localhost:8787/api/claim/{token} \
  -H "Content-Type: application/json" \
  -d '{"wallet_address": "SOLANA_WALLET_ADDRESS"}'

# 5. Verify on-chain:
curl -s -X POST "https://devnet.helius-rpc.com/?api-key=$HELIUS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"v","method":"getAsset","params":{"id":"ASSET_ID"}}'
```

## Reflection

### Struggling / Solved

- **Column mapping mismatch**: The code assumed columns G/K/L/M for custom fields, but the Google Form occupies those columns. Debug logging (`log::info!` not `tracing::info!` — wrangler only shows `console.log`) revealed the actual layout. Solution: use columns P-S which are empty after the form's A-O range.

- **`tracing` vs `log` in Workers**: `tracing::info!` macros don't appear in `wrangler dev` output because the worker initializes `console_log` (which maps `log` crate to `console.log`), not `tracing`. Had to use `log::info!` for debug output.

- **JWT generation for testing**: The Rust code uses `secret.as_bytes()` directly as HMAC key (not base64-decoded). The JWT payload requires `email`, `sub`, `iat`, `exp` fields. Python generation: `hmac.new(secret_bytes, msg_bytes, hashlib.sha256)`.

- **Helius API simplicity**: Expected to need Merkle tree setup, collection creation, wallet funding. The deprecated `mintCompressedNft` handles everything — just send name/symbol/owner and it mints. This dramatically simplifies the architecture.

- **WASM format spec**: Rust uses `{:02}` for zero-padded integers, not `{:02d}` (Python/C syntax). The `d` format trait doesn't exist in Rust — only `Display`, `Debug`, `Binary`, `Octal`, `LowerHex`, `UpperHex`, `LowerExp`, `UpperExp`, `Pointer`.