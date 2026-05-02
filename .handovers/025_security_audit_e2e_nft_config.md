# 025 — Security Audit, E2E Test Script, and Fixes

## What Happened

Conducted a full security audit of the BeThere event platform after the Rust Adventures implementation (handover 024). The audit uncovered 3 critical bugs in the adventure save handler and several security warnings. All critical issues were fixed. An automated E2E test script was created. The session also identified the NFT config setup as the remaining blocker for production.

## Branch

- `develop` (a0d7cba) → merged to `main` (a0d7cba) — fast-forward

## Key Changes

### Security Fixes (7 files, +1186 / -12 lines)

| File | Change |
|------|--------|
| `worker/src/handlers/adventure.rs` | Added `get_attendee_by_claim_token` validation — only checked-in attendees can save progress |
| `worker/src/handlers/adventure.rs` | Populated `required_levels` from config — was always empty, making `passed` flag unreachable |
| `worker/src/adventure.rs` | Fixed `passed` flag logic — fallback to marking passed when any level completed |
| `worker/src/handlers/auth.rs` | Added `Secure` flag to session and logout cookies |
| `domain/src/config/types.rs` | Manual `Debug` impls for `AppConfig`, `GoogleOAuthConfig`, `GoogleServiceAccountConfig` — redacts secrets |

### New Files

| File | Description |
|------|-------------|
| `scripts/e2e/test_devnet.sh` | 7-test automated E2E test suite for devnet |
| `.issues/005_growth_marketing_plan.md` | Growth marketing plan |
| `.issues/006_rust_adventures.md` | Rust Adventures game design document |

### E2E Test Script (`scripts/e2e/test_devnet.sh`)

| # | Test | Result |
|---|------|--------|
| 1 | Health check (`GET /api/health`) | ✅ Automated |
| 2 | Frontend serves `index.html` | ✅ Automated |
| 3 | Claim token lookup (invalid token) | ✅ Automated |
| 4 | Adventure status (invalid token) | ✅ Automated |
| 5 | Helius mint (real cNFT on devnet) | ✅ Automated |
| 6 | Admin config CRUD (adventure) | ✅ Automated |
| 7 | Adventure save progress | ✅ Automated |
| 8 | Full browser login → check-in flow | ⏭️ Manual |
| 9 | Full claim → mint flow | ⏭️ Manual |

Usage: `./scripts/e2e/test_devnet.sh [--mint-only] [AUTH_TOKEN] [CLAIM_TOKEN]`

## Security Audit Findings

### Critical (Fixed)

1. **Adventure save had no attendee validation** — anyone with a token could save progress without being checked in
2. **`required_levels` was always empty** — `passed` flag never set → adventure gate blocked everyone permanently
3. **`passed` flag logic only ran when `required_levels` non-empty** — dead code path

### Warnings (Fixed)

4. **Session cookie missing `Secure` flag** — only had `HttpOnly; SameSite=Lax`
5. **`#[derive(Debug)]` leaked secrets** — jwt_secret, helius_api_key, client_secret, private_key exposed in debug output

### Deferred

6. **Double-claim race condition** — check-then-mint pattern not atomic, two simultaneous POST requests could both pass `claimed_at.is_none()` check
7. **No JWT revocation** — compromised staff tokens valid for full 24h expiry
8. **Secret configs fall back to empty string** — NFT features silently disabled instead of failing fast

### Confirmed Secure

- **Claim handler** — all gates checked sequentially with early returns, no bypass path
- **Admin routes** — behind `require_auth` middleware with staff email verification
- **Auth handler** — JWT with HMAC-SHA256, constant-time signature comparison, 24h expiry
- **Solana RPC** — no injection vectors, hardcoded method, serde serialization
- **Wallet validation** — checks length (32-44) and base58 character set
- **Secrets storage** — all via `env.secret()` (Workers encrypted secrets)
- **Cookie** — `HttpOnly`, `SameSite=Lax`, `Path=/api`, `Secure` (after fix)
- **.gitignore** — `worker/.dev.vars` properly excluded

## NFT Config Setup (Next Step)

The claim flow requires these to be configured before minting works:

| Config | Scope | How to Set |
|--------|-------|------------|
| `HELIUS_API_KEY` | Global (Worker secret) | `wrangler secret put HELIUS_API_KEY` |
| `nft_collection_mint` | Per-event (Admin UI) | Optional — Helius uses its own tree |
| `nft_metadata_uri` | Per-event (Admin UI) | Arweave/IPFS metadata JSON URL |
| `nft_image_url` | Per-event (Admin UI) | Arweave/IPFS/CDN image URL |

### Metadata JSON Format (Metaplex standard)

```json
{
  "name": "BeThere - {Event Name}",
  "symbol": "BETHERE",
  "description": "Proof of attendance at {Event Name}",
  "image": "https://arweave.net/{image_hash}",
  "external_url": "https://bethere.solana-thailand.workers.dev",
  "attributes": [
    { "trait_type": "Event", "value": "{Event Name}" },
    { "trait_type": "Type", "value": "Attendance Badge" },
    { "trait_type": "Date", "value": "2025-01-01" }
  ],
  "properties": {
    "category": "image",
    "files": [{ "uri": "https://arweave.net/{image_hash}", "type": "image/png" }]
  }
}
```

### Availability Check

The claim page shows NFT as available only when all 3 fields + API key are set:

```rust
let nft_available = !event.nft_metadata_uri.is_empty()
    && !event.nft_image_url.is_empty()
    && !state.config.helius_api_key.is_empty();
```

## Remaining Work

### 🔴 Blocking
- [ ] NFT config setup — upload image, create metadata JSON, set Helius secret
- [ ] Full browser E2E test — login → quiz → adventure → claim → mint

### 🟡 High Priority
- [ ] Playtest all 10 adventure levels
- [ ] KV-based claim dedup lock — prevent double-claim race condition
- [ ] Deploy security fixes to staging/production

### 🟢 Future
- [ ] JWT revocation mechanism — KV-based token blacklist
- [ ] Fail-fast for missing secrets — worker should refuse to start
- [ ] Mobile D-pad hold-to-repeat
- [ ] Sound effects
- [ ] Accessibility — keyboard hints, screen reader support

## How to Dev/Test

```bash
# Run E2E test suite
./scripts/e2e/test_devnet.sh

# Run only mint test (needs HELIUS_API_KEY in worker/.dev.vars)
./scripts/e2e/test_devnet.sh --mint-only

# Run with pre-authenticated tokens
AUTH_TOKEN=xxx CLAIM_TOKEN=yyy ./scripts/e2e/test_devnet.sh

# Dev server
cd worker && ./deploy.sh dev

# Run all unit tests
cargo test

# Clippy
cargo clippy --all-targets
```

## Issues Ref

- `.issues/007_devnet_e2e_test.md` — E2E test tracking
- `.issues/008_nft_config_and_production_readiness.md` — NFT config + production checklist
