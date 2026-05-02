# 024 — Rust Adventures Game

## What Happened

Implemented a complete 10-level tile-based puzzle game ("Rust Adventures") that teaches Rust programming concepts. The game is integrated into the event claim flow as an optional gate between quiz and NFT minting.

## Branch

- `feat/adventure-fixes` — squash merged into `develop` as `020000f`
- Fast-forward merged `develop` → `main`
- Feature branch deleted

## Key Changes

### Frontend (Leptos WASM)

| File | Description |
|------|-------------|
| `pages/adventure/page.rs` | Main game component — grid, puzzles, D-pad, level select |
| `pages/adventure/engine.rs` | Game engine — movement, collision, key/gate/puzzle logic |
| `pages/adventure/types.rs` | Domain types — LevelData, Tile, KeyDef, NpcDef, etc. |
| `pages/adventure/mod.rs` | Level definitions (10 built-in levels) + AdventureConfig |
| `pages/adventure_config.rs` | Admin UI to enable/configure adventure per event |
| `pages/admin.rs` | Adventure section in admin sidebar |
| `pages/claim.rs` | Quiz → Adventure gate flow before wallet input |
| `pages/mod.rs` | Adventure module registration |
| `api.rs` | Adventure API methods (status, save, admin config) |
| `lib.rs` | `/adventure` route |
| `style.css` | ~400 lines of adventure CSS, D-pad, animations |

### Backend (Axum on Cloudflare Workers)

| File | Description |
|------|-------------|
| `worker/src/adventure.rs` | Adventure business logic — save progress, check status |
| `worker/src/handlers/adventure.rs` | API endpoints — save, status, admin CRUD |
| `worker/src/handlers/claim.rs` | Adventure gate enforcement on mint |
| `worker/src/handlers/mod.rs` | Route registration |
| `domain/src/models/adventure.rs` | Domain models — AdventureConfig, AdventureProgress, AdventureStatus |

### Features Implemented

- **10 Rust learning levels** — movement, keys, gates, code puzzles, NPCs
- **Per-event adventure config** — enable/disable, set required level threshold
- **Claim flow integration** — quiz → adventure gate → wallet → mint
- **Admin UI** — Adventure Config section with toggle + level selector
- **Active Event selector** — moved to top of admin sidebar
- **localStorage persistence** — demo mode progress without claim token
- **No-token info banner** — "Playing in demo mode" with NFT hint
- **Play Again button** — resets progress for demo players
- **Mobile D-pad** — 56px default, 64px on touch devices
- **Viewport camera** — auto-scrolls to follow player on large grids

## Claim Flow (Full Attendee Journey)

```
1. Staff scans QR → check-in → claim token generated
2. /claim/{token} → attendee sees NFT preview
3. Quiz gate (if enabled) → must pass threshold
4. Adventure gate (if enabled) → must complete required levels
5. /adventure?token=xxx → play levels → auto-save progress
6. All required levels done → "Claim your NFT Badge" → back to /claim/{token}
7. Wallet input → cNFT minted on Solana via Helius
```

## Struggling / Solved

- **Build errors with Leptos 0.7 signals** — `WriteSignal` doesn't have `.get()`, must use read signal for reading
- **`Storage::remove` doesn't exist** — correct API is `Storage::remove_item`
- **`JsValue` doesn't implement `Display`** — can't use in `format!()`, removed error formatting
- **Quiz → Adventure gate was missing** — quiz `Passed` action jumped straight to wallet input, skipping adventure check. Fixed by adding `check_adventure_and_proceed` closure
- **Squash merge chose over rebase** — 13 commits with overlapping style.css changes made rebase impractical (26 conflicts to resolve one-by-one). Squash merge resolved all at once

## Remaining Work

- [ ] NFT config setup — event needs `nft_metadata_uri`, `nft_image_url`, `nft_collection_mint`, `helius_api_key` for claim flow to work
- [ ] E2E test — full check-in → quiz → adventure → claim → mint with real API
- [ ] Mobile D-pad hold-to-repeat — faster navigation on mobile
- [ ] Sound effects (optional, toggle)
- [ ] Trunk config migration — `address` → `addresses` field (deprecation warning)
- [ ] Accessibility — keyboard hints, screen reader support

## How to Dev/Test

```bash
# Dev server
cd worker && nohup bash deploy.sh dev > /tmp/wrangler.log 2>&1 &
# Frontend rebuild (after changes)
cd frontend-leptos && CARGO_BUILD_JOBS=1 bash build.sh
# Access at http://localhost:8787

# Key routes
/adventure          → game (demo mode, localStorage)
/adventure?token=X  → game (claim flow, API save)
/admin              → Adventure Config section
/claim/{token}      → full claim flow

# API endpoints
GET  /api/adventure/{token}/status
POST /api/adventure/{token}/save
GET  /api/admin/adventure?event_id=X
PUT  /api/admin/adventure?event_id=X
```

## Issues Ref

- `.issues/006_rust_adventures.md`
