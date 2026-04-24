# Handover 014: Solana Integration — NFT Badge + Hybrid Refund

> **Date**: 2025-06-30
> **Branch**: `feature/014_solana_integration` (branched from `develop`)
> **Status**: Phase 1 tested & deployed, ready for Phase 2
> **Depends on**: `DISCUSSION.md` in repo root

## What Happened

Completed a full architecture discussion with the team and CTO about evolving BeThere from a Google Sheets-only system to a Solana-integrated event platform. Key decisions were made about the role of NFTs, refund mechanism, and user onboarding flow. Created `develop` branch and `feature/014_solana_integration` branch to begin implementation.

### Discussion Outcome

- **DISCUSSION.md** created in repo root documenting AS-IS, TO-BE, and all decisions
- Architecture direction finalized: **NFT Badge (post-check-in reward), not NFT Ticket (pre-event gate)**
- Refund method decided: **Hybrid SOL airdrop + USDC**
- Implementation phased: 3 phases over 7-10 days
- Git branches created: `develop` → `feature/014_solana_integration`

## Current Codebase State

### Repository Structure
```
event-checkin/
├── Cargo.toml              # workspace root
├── DISCUSSION.md           # architecture decisions (new)
├── README.md               # updated with roadmap + columns L/M
├── domain/                 # shared types and config
│   └── src/
│       ├── config.rs       # AppConfig, SheetsConfig, etc.
│       └── models/         # Attendee, CheckinResponse, etc.
├── worker/                 # Cloudflare Workers (Rust WASM)
│   ├── Cargo.toml
│   ├── wrangler.toml
│   └── src/
│       ├── lib.rs          # entry point, router, SPA fallback
│       ├── state.rs        # AppState::from_env()
│       ├── sheets.rs       # Google Sheets API client
│       ├── auth.rs         # Google OAuth + JWT
│       ├── crypto.rs       # service account JWT signing
│       ├── http.rs         # HTTP helpers
│       ├── middleware.rs   # security headers
│       └── handlers/
│           ├── mod.rs      # routes() function
│           ├── attendee.rs # attendee lookup
│           ├── auth.rs     # login/logout/callback
│           ├── checkin.rs  # QR scan check-in
│           ├── health.rs   # health check
│           └── qr.rs       # QR code generation
└── frontend-leptos/        # Leptos WASM frontend
```

### Key Files for Phase 1 Implementation

| File | What to Change |
|------|---------------|
| `worker/src/handlers/checkin.rs` | Generate UUID claim_token, include claim URL in response |
| `worker/src/sheets.rs` | Add column L (claim_token) to batch update writes |
| `worker/src/state.rs` | Add `CLAIM_BASE_URL` var from wrangler.toml |
| `worker/Cargo.toml` | Add `uuid` dependency (Uuid::now_v7()) |
| `worker/wrangler.toml` | Add `CLAIM_BASE_URL` to `[vars]` |
| `domain/src/models/mod.rs` | Add `claim_url` field to CheckinResponse |

## Key Decisions

| Decision | Choice |
|----------|--------|
| NFT as ticket or badge? | **Badge** (post-check-in reward, like POAP) |
| Wallet required at check-in? | **No** — check-in works without Solana |
| 500 THB refund method? | **Hybrid: 0.01 SOL + ~$13 USDC** |
| NFT standard? | **Compressed NFT (cNFT) via Bubblegum** |
| Claim flow? | **URL-based, generated at check-in, accessed on attendee's phone** |
| Non-claimers? | **NFT claimable anytime, cash refund fallback** |
| Check-in verification? | **Keep Google Sheets** — Solana is additive |
| Data store? | **Google Sheets remains source of truth** |

## Implementation Plan

### Phase 1: Claim Token (0.5 day) ✅ DONE (commit `221bed9`)

**What:** Generate UUID claim token at check-in, store in Google Sheet column L, return claim URL to staff.

**Step-by-step:**

1. Add `uuid` crate to `worker/Cargo.toml` with `v7` feature
2. Add `CLAIM_BASE_URL` to `worker/wrangler.toml` `[vars]`
3. Add `claim_base_url` to `AppState` / domain config
4. In `checkin.rs` handler:
   - After successful check-in, generate `Uuid::now_v7()`
   - Build claim URL: `{CLAIM_BASE_URL}/{uuid}`
   - Include claim URL in JSON response
5. In `sheets.rs` batch update:
   - Write UUID to column L (index 11) during check-in
6. Add `claim_url` field to `CheckinResponse` in domain models

**Dependencies:** None — purely additive, no breaking changes.

**Sheet changes:**
- Column L (index 11): `claim_token` — UUID generated at check-in
- Column M (index 12): `claimed_at` — filled when attendee claims NFT/refund (Phase 2)

### Phase 2: Claim Page + NFT Minting (4-5 days)

**What:** Frontend claim page where attendee connects wallet, receives cNFT badge.

**Files to create:**
- `worker/src/solana.rs` — Solana JSON-RPC client (getTokenAccountsByOwner, getAsset, etc.)
- `worker/src/handlers/claim.rs` — claim endpoint (GET /claim/{token}, POST /api/claim/{token})
- `frontend-leptos/src/pages/claim.rs` — claim page UI (wallet connect, NFT preview, claim button)

**Files to modify:**
- `worker/src/handlers/mod.rs` — add claim routes
- `worker/src/state.rs` — add SOLANA_RPC_URL, NFT_COLLECTION_MINT secrets
- `worker/Cargo.toml` — add Solana-related dependencies
- `domain/src/models/mod.rs` — add ClaimResponse type
- `frontend-leptos/src/pages/mod.rs` — add claim route

**Dependencies:**
- Bubblegum program deployed on mainnet (or use existing)
- NFT collection created (Metaplex Candy Machine or manual)
- RPC provider (Helius free tier)

### Phase 3: Hybrid Refund (2-3 days)

**What:** Send SOL airdrop + USDC transfer to attendee wallet on claim.

**Files to create:**
- `worker/src/handlers/refund.rs` — refund logic (build + send transaction)

**Files to modify:**
- `worker/src/solana.rs` — add transaction building, signing, sending
- `worker/src/handlers/claim.rs` — trigger refund after NFT mint
- `worker/src/state.rs` — add TREASURY_PRIVATE_KEY, REFUND_SOL_AMOUNT, REFUND_USDC_AMOUNT secrets

**Dependencies:**
- Treasury wallet funded with SOL + USDC
- Exchange rate source (Pyth oracle or hardcoded for MVP)

## New Secrets Required

| Secret | Purpose | Phase |
|--------|---------|-------|
| `SOLANA_RPC_URL` | RPC endpoint (e.g. Helius) | Phase 2 |
| `NFT_COLLECTION_MINT` | cNFT collection address | Phase 2 |
| `TREASURY_PRIVATE_KEY` | Wallet for refund txs | Phase 3 |
| `REFUND_SOL_AMOUNT` | SOL to send (e.g. "0.01") | Phase 3 |
| `REFUND_USDC_AMOUNT` | USDC to send (e.g. "13.00") | Phase 3 |
| `USDC_MINT_ADDRESS` | SPL USDC mint address | Phase 3 |

## New Wrangler Vars

| Var | Default | Purpose | Phase |
|-----|---------|---------|-------|
| `CLAIM_BASE_URL` | `https://bethere.solana-thailand.workers.dev/claim` | Base URL for claim links | Phase 1 |

## Testing Strategy

| Phase | Test Type | What |
|-------|-----------|------|
| Phase 1 | Unit | UUID generation, claim URL format |
| Phase 1 | Manual | Check-in returns claim URL, sheet has column L filled |
| Phase 2 | Integration | Solana RPC calls against devnet |
| Phase 2 | Unit | Claim token validation |
| Phase 3 | Integration | SOL transfer on devnet |
| Phase 3 | Integration | USDC transfer on devnet (Token-2022 or SPL) |
| All | Manual | Full flow: check-in → claim → NFT → refund |

## Performance Targets

| Metric | Target | Current |
|--------|--------|---------|
| Check-in latency (no claim) | < 2s (unchanged) | 500ms - 2s |
| Claim page load | < 1s | N/A |
| NFT minting (cNFT) | < 5s | N/A |
| Refund transaction | < 3s | N/A |
| Total claim flow (mint + refund) | < 8s | N/A |

## Open Questions for CTO

1. **Exchange rate source** — Pyth oracle on-chain or hardcoded for MVP?
2. **Treasury wallet** — Single keypair or multi-sig (Squads)?
3. **NFT artwork** — Who designs? When?
4. **Refund amount in THB** — Fixed 500 THB or configurable per event?

## Remain Work

- [x] Phase 1: Claim token generation ✅ `221bed9`
- [x] Phase 1: WASM build fix (uuid `js` feature) ✅ `027bfa4`
- [x] Phase 1: localhost:3000 → 8787 fixes ✅ `177fd09`
- [x] Phase 1: Local integration test ✅ all pass
- [x] Phase 1: Production deploy ✅
- [ ] Phase 2a: Claim page frontend ← NEXT
- [ ] Phase 2b: Wallet connect UI
- [ ] Phase 2c: cNFT minting (Bubblegum)
- [ ] Phase 3a: SOL airdrop
- [ ] Phase 3b: USDC transfer
- [ ] Phase 3c: On-chain check-in tx (optional)

### Before Real Event

- [ ] Regenerate QR URLs on production (`POST /api/generate-qrs?force=true`) so they point to `https://bethere.solana-thailand.workers.dev/staff/?scan=...`
- [ ] Add `claim_token` header to column L row 1 in Google Sheet

## Issues Ref

- N/A (no issues created yet)

## How to Dev/Test

```bash
# Branch setup (done)
git checkout develop
git checkout feature/014_solana_integration

# Phase 1: Run existing tests after changes
cargo test

# Phase 2: Need devnet SOL and RPC
# 1. Get devnet SOL: solana airdrop 2 <TREASURY_WALLET> --url devnet
# 2. Set SOLANA_RPC_URL secret to devnet endpoint
# 3. Test RPC calls against devnet

# Phase 3: Need devnet USDC
# 1. Use devnet USDC mint: 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
# 2. Mint devnet USDC to treasury wallet
# 3. Test transfers on devnet before mainnet

# Deploy for testing
cd worker && ./deploy.sh dev
```

## Reflection

This plan preserves the existing Google Sheets workflow that organizers trust, while adding Solana as a **reward layer** — not a gate. The NFT badge approach is simpler than NFT tickets because:

1. Check-in doesn't depend on Solana (no risk of blockchain issues blocking entry)
2. Users don't need wallets at registration (lower friction)
3. NFT minting is on-demand (lower cost risk)
4. The system degrades gracefully (no wallet = no NFT, but still checked in)

The key principle: **Web3 is additive, never blocking.** Check-in works without Solana. The NFT and refund are rewards for those who want them.

### Struggling / Solved

- **Git branch naming**: `develop/feature/014_solana_integration` is invalid because `develop` exists as a branch. Solution: use flat namespace `feature/014_solana_integration` instead.
- **Column indexing**: Google Sheets API uses 0-based column indices. Column L = index 11, Column M = index 12. Must be consistent in sheets.rs batch update.
- **Test fixture update**: `auth.rs` tests create `AppConfig` manually — must add new `claim_base_url` field when config struct changes. Build succeeded but `cargo test` caught it.
- **UUID WASM compatibility**: `uuid` crate needs `"js"` feature for `wasm32-unknown-unknown` target. Native tests pass but WASM build fails — this is a recurring pattern for any crate using randomness/IO in Workers.
- **localhost port mismatch**: Old Axum server used port 3000, Workers uses 8787. Found stale references in `Trunk.toml` (proxy), `api.rs` (fallback), and 21 QR URLs in the Google Sheet. Rule: any port change requires regenerating QR URLs.
- **JWT generation for testing**: Worker uses HMAC-SHA256 JWT via SubtleCrypto. To test locally, generate JWT with Python using compact JSON (no spaces in `separators=(',',':')`) to match Rust's `serde_json` output. Cookie name is `event_checkin_token`, not `session`.

### Phase 1 Changes Summary

| File | Change |
|------|--------|
| `worker/Cargo.toml` | Added `uuid = { version = "1", features = ["v7", "serde", "js"] }` |
| `worker/wrangler.toml` | Added `CLAIM_BASE_URL` to `[vars]` |
| `worker/src/state.rs` | Read `CLAIM_BASE_URL` var, pass to `AppConfig` with fallback |
| `domain/src/config/types.rs` | Added `claim_base_url: String` to `AppConfig` |
| `domain/src/models/api.rs` | Added `claim_url: Option<String>` to `CheckInResponse` |
| `worker/src/handlers/checkin.rs` | Generate `Uuid::now_v7()`, build claim URL, pass to sheet + response |
| `worker/src/sheets.rs` | `mark_checked_in` now accepts `claim_token`, writes to column L |
| `worker/src/auth.rs` | Fixed test fixture with `claim_base_url` field |
| `frontend-leptos/Trunk.toml` | Updated proxy `localhost:3000` → `localhost:8787` |
| `frontend-leptos/src/api.rs` | Updated fallback origin `localhost:3000` → `localhost:8787` |

### Phase 1 Testing & Bug Fixes

#### Bug 1: WASM build failure (commit `027bfa4`)

`uuid` crate requires a randomness source on `wasm32-unknown-unknown`. Native tests passed (macOS) but `wrangler dev` failed:

```
error: to use `uuid` on `wasm32-unknown-unknown`, specify a source of randomness
using one of the `js`, `rng-getrandom`, or `rng-rand` features
```

**Fix:** Added `"js"` feature to `uuid` dependency in `worker/Cargo.toml`:
```toml
uuid = { version = "1", features = ["v7", "serde", "js"] }
```

This enables `crypto.getRandomValues()` via wasm-bindgen, available in Cloudflare Workers runtime.

#### Bug 2: Stale localhost:3000 references (commit `177fd09`)

Found 2 runtime files and 21 Google Sheet QR URLs still pointing to `localhost:3000` (old Axum port). The Worker runs on `localhost:8787`.

**Fix:**
- `frontend-leptos/Trunk.toml`: proxy backend updated
- `frontend-leptos/src/api.rs`: fallback origin updated
- Regenerated all 21 QR URLs via `POST /api/generate-qrs?force=true`

#### Local Integration Test Results

All tests performed on `localhost:8787` with HMAC-SHA256 JWT:

| Test | Result |
|------|--------|
| `GET /api/health` | ✅ `{"status":"ok"}` |
| `GET /api/auth/me` (valid JWT) | ✅ Returns email + is_staff |
| `GET /api/attendees` | ✅ Lists all from Google Sheet |
| `POST /api/checkin/{id}` (In-Person, approved) | ✅ Returns `claim_url` with UUID v7 |
| `POST /api/checkin/{id}` (duplicate) | ✅ Blocked: "already checked in" |
| `POST /api/checkin/{id}` (Online) | ✅ Blocked: "not In-Person" |
| `POST /api/checkin/{id}` (no auth) | ✅ 401: "missing authentication token" |

Sample claim_url returned:
```
https://bethere.solana-thailand.workers.dev/claim/019dbd2a-1b2d-70c0-9a54-810e99631c99
```

#### Production Deploy

Deployed to `https://bethere.solana-thailand.workers.dev`:
- Version ID: `db7c8fff-d93c-43fa-828f-b599bea01693`
- Health check: ✅
- Auth URL: ✅
- Unauthenticated check-in: ✅ returns 401
- **⚠️ Production QR URLs still need regeneration** via `POST /api/generate-qrs?force=true` on production (they'll use production `SERVER_URL`)