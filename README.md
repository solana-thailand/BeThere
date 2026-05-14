# BeThere — Solana-Powered Event Check-In

**Turn every event into an on-chain experience.**

[![Solana](https://img.shields.io/badge/Solana-Devnet-9945FF?logo=solana)](https://solana.com)
[![Rust](https://img.shields.io/badge/Rust-100%25-000000?logo=rust)](https://www.rust-lang.org/)
[![Cloudflare Workers](https://img.shields.io/badge/Edge-Cloudflare-F38020?logo=cloudflare)](https://workers.cloudflare.com/)
[![Tests](https://img.shields.io/badge/tests-61%20passing-success)](./scripts/e2e/)


> Free events have **30-40% no-show rates**. BeThere fixes this with **USDC deposit commitments** — attendees get their money back when they show up, forfeit if they don't. Built on Solana for **$0.001 NFT badges**, **$0.00087 on-chain costs**, and **< 500ms check-in** at the edge.

### 🎯 The Problem → The Solution

| Problem | BeThere Solution |
|---------|------------------|
| 30-40% no-show rates for free events | USDC deposit commitment — skin in the game |
| No on-chain proof of attendance | Compressed NFT badges (cNFT) — 990x cheaper than POAP |
| Web2-only event tools | Solana-native: deposits, refunds, NFTs all on-chain |
| Expensive NFT minting ($0.50/ea) | cNFT on Solana: **$0.001 per badge** |
| ETH gas fees too high | Solana: **$0.00087 per transaction** |

### 🏗️ Stack

`Rust` → `Solana (Quasar)` → `Cloudflare Workers` → `Leptos WASM` → `Google Sheets`

100% Rust codebase — shared types from on-chain program → edge worker → WASM frontend. Zero serialization bugs.

### 📊 Key Numbers

| Metric | Value |
|--------|-------|
| On-chain program | **63 KB** (optimized) |
| NFT mint cost | **$0.001** per badge |
| Transaction cost | **$0.00087** (at $172/SOL) |
| Check-in latency | **< 500ms** (edge worker) |
| Tests | **66 passing** (39 worker + 27 on-chain) |
| Program ID (devnet) | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |

### 🎮 Live Demo Flow (Devnet)

```
1. 📋 Organizer creates event → sets $5 USDC deposit
2. 🪙 Attendee deposits USDC via Phantom wallet (Solana Pay QR)
3. 📱 Staff scans QR at door → on-chain check-in
4. 💰 Attendee gets refund + compressed NFT badge
5. ❌ No-show? → Organizer claims forfeited deposit
```

---

## Quick Start

```bash
# 1. Install prerequisites
cargo install wasm-bindgen-cli --version 0.2.100
cd worker && npm install && cd ..

# 2. Build frontend
cd frontend-leptos && trunk build && cd ..

# 3. Configure secrets (first time only)
cd worker
npx wrangler secret put JWT_SECRET
npx wrangler secret put GOOGLE_CLIENT_ID
npx wrangler secret put GOOGLE_CLIENT_SECRET
npx wrangler secret put GOOGLE_REDIRECT_URI
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_EMAIL
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_TOKEN_URI
npx wrangler secret put GOOGLE_SHEET_ID
npx wrangler secret put STAFF_EMAILS
npx wrangler secret put SUPER_ADMIN_EMAILS

# Dev mode (optional — for local E2E testing without Google OAuth)
# ⚠️ NEVER enable in production!
echo 'DEV_MODE = "1"' >> worker/.dev.vars
echo 'DEV_EMAIL = "your-email@example.com"' >> worker/.dev.vars

# 4. Create KV namespaces (first time only)
npx wrangler kv namespace create EVENTS
npx wrangler kv namespace create EVENTS --preview
# Update wrangler.toml with returned IDs

# 5. Run locally
cd worker && ./deploy.sh dev

# 6. Seed first event (after server is running)
curl -X POST http://localhost:8787/api/events/seed -H "Cookie: session=<jwt>"
```

Open `http://localhost:8787`.

> **Note:** `deploy.sh` automatically handles the Yarn PnP (`~/.pnp.cjs`) conflict with wrangler's esbuild bundler — no manual `mv` needed.

## Workspace Structure

```
event-checkin/
├── domain/               — Shared types & logic (compiles x86_64 + wasm32)
├── worker/               — Cloudflare Worker (wasm32-unknown-unknown)
├── frontend-leptos/      — Leptos WASM frontend (standalone trunk build)
├── Cargo.toml            — Workspace root (members: domain, worker)
└── README.md
```

The `domain/` crate contains shared types (`Attendee`, `Claims`, `AppConfig`), QR generation, and sheet row parsing. The `worker/` crate consumes it, replacing `reqwest` with `worker::Fetch` and `rsa`/`jsonwebtoken` with V8 SubtleCrypto via `wasm-bindgen`.

## Google Sheet Layout

The attendee sheet (tab name configurable via `GOOGLE_SHEET_NAME`, default `"checkin"`):

| Column | Index | Field | Notes |
|--------|-------|-------|-------|
| A | 0 | `api_id` | Unique ID (e.g. `gst-abc123`) |
| B | 1 | `name` | First name |
| C | 2 | `last_name` | Last name |
| D | 3 | `display_name` | Fallback display name |
| E | 4 | `email` | Attendee email |
| F | 5 | `ticket_name` | Ticket type |
| G | 6 | `solana_address` | Filled at claim time (attendee wallet) |
| H | 7 | `approval_status` | Approval state |
| I | 8 | `checked_in_at` | ISO 8601 timestamp |
| J | 9 | `checked_in_by` | Staff email who checked in |
| K | 10 | `qr_code_url` | QR code link |
| L | 11 | `claim_token` | UUID generated at check-in (for NFT claim) |
| M | 12 | `claimed_at` | Timestamp when NFT + refund claimed |
| Y | 24 | `participation_type` | In-Person / Online |

A separate **"staff"** sheet tab (configurable via `GOOGLE_STAFF_SHEET_NAME`) holds authorized staff emails in column A (header in row 1, emails from row 2). This is unioned with the `STAFF_EMAILS` secret — a user is staff if their email appears in either source.

## Deployment

```bash
# Build frontend (if changed)
cd frontend-leptos && trunk build && cd ..

# Deploy to Cloudflare Workers
cd worker && ./deploy.sh
```

The `deploy.sh` script handles the Yarn PnP conflict automatically. Alternatively, you can run `npx wrangler deploy` directly if you don't have `~/.pnp.cjs`.

Non-secret vars are in `worker/wrangler.toml` `[vars]`:

| Var | Default | Purpose |
|-----|---------|---------|
| `SERVER_URL` | `https://event-checkin.workers.dev` | Public URL for OAuth redirect |
| `GOOGLE_SHEET_NAME` | `checkin` | Attendee sheet tab name |
| `GOOGLE_STAFF_SHEET_NAME` | `staff` | Staff sheet tab name |
| `EVENT_NAME` | _(none)_ | Default event name for seeding |
| `ORGANIZER_EMAILS` | _(none)_ | Comma-separated organizer emails for seeding |
| `SUPER_ADMIN_EMAILS` | _(secret)_ | Global admins who can create/manage all events |

The frontend is served from `frontend-leptos/dist/` via Workers Assets with SPA fallback.

## API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/health` | No | Health check |
| GET | `/api/auth/url` | No | Google OAuth URL |
| GET | `/api/auth/callback` | No | OAuth callback, sets cookie |
| GET | `/api/auth/me` | Cookie | Current user info + role |
| GET | `/api/auth/logout` | No | Clear session cookie |
| GET | `/api/events` | Cookie | List all events |
| POST | `/api/events` | Cookie + SuperAdmin | Create new event |
| GET | `/api/events/{id}` | Cookie | Get event config |
| PUT | `/api/events/{id}` | Cookie + SuperAdmin | Update event config |
| DELETE | `/api/events/{id}` | Cookie + SuperAdmin | Archive event |
| POST | `/api/events/{id}/restore` | Cookie + SuperAdmin | Restore archived event to Draft |
| POST | `/api/events/seed` | Cookie + SuperAdmin | Seed event from env vars |
| POST | `/api/events/migrate` | Cookie + SuperAdmin | Migrate quiz KV → event KV |
| GET | `/api/attendees` | Cookie + Event Staff | List all attendees + stats |
| GET | `/api/attendee/{id}` | Cookie + Event Staff | Single attendee details |
| POST | `/api/checkin/{id}` | Cookie + Event Staff | Check in attendee |
| POST | `/api/generate-qrs` | Cookie + Event Staff | Generate QR codes |
| GET | `/api/quiz` | No | Get quiz questions (public) |
| POST | `/api/quiz/{token}/submit` | No | Submit quiz answers |
| GET | `/api/quiz/{token}/status` | No | Get quiz progress |
| PUT | `/api/admin/quiz` | Cookie + Staff | Create/update quiz config |
| GET | `/api/claim/{token}` | No | Get claim info |
| POST | `/api/claim/{token}` | No | Claim NFT badge + refund |
| GET | `/api/adventure/{token}/status` | No | Get adventure progress |
| POST | `/api/adventure/{token}/save` | No | Save adventure progress |
| GET | `/api/admin/adventure` | Cookie + Staff | Get adventure config |
| PUT | `/api/admin/adventure` | Cookie + Staff | Update adventure config |
| GET | `/api/deposit/status/{attendee_id}` | No | Check deposit status for attendee |
| POST | `/api/deposit/usdc` | No | Build Solana Pay deposit TX (USDC) |
| GET | `/api/deposit/usdc/confirm` | No | Poll deposit TX confirmation via Solana RPC |
| POST | `/api/deposit/usdc/webhook` | No | Record TX signature, verify on-chain |
| POST | `/api/deposit/thb/upload` | No | Upload PromptPay slip URL (THB) |
| GET | `/api/deposit/thb/pending` | Cookie + Staff | List pending THB slips |
| POST | `/api/deposit/thb/verify` | Cookie + Staff | Verify/reject THB slip |
| GET | `/api/refund/queue` | Cookie + Staff | List pending refunds |
| POST | `/api/refund/mark/{id}` | Cookie + Staff | Mark refund as completed |
| POST | `/api/escrow/create-event` | Cookie + Organizer | Initialize on-chain escrow PDA |
| GET  | `/api/deposit/usdc/tx` | No | Solana Pay TX callback (wallet fetches serialized TX) |
| POST | `/api/escrow/create-vault-ata` | Cookie + Organizer | Create vault's Associated Token Account |
| POST | `/api/escrow/mark-checked-in` | Cookie + Organizer | Mark attendee as checked-in on-chain (prerequisite for refund) |
| POST | `/api/escrow/refund` | No | Build refund TX for attendee's wallet to sign |
| DELETE | `/api/events/{id}/delete` | Cookie + SuperAdmin | Permanently delete event (`?force=true` for devnet cleanup) |
| GET | `/api/events/{id}/audit` | Cookie + Organizer | Get audit trail for event |
| GET | `/api/audit/global` | Cookie + SuperAdmin | Get system-wide audit trail |
| POST | `/api/escrow/deactivate-event` | Cookie + Organizer | Build deactivate escrow TX |
| POST | `/api/escrow/close-event` | Cookie + Organizer | Build close escrow TX (reclaim rent) |
| GET | `/api/public/events` | Public | List upcoming active events (nearest first) |

## Frontend Routes

| Path | Page | Auth |
|------|------|------|
| `/` | Landing — public marketing page, waitlist, how-it-works swimlane | Public |
| `/login` | Login — Google OAuth sign-in | Public |
| `/claim/{token}` | Claim — quiz + NFT badge + refund | Token-gated |
| `/staff` | Scanner — camera QR + manual lookup | Staff |
| `/admin` | Dashboard — attendee list, stats, QR management | Staff |
| `/admin/events` | Events — create, edit, manage events | SuperAdmin |
| `/adventure` | Rust Adventures — educational game | Public |
| `/deposit/{attendee_id}` | Deposit — wallet adapter + QR for USDC/THB deposit | Public |

## Architecture

```
worker/src/             — Cloudflare Worker
  handlers/             — API endpoints (auth, check-in, QR, attendee, events, quiz, claim, adventure, health)
    deposit.rs            — USDC/THB deposit, confirmation, webhook, refund queue
    ext.rs              — Shared utilities (EventIdQuery, resolve_event_with_access, resolve_kv)
  adventure.rs          — Adventure business logic (save progress, check completion)
  auth.rs               — Google OAuth + JWT + role resolution (super_admin/organizer/staff)
  error.rs              — Typed AppError → Axum IntoResponse integration
  event_store.rs        — KV event registry CRUD, seed, migration, hard_delete_event
  audit_store.rs        — Append-only audit trail (per-event + global, 27 action types)
  quiz.rs               — Quiz business logic (scoring, KV interaction)
  sheets.rs             — Google Sheets read/write (worker::Fetch) + KV attendee cache + token cache
  solana.rs             — Helius cNFT minting (mintCompressedNft RPC, MintRequest struct)
  solana_escrow.rs      — Solana escrow TX builders (deposit, refund, create_event, mark_checked_in, create_vault_ata)
  crypto.rs             — SubtleCrypto bridge (RSA-SHA256, HMAC-SHA256)
  http.rs               — HTTP client wrapping worker::Fetch
  middleware.rs         — Security headers, auth guard
  state.rs              — AppState from Env bindings

domain/src/             — Shared (compiles x86_64 + wasm32)
  config/               — AppConfig (grouped: OAuth, Sheets, Solana, Nft, Server, EventDefaults)
  models/               — Attendee, Claims, EventConfig, AdventureConfig, AppError, API response types
  qr/                   — QR URL generation + base64 image

frontend-leptos/src/
  pages/                — Landing, Login, Scanner, Admin, Claim, Quiz Editor, Adventure
  pages/adventure/      — Game engine, level definitions, types
  api.rs                — API client types and fetch wrappers
  components.rs         — Shared components + role helpers
  utils.rs              — Helpers (timestamps, badges, participation)
  js/                   — Camera + QR detection module
```

### Solana Escrow Architecture

The escrow system uses PDAs (Program Derived Addresses) to hold attendee USDC deposits on-chain. The escrow program is deployed on devnet at `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`.

**Escrow Flow (5 steps, all validated on devnet):**

```
1. create_event        →  Organizer signs  →  EventEscrow PDA + Vault ATA initialized (single TX)
2. deposit             →  Attendee signs   →  USDC → vault (Solana Pay)
3. mark_checked_in     →  Organizer signs  →  Attendee checked-in on-chain
4. refund              →  Attendee signs   →  USDC → attendee (after event ends)
5. claim_forfeited     →  Organizer signs  →  Forfeited deposits → organizer (after refund deadline)
```

**PDA Seeds:**
- `EventEscrow`: `["escrow", organizer_pubkey, event_id_u64_le]`
- `AttendeeDeposit`: `["deposit", event_escrow_pubkey, attendee_pubkey]`
- `Vault ATA`: Associated Token Account for (EventEscrow, USDC mint)

**Important constraints:**
- Refund requires `clock > event_end` (event must have ended) — no check-in required
- Deposits are rejected after the event has ended (`event_end > now` check)
- `mark_checked_in` rejects after `event_end` (SEC-011: prevents post-event attendance manipulation)
- `claim_forfeited` requires `clock > refund_deadline` (post-deadline only)
- All SPL token transfers use `transfer_checked()` with 6-decimal USDC (Token-2022 compatible)

> **Security note**: SEC-001 (check-in gate rug pull) has been fixed — refunds no longer require `checked_in == true`. Attendees can refund after `event_end` regardless of check-in status. See `docs/security_audit.md` for full audit.

**Constants:**
| Constant | Devnet | Mainnet |
|----------|--------|--------|
| Program ID | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` | TBD |
| USDC Mint | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m` |

**Transaction building:** All TX builders are in `worker/src/solana_escrow.rs`. They use a shared `build_message_accounts()` helper for Solana's canonical 4-pass account ordering.

### Performance Layers

| Layer | Mechanism | TTL | Benefit |
|-------|-----------|-----|--------|
| Google access token | KV cache | 3500s | Eliminates RSA-JWT signing per request (~100ms saved) |
| Attendee data | KV cache | 30s | Eliminates Google Sheets full-scan per request (~200-800ms saved) |
| Cache invalidation | On write-through | Immediate | Mutations (check-in, claim) invalidate cache instantly |

## Tests

```bash
# All unit tests
cargo test

# Individual crates
cargo test -p event-checkin-domain   # Shared types, QR logic
cargo test -p event-checkin-worker   # Crypto, auth, sheets, events

# Full 10-step E2E test (requires running worker + worker/.dev.vars with HELIUS_API_KEY)
./scripts/e2e/test_full_e2e.sh

# Devnet API test suite (7 tests, no browser needed)
./scripts/e2e/test_devnet.sh

# Mint-only test (single cNFT mint on devnet)
./scripts/e2e/test_devnet.sh --mint-only

# Worker WASM build check
cargo check -p event-checkin-worker --target wasm32-unknown-unknown

# Clippy
cargo clippy --all-targets
```

### Devnet Escrow E2E

```bash
# Full 5-step escrow E2E on Solana devnet (requires USDC-funded attendee wallet)
ATTENDEE_WALLET=~/.config/solana/id.json bash scripts/e2e/test_escrow_devnet.sh
```

See `scripts/e2e/test_escrow_devnet.sh` for the complete test flow. All 24 tests validated on devnet.

## Features

- **Camera QR Scanner** — BarcodeDetector (Chrome) + jsQR fallback (Firefox/Safari)
- **Staff check-in logging** — Records which staff member checked in each attendee (column J)
- **Sheet-based staff list** — Staff emails loaded from "staff" sheet tab + env var (unioned)
- **Participation types** — In-Person / Online badges, reject online at physical check-in
- **Admin stats** — Checked-in count, In-Person vs Online breakdown
- **Force QR regenerate** — Admin can regenerate codes per attendee
- **CSP compliant** — Zero `eval()` calls, no `unsafe-eval` directive
- **Edge deployment** — Cloudflare Workers with SubtleCrypto for JWT signing
- **Multi-event support** — KV-based event registry with per-event config, staff, quiz
- **Per-event access control** — 3-tier role system: super_admin → organizer → staff
- **Quiz-gated claim** — Attendees complete quiz before claiming NFT badge
- **Landing page** — Public marketing page with interactive swimlane (3-role walkthrough), waitlist, FAQ, social proof
- **Rust Adventures** — Educational tile-based game teaching Solana/Rust concepts
- **Security hardened** — Cookie Secure flag, secret redaction in Debug, attendee-validated adventure saves
- **Automated E2E tests** — 10-step full E2E suite (`scripts/e2e/test_full_e2e.sh`) + 7-test devnet suite
- **PDA escrow deposits** — USDC deposits held in on-chain PDAs, refundable after event
- **Solana Pay integration** — Deposit via QR code scan or wallet adapter (Phantom, Backpack, Solflare)
- **Dual-track deposits** — USDC (on-chain escrow) or THB (PromptPay QR + slip verification)
- **Single-TX escrow init** — Admin creates vault ATA + event escrow in one transaction via wallet signing (Phantom/Solflare)
- **On-chain check-in** — Staff marks attendees checked in on-chain via wallet-signed TX (escrow refund gate)
- **Wallet adapter interop** — Shared JS module for wallet detection, connection, TX signing across scanner + admin
- **Escrow lifecycle management** — Full deactivate → close flow in admin UI, rent reclamation
- **Force delete for devnet cleanup** — SuperAdmin can hard-delete events with `?force=true`
- **Slug auto-deduplication** — Recurring events get auto-incremented suffix on name collision
- **Audit trail** — Append-only event log tracking all state-changing operations (CRUD, escrow, deposits, check-ins) with actor attribution

## Security

| Area | Status | Notes |
|------|--------|-------|
| Auth | ✅ Secure | JWT HMAC-SHA256, constant-time comparison, 24h expiry |
| Cookie | ✅ Secure | `HttpOnly; Secure; SameSite=Lax; Path=/api` |
| Admin routes | ✅ Secure | `require_auth` middleware, staff email verification |
| Claim gates | ✅ Secure | Sequential check-in → quiz → adventure → mint, no bypass |
| Solana RPC | ✅ Secure | Hardcoded method, serde serialization, null-safe deserialization, no user-controlled params |
| Secrets | ✅ Secure | All via `env.secret()`, redacted from Debug output |
| Escrow (on-chain) | ✅ Secure | Immutable params after creation, checked arithmetic, canonical PDA derivation, `transfer_checked()` (SEC-009 fixed), `event_end` guard (SEC-011 fixed) |
| Escrow (business logic) | ✅ Secure | SEC-001/002/003/004 all fixed — refunds don't require check-in, fields locked after escrow init, $1K deposit cap, archive guards escrow |
| Escrow (Token-2022) | ✅ Secure | SEC-009 fixed — all transfers use `transfer_checked()` with 6-decimal USDC |
| Double-claim | ⚠️ Deferred | KV dedup lock recommended before high-traffic events |
| Audit logging | 🟡 Basic | Append-only audit trail per event (CRUD + escrow + check-in + deposits). Global audit for deletions. Missing: on-chain CPI event indexing, UI viewer |
| JWT revocation | ⚠️ Deferred | KV blacklist recommended for compromised tokens |
| Dev mode | ⚠️ Local only | `DEV_MODE=1` bypasses JWT verification — only for `.dev.vars`, never production |

See [`docs/security_audit.md`](docs/security_audit.md) for the full escrow security audit (11 findings, 8 fixed, Safe Solana Builder cross-reference). See `.handovers/025_security_audit_e2e_nft_config.md` for the earlier auth/RPC audit.

## Roles & Access Control

| Role | Can Do |
|------|--------|
| `super_admin` | Create/edit/delete events, manage all events, full dashboard |
| `organizer` | Edit assigned event config, manage quiz, view dashboard |
| `staff` | Check in attendees, view attendee list for assigned event |
| _(unauthenticated)_ | View landing page, play adventure, take quiz, claim NFT badge |

## 🏆 What's Built (Devnet-Validated)

✅ 9 phases complete — from check-in to escrow to security audit. Everything runs on Solana devnet with real wallets.

| Core Flow | Status | Details |
|-----------|--------|----------|
| QR check-in | ✅ | Camera scan + manual lookup, staff logging |
| cNFT badges | ✅ | Compressed NFTs via Helius, $0.001 mint |
| Quiz gating | ✅ | Per-event quiz before NFT claim |
| Adventure gating | ✅ | Rust-themed educational game (10 levels) |
| Multi-event | ✅ | KV registry, 3-tier roles (super_admin/organizer/staff) |
| USDC escrow | ✅ | PDA-based deposits, refund, claim forfeited |
| Dual-track payments | ✅ | USDC (on-chain) + PromptPay THB (fiat QR) |
| Wallet adapter | ✅ | Phantom, Solflare, Backpack, Coinbase |
| Security audit | ✅ | 11 findings, 8 fixed, SEC-001–011 addressed |
| E2E tests | ✅ | 61 tests (39 worker + 22 on-chain), devnet validated |

## 📈 Competitive Landscape

| Feature | BeThere | Luma | Eventbrite | POAP | Kickback* |
|---------|---------|------|------------|------|-----------|
| On-chain deposits | ✅ USDC escrow | ❌ | ❌ | ❌ | ✅ ETH (defunct) |
| Attendance NFTs | ✅ cNFT | ❌ | ❌ | ✅ (Ethereum) | ❌ |
| Deposit refund | ✅ Auto | ❌ | Manual | ❌ | ✅ Payout pool |
| No-show penalty | ✅ Forfeit to org | ❌ | ❌ | ❌ | ✅ Pool split |
| Quiz/Adventure gating | ✅ Built-in | ❌ | ❌ | ❌ | ❌ |
| Cost per NFT | **$0.001** | N/A | N/A | ~$0.50 | N/A |
| Stablecoin deposits | ✅ USDC | ❌ | ❌ | ❌ | ❌ (volatile ETH) |
| Open source | ✅ | ❌ | ❌ | ❌ | ✅ (archived) |

*\*Kickback (2016–2022) — Ethereum event deposit platform, shut down due to gas costs and team burnout. BeThere addresses every structural weakness. See [`docs/competitive_analysis_kickback.md`](docs/competitive_analysis_kickback.md) for full analysis.*

## 🗺️ Roadmap

| Phase | Feature | Status |
|-------|---------|--------|
| **1–6** | Check-in → NFT → Quiz → Multi-event → Adventure → Security | ✅ Done |
| **7** | NFT config + production deployment | 🟡 Devnet working |
| **8–9** | USDC escrow + security hardening | ✅ Done (devnet deployed) |
| **10** | **Mainnet deployment** | 📋 Next (~1.5 SOL cost) |
| **11** | Platform fees (1-2% on forfeited deposits) | 📋 Planned |
| **12** | Multi-organizer SaaS | 📋 Planned |

See **[DISCUSSION.md](./DISCUSSION.md)** for the full architecture direction and decisions.