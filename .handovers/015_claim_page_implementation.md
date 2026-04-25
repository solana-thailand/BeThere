# Handover 015: Phase 2 — Claim Page + NFT Minting (Backend + Frontend)

> **Date**: 2025-07-01
> **Branch**: `feature/014_solana_integration` (branched from `develop`)
> **Status**: Phase 2 backend + frontend complete, ready for integration testing
> **Depends on**: Handover 014, DISCUSSION.md

## What Happened

Continued from Handover 014 where Phase 1 (claim token generation at check-in) was completed and deployed. This session implemented **Phase 2: Claim Page + NFT Minting**, covering both backend API endpoints and the frontend claim page component.

### Session Summary

1. **Verified existing tests** — 34 tests pass (14 domain + 20 worker), clippy clean
2. **Reviewed Phase 2 backend code** (from previous session) — claim handler, Solana RPC client, model updates
3. **Implemented frontend claim page** — full Leptos WASM component with state machine flow
4. **Added API client functions** — `get_claim()` and `post_claim()` for claim endpoints
5. **Registered claim route** — `/claim/:token` in the Leptos router
6. **Verified all builds** — native tests, WASM check (worker + frontend), clippy

## Current Codebase State

### Git Status

```
M domain/src/config/types.rs           (+10 Helius/NFT config fields)
M domain/src/models/api.rs             (+20 ClaimLookupResponse, ClaimResponse)
M domain/src/models/attendee.rs        (+17 claim_token, claimed_at fields)
M domain/src/qr/generator.rs           (+2 test fixture)
M frontend-leptos/src/api.rs           (+110 claim API functions + types)
M frontend-leptos/src/lib.rs           (+2 claim route)
M frontend-leptos/src/pages/mod.rs     (+1 pub mod claim)
M worker/src/auth.rs                   (+5 test fixture)
M worker/src/handlers/mod.rs           (+12 claim routes, public/protected split)
M worker/src/lib.rs                    (+1 mod solana)
M worker/src/sheets.rs                 (+51 get_attendee_by_claim_token, mark_claimed)
M worker/src/state.rs                  (+13 Helius/NFT secrets)
M worker/wrangler.toml                 (+7 Phase 2 secret comments)
?? frontend-leptos/src/pages/claim.rs  (NEW — claim page component)
?? worker/src/handlers/claim.rs        (NEW — claim API endpoints)
?? worker/src/solana.rs                (NEW — Helius JSON-RPC client)
```

### Files Created (3)

| File | Purpose |
|------|---------|
| `worker/src/solana.rs` | Helius JSON-RPC client: `mint_compressed_nft()`, `validate_wallet_address()`, 7 unit tests |
| `worker/src/handlers/claim.rs` | `GET /api/claim/{token}` (lookup) + `POST /api/claim/{token}` (mint + mark claimed) |
| `frontend-leptos/src/pages/claim.rs` | Claim page component with 6-state flow: Loading → NotFound/Ready → Minting → Success/Error/AlreadyClaimed |

### Files Modified (13)

| File | Changes |
|------|---------|
| `domain/src/config/types.rs` | Added 5 fields: `helius_rpc_url`, `helius_api_key`, `nft_collection_mint`, `nft_metadata_uri`, `nft_image_url` |
| `domain/src/models/api.rs` | Added `ClaimLookupResponse` and `ClaimResponse` types |
| `domain/src/models/attendee.rs` | Added `claim_token` and `claimed_at` fields to `Attendee` + `AttendeeRow`; updated `from_sheet_values()` to parse columns L (11) and M (12) |
| `domain/src/qr/generator.rs` | Updated test fixture with new `claim_token` and `claimed_at` fields |
| `frontend-leptos/src/api.rs` | Added `ClaimLookupData`, `ClaimMintData` types; added `get_claim()` (GET) and `post_claim()` (POST) public API functions |
| `frontend-leptos/src/lib.rs` | Added `/claim/:token` route with `Claim` component |
| `frontend-leptos/src/pages/mod.rs` | Added `pub mod claim;` |
| `worker/src/auth.rs` | Updated test fixture with 5 Helius/NFT config fields |
| `worker/src/handlers/mod.rs` | Restructured routes into public (health, auth, claim) and protected (attendees, checkin, qr) sections |
| `worker/src/lib.rs` | Added `mod solana;` |
| `worker/src/sheets.rs` | Added `get_attendee_by_claim_token()` and `mark_claimed()` functions |
| `worker/src/state.rs` | Added env reads for 5 Helius/NFT secrets (`HELIUS_RPC_URL` has default) |
| `worker/wrangler.toml` | Added Phase 2 secret comments |

## Test Results

| Suite | Count | Status |
|-------|-------|--------|
| Domain (unit) | 14 | ✅ Pass |
| Worker (unit) | 20 | ✅ Pass |
| Frontend WASM check | — | ✅ Clean |
| Worker WASM check | — | ✅ Clean |
| Clippy | — | ✅ Clean |
| **Total** | **34** | ✅ Pass |

### New Tests (7 — all in `worker/src/solana.rs`)

| Test | What |
|------|------|
| `test_validate_wallet_address_valid` | Standard 32-char address passes |
| `test_validate_wallet_address_too_short` | 3-char address rejected |
| `test_validate_wallet_address_too_long` | 50-char address rejected |
| `test_validate_wallet_address_invalid_chars` | Address with `0` (not in base58) rejected |
| `test_validate_wallet_address_min_length` | 32-char valid base58 passes |
| `test_validate_wallet_address_max_length` | 44-char valid base58 passes |
| `test_validate_wallet_address_real_solana_address` | System Program address passes |

## Architecture Decisions

### Backend: Public vs Protected Routes

Claim endpoints are **public** (no auth middleware). This is intentional — attendees claim NFTs on their own phones without needing staff credentials. The router was split into:

```
/api
├── Public (no auth)
│   ├── /health
│   ├── /auth/* (login flow)
│   └── /claim/{token} (GET + POST)
└── Protected (require_auth middleware)
    ├── /attendees
    ├── /attendee/{id}
    ├── /checkin/{id}
    └── /generate-qrs
```

### Frontend: State Machine Pattern

The claim page uses a 6-state enum to track the flow:

```
Loading ──→ NotFound (invalid token)
        ──→ AlreadyClaimed (was claimed before)
        ──→ Ready ──→ Minting ──→ Success
                              └─→ MintError ──→ Ready (retry)
```

This avoids the common "multiple boolean flags" anti-pattern and makes the UI deterministic.

### Helius Integration

Uses the deprecated but functional `mintCompressedNft` JSON-RPC method. Auth is via query-param (`?api-key=KEY`), not headers. The worker sends metadata and Helius handles transaction building/signing server-side. Zero new crate dependencies — uses existing `worker::Fetch`.

### Route Param Extraction

Leptos 0.8 requires importing `leptos_router::params::Params` trait for the `#[derive(Params)]` macro. The route uses `:token` syntax in the path (`/claim/:token`) and `use_params::<ClaimParams>()` to extract it.

## Key Implementation Details

### Claim Page Component (`frontend-leptos/src/pages/claim.rs`)

- **371 lines** — self-contained component with all states
- Uses `use_params::<ClaimParams>()` to extract token from URL
- Client-side wallet validation (length 32-44, base58 charset) before sending to server
- Displays Solscan explorer links on success (devnet cluster)
- "Try Again" button on mint error returns to Ready state
- Reuses existing CSS classes: `center-page`, `result-success`, `result-error`, `result-warning`, `card`, `btn`

### Claim API (`frontend-leptos/src/api.rs`)

Two new public API functions (no auth headers):

- `get_claim(token: &str) -> Result<ClaimLookupData, ApiError>` — GET /api/claim/{token}
- `post_claim(token: &str, wallet_address: &str) -> Result<ClaimMintData, ApiError>` — POST /api/claim/{token} with JSON body

### Claim Handler (`worker/src/handlers/claim.rs`)

- `get_claim()` — looks up attendee by claim token, returns name + claim status
- `post_claim()` — validates wallet → checks not already claimed → mints cNFT via Helius → marks claimed in sheet
- Writes wallet to column G and claimed_at to column M

### Solana Module (`worker/src/solana.rs`)

- `mint_compressed_nft()` — Helius JSON-RPC 2.0 call with `mintCompressedNft` method
- `validate_wallet_address()` — base58 charset + length check (32-44 chars)
- Returns `MintResult { signature, asset_id }` on success

## Outcomes

### Completed

- ✅ Phase 2 backend API fully implemented and tested
- ✅ Phase 2 frontend claim page fully implemented
- ✅ Claim lookup endpoint (GET) validates token, returns attendee info
- ✅ Claim mint endpoint (POST) validates wallet, mints cNFT via Helius, writes to sheet
- ✅ Solana RPC client uses existing `worker::Fetch` — zero new crate dependencies
- ✅ Routes restructured: public claim endpoints, protected staff endpoints
- ✅ Frontend WASM build compiles cleanly
- ✅ All 34 tests pass, clippy clean

### Not Yet Done (Remaining Work)

- ❌ Integration testing against Helius devnet (need API key + collection)
- ❌ WASM runtime test (`wrangler dev` full stack)
- ❌ CSS polish for claim page (optional — works with existing styles)
- ❌ Handover document ← this document

## Prerequisites for Integration Testing

Before the claim flow can be tested end-to-end:

1. **Helius account** — sign up at dev.helius.xyz, get API key
2. **Devnet SOL** — fund a treasury wallet for gas
3. **NFT collection** — create a Merkle tree + collection on devnet
4. **Artwork** — design NFT badge image, upload to Arweave/IPFS
5. **Metadata JSON** — create on-chain metadata pointing to image
6. **Configure Worker secrets**:
   ```bash
   npx wrangler secret put HELIUS_API_KEY
   npx wrangler secret put NFT_COLLECTION_MINT
   npx wrangler secret put NFT_METADATA_URI
   npx wrangler secret put NFT_IMAGE_URL
   # HELIUS_RPC_URL defaults to https://devnet.helius-rpc.com
   ```

## New Secrets Required

| Secret | Purpose | Required? |
|--------|---------|-----------|
| `HELIUS_RPC_URL` | RPC endpoint | No (defaults to devnet) |
| `HELIUS_API_KEY` | RPC authentication | **Yes** |
| `NFT_COLLECTION_MINT` | cNFT collection address | **Yes** |
| `NFT_METADATA_URI` | Metadata JSON URI | **Yes** |
| `NFT_IMAGE_URL` | NFT badge image URL | **Yes** |

## Action Items & Next Steps

### Immediate (Next Session)

- [ ] Set up Helius devnet account and API key
- [ ] Create Merkle tree + NFT collection on devnet
- [ ] Design placeholder NFT artwork (can be simple gradient with event name)
- [ ] Upload metadata JSON to Arweave (use arweave.net deploy or Irys)
- [ ] Configure Worker secrets on dev environment
- [ ] Run `wrangler dev` and test full claim flow
- [ ] Test with real devnet wallet (Phantom/Solflare)

### Phase 3 (After Phase 2 Integration Testing)

- [ ] SOL airdrop for gas (0.01 SOL)
- [ ] USDC refund transfer (~$13)
- [ ] Fund treasury wallet with SOL + USDC (devnet for testing)
- [ ] Add refund logic to claim handler

### Before Real Event

- [ ] Switch to mainnet Helius RPC URL
- [ ] Create mainnet NFT collection
- [ ] Fund mainnet treasury wallet
- [ ] Regenerate QR URLs on production
- [ ] Test full flow on mainnet

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

# Run clippy
cargo clippy --all-targets

# Local dev server (requires Worker secrets)
cd worker && npx wrangler dev

# Test claim page manually
# 1. Check in an attendee via staff scanner
# 2. Copy the claim URL from the check-in response
# 3. Open claim URL in browser (or phone)
# 4. Enter a Solana wallet address
# 5. Click "Claim NFT Badge"
# 6. Verify NFT appears on Solscan

# Deploy to dev
cd worker && ./deploy.sh dev
```

## Reflection

### Struggling / Solved

- **Leptos 0.8 Params derive**: The `#[derive(Params)]` macro requires importing `leptos_router::params::Params` trait explicitly. The compiler error message was clear: `error[E0404]: expected trait, found derive macro 'Params'`. Adding `use leptos_router::params::Params;` fixed it.

- **Route path syntax**: Leptos 0.8 uses `path!("/claim/:token")` with colon prefix for dynamic segments. This matches the existing route pattern in the router. The `use_params::<ClaimParams>()` hook returns `Result<ClaimParams, _>` which must be handled for missing/invalid tokens.

- **Frontend not in workspace**: The `frontend-leptos` crate has its own `[workspace]` section in `Cargo.toml` and is not a member of the root workspace. Building it requires `cd frontend-leptos && cargo check` rather than `cargo check -p event-checkin-frontend`.

- **Public API endpoints**: The claim API functions (`get_claim`, `post_claim`) in the frontend don't send auth headers — they use raw `gloo::net::http::Request` instead of the `api_get`/`api_post` helpers which attach `Authorization` headers. This is correct since claim endpoints are public.

### Design Decisions

1. **State machine over boolean flags**: The claim page uses `enum ClaimState` with 6 variants instead of multiple `bool` signals. This prevents impossible states (e.g., "loading" and "success" simultaneously) and makes the view rendering a clean `match`.

2. **Inline styles for claim-specific CSS**: The claim page uses inline `style=` attributes for layout tweaks rather than adding new CSS classes to `style.css`. This keeps the global stylesheet small and the component self-contained. Shared classes (`card`, `btn`, `result-*`) are reused.

3. **Client-side wallet validation**: The claim button is disabled when the wallet input doesn't pass basic validation (length 32-44, not empty). This prevents obviously invalid requests from reaching the server. The server still does its own validation as the authoritative check.

4. **Retry on mint error**: The `MintError` state includes a clone of the `ClaimLookupData` so the user can click "Try Again" and return to the `Ready` state without re-fetching from the server. This is a better UX than a full page reload.
