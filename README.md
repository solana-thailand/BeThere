# BeThere — Solana-Powered Event Check-In

**Turn every event into an on-chain experience.**

[![Solana](https://img.shields.io/badge/Solana-Devnet-9945FF?logo=solana)](https://solana.com)
[![Rust](https://img.shields.io/badge/Rust-100%25-000000?logo=rust)](https://www.rust-lang.org/)
[![Cloudflare Workers](https://img.shields.io/badge/Edge-Cloudflare-F38020?logo=cloudflare)](https://workers.cloudflare.com/)
[![Tests](https://img.shields.io/badge/tests-85%20passing-success)](./scripts/e2e/)


> Free events have **30-40% no-show rates**. BeThere fixes this with **USDC deposit commitments** — attendees get their money back when they show up, forfeit if they don't. Built on Solana for **$0.001 NFT badges**, **$0.00087 on-chain costs**, and **< 500ms check-in** at the edge.

### 🎯 The Problem → The Solution

| Problem | BeThere Solution |
|---------|------------------|
| 30-40% no-show rates for free events | USDC deposit commitment — skin in the game |
| No on-chain proof of attendance | Compressed NFT badges (cNFT) — ~50× cheaper than POAP |
| Web2-only event tools | Solana-native: deposits, refunds, NFTs all on-chain |
| Expensive NFT minting ($0.05–0.20/ea on Gnosis) | cNFT on Solana: **$0.001 per badge** |
| ETH gas fees too high | Solana: **$0.00087 per transaction** |

### 🏗️ Stack

`Rust` → `Solana (Quasar)` → `Cloudflare Workers` → `Leptos WASM` → `Google Sheets`

100% Rust codebase — shared types from on-chain program → edge worker → WASM frontend. Zero serialization bugs.

### 📊 Key Numbers

| Metric | Value |
|--------|-------|
| On-chain program | **88 KB** (optimized, 89,856 bytes) |
| NFT mint cost | **$0.001** on-chain (cNFT) · hosted mint billed per Crossmint pricing |
| Transaction cost | **$0.00087** (at $172/SOL) |
| Check-in latency | **< 500ms** (edge worker) |
| Tests | **250 passing** (54 on-chain + 73 domain + 123 worker) + 147 frontend specs + 16 Kani harnesses |
| Program ID (devnet) | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |

> Every figure above is sourced in [`docs/sources.md`](docs/sources.md) — the evidence ledger (primary sources, measurement method, confidence, and known caveats). Refresh self-measured rows with `python3 scripts/measure_metrics.py`.

### 🎮 Live Demo Flow (Devnet)

```
1. 📋 Organizer creates event → sets $5 USDC deposit
2. 📝 Attendee clicks "Reserve Spot" → auto-redirect to deposit page
3. 🪙 Attendee deposits USDC via Phantom wallet (Solana Pay QR)
   → Or uploads THB slip → auto-redirect to ticket/QR page
4. 📱 Staff scans QR at door → on-chain check-in
5. 💰 Attendee gets refund + compressed NFT badge
6. ❌ No-show? → Organizer claims forfeited deposit
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

The attendee sheet (tab name configurable via `GOOGLE_SHEET_NAME`, default `"Attendees"`):

| Column | Index | Field | Notes |
|--------|-------|-------|-------|
| A | 0 | `api_id` | Unique ID (e.g. `gst-abc123`) |
| B | 1 | `name` | Full name |
| C | 2 | `first_name` | First name |
| D | 3 | `last_name` | Last name |
| E | 4 | `email` | Attendee email |
| F | 5 | `ticket_name` | Ticket type |
| G | 6 | `registration_date` | ISO 8601 registration date |
| H | 7 | `approval_status` | Approval state |
| I | 8 | `participation_type` | In-Person / Online |
| J | 9 | `phone` | Phone number |
| K | 10 | `contact_channel` | Telegram / Discord / etc. |
| L | 11 | `contact_handle` | Contact username |
| M | 12 | `deposit_agreed` | Yes/No |
| N | 13 | `deposit_method` | USDC / THB / etc. |
| O | 14 | `deposit_amount` | Deposit amount |
| P | 15 | `deposit_tx_signature` | Transaction signature |
| Q | 16 | `deposit_verified` | Deposit verification status |
| R | 17 | `checked_in_at` | ISO 8601 timestamp |
| S | 18 | `checked_in_by` | Staff email who checked in |
| T | 19 | `solana_address` | Filled at claim time (attendee wallet) |
| U | 20 | `qr_code_url` | QR code link |
| V | 21 | `claim_token` | UUID generated at check-in (for NFT claim) |
| W | 22 | `claimed_at` | Timestamp when NFT + refund claimed |

A separate **"staff"** sheet tab (configurable via `GOOGLE_STAFF_SHEET_NAME`) holds authorized staff emails in column A (header in row 1, emails from row 2). This is unioned with the `STAFF_EMAILS` secret — a user is staff if their email appears in either source.

## Deployment

```bash
# Build frontend (if changed)
cd frontend-leptos && bash build.sh && cd ..

# Deploy to Cloudflare Workers
cd worker && ./deploy.sh
```

The `deploy.sh` script tries `npx wrangler deploy` first. If Cloudflare's versions API fails (error 10013), it falls back to a direct API deployment using BLAKE3 asset manifests. Requires `pip3 install blake3` for the fallback path.

**Pre-deploy check**: Ensure `frontend-leptos/dist/` is non-empty before deploying — the worker embeds `index.html` at compile time.

Non-secret vars are in `worker/wrangler.toml` `[vars]`:

| Var | Default | Purpose |
|-----|---------|---------|
| `SERVER_URL` | `https://event-checkin.workers.dev` | Public URL for OAuth redirect |
| `GOOGLE_SHEET_NAME` | `Attendees` | Attendee sheet tab name |
| `GOOGLE_STAFF_SHEET_NAME` | `staff` | Staff sheet tab name |
| `EVENT_NAME` | _(none)_ | Default event name for seeding |
| `ORGANIZER_EMAILS` | _(none)_ | Comma-separated organizer emails for seeding |
| `SUPER_ADMIN_EMAILS` | _(secret)_ | Global admins who can create/manage all events |

The frontend is served from `frontend-leptos/dist/` via Workers Assets with SPA fallback.

## API Endpoints

### Auth & Users

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/health` | No | Health check |
| GET | `/api/auth/url` | No | Google OAuth URL (optional `?redirect=` param) |
| GET | `/api/auth/callback` | No | OAuth callback, sets HttpOnly cookie, redirects based on role |
| POST | `/api/auth/logout` | No | Clear session cookie |
| GET | `/api/auth/me` | Cookie | Current user info + role + `wallet_only`/`wallet_address` (Plan 017) |
| GET | `/api/my-registration/{slug}` | Cookie | Get signed-in user's registration for a specific event |
| GET | `/api/my-registrations` | Cookie | List all registrations for the signed-in user |

#### Sign-In With Solana + social linking (Plans 006 / 017)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/auth/wallet/nonce` | No | Issue a SIWS challenge; stores the exact message in KV (5-min TTL) |
| POST | `/api/auth/wallet/verify` | No | Verify the wallet's ed25519 signature over the stored challenge, issue session |
| POST | `/api/auth/wallet/bind` | Cookie | Bind a wallet to the signed-in account (ownership-verified via SIWS) |
| GET | `/api/auth/github` | Cookie | Start GitHub OAuth link (HMAC-signed `state` carrying email) |
| GET | `/api/auth/github/callback` | No | GitHub callback; verifies signed state, saves verified handle |
| GET | `/api/auth/telegram/config` | No | Whether the Telegram widget is enabled + bot username (never the token) |
| GET | `/api/auth/telegram/state` | Cookie | Signed state token embedded in the widget's `data-auth-url` |
| GET | `/api/auth/telegram/callback` | No | Telegram redirect flow; verifies signed state + widget HMAC, saves handle |
| POST | `/api/auth/social/unlink` | Cookie | Remove a verified social link (github/telegram/discord) |

### Public Event & Registration

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/public/events` | No | List upcoming active events (nearest first) |
| GET | `/api/public/event/{slug}` | No | Get public event details (no auth required) |
| POST | `/api/public/register` | Cookie | Register for event (email from JWT, not body) |
| GET | `/api/public/ticket/{id}` | No | Get attendee ticket/QR slip details |
| GET | `/api/badge.svg` | No | NFT badge SVG (by claim token) |
| GET | `/api/badge-hd.svg` | No | NFT badge HD SVG (by claim token) |
| POST | `/api/waitlist` | No | Join waitlist (email + use case) |

### Events (Admin)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/events` | Cookie | List all events |
| POST | `/api/events` | Cookie + SuperAdmin | Create new event |
| GET | `/api/events/{id}` | Cookie | Get event config |
| PUT | `/api/events/{id}` | Cookie + SuperAdmin | Update event config |
| DELETE | `/api/events/{id}` | Cookie + SuperAdmin | Archive event |
| POST | `/api/events/{id}/restore` | Cookie + SuperAdmin | Restore archived event to Draft |
| DELETE | `/api/events/{id}/delete` | Cookie + SuperAdmin | Permanently delete event (`?force=true` for devnet cleanup) |
| POST | `/api/events/seed` | Cookie + SuperAdmin | Seed event from env vars |
| POST | `/api/events/migrate` | Cookie + SuperAdmin | Migrate quiz KV → event KV |
| GET | `/api/events/{id}/audit` | Cookie + Organizer | Get audit trail for event |
| GET | `/api/audit/global` | Cookie + SuperAdmin | Get system-wide audit trail |

### Organizations (SuperAdmin)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/orgs` | Cookie + SuperAdmin | List all organizations |
| POST | `/api/orgs` | Cookie + SuperAdmin | Create organization |
| GET | `/api/orgs/{id}` | Cookie + SuperAdmin | Get organization details |
| PUT | `/api/orgs/{id}` | Cookie + SuperAdmin | Update organization |
| DELETE | `/api/orgs/{id}` | Cookie + SuperAdmin | Delete organization |

### Contacts (Master Sheet)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/contacts` | Cookie + Staff | List deduplicated master contacts |
| GET | `/api/contacts/events` | Cookie + Staff | List events tab |
| GET | `/api/contacts/stats` | Cookie + Staff | Contacts statistics |
| POST | `/api/contacts/sync` | Cookie + Staff | Sync contacts to Google Sheet |

### Attendees & Check-In

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/attendees` | Cookie + Event Staff | List all attendees + stats |
| GET | `/api/attendee/{id}` | Cookie + Event Staff | Single attendee details |
| POST | `/api/checkin/{id}` | Cookie + Event Staff | Check in attendee |
| POST | `/api/attendee/{id}/undo-checkin` | Cookie + Event Staff | Undo check-in |
| POST | `/api/generate-qrs` | Cookie + Event Staff | Generate QR codes |
| POST | `/api/admin/flush-cache` | Cookie + Staff | Flush server-side caches |

### Walk-in Attendees

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/walkin/register` | Cookie + Staff | Register walk-in attendee (on-the-spot) |
| GET | `/api/walkin/list` | Cookie + Staff | List walk-in attendees for event |
| GET | `/api/walkin/export` | Cookie + Staff | Export walk-in attendees as CSV |
| POST | `/api/walkin/sync` | Cookie + Staff | Sync walk-in attendees to Google Sheet |

### Quiz & Adventure

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/quiz` | No | Get quiz questions (public) |
| POST | `/api/quiz/{token}/submit` | No | Submit quiz answers |
| GET | `/api/quiz/{token}/status` | No | Get quiz progress |
| PUT | `/api/admin/quiz` | Cookie + Staff | Create/update quiz config |
| POST | `/api/admin/quiz/questions` | Cookie + Staff | Add individual quiz question |
| PUT | `/api/admin/quiz/questions/{id}` | Cookie + Staff | Update individual quiz question |
| DELETE | `/api/admin/quiz/questions/{id}` | Cookie + Staff | Delete individual quiz question |
| PATCH | `/api/admin/quiz/questions/{id}/toggle` | Cookie + Staff | Toggle quiz question active/inactive |
| GET | `/api/adventure/{token}/status` | No | Get adventure progress |
| POST | `/api/adventure/{token}/save` | No | Save adventure progress |
| GET | `/api/admin/adventure` | Cookie + Staff | Get adventure config |
| PUT | `/api/admin/adventure` | Cookie + Staff | Update adventure config |

### Claim & NFT

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/claim/{token}` | No | Get claim info |
| POST | `/api/claim/{token}` | No | Claim NFT badge + refund |

### Deposits

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/deposit/status/{attendee_id}` | No | Check deposit status for attendee |
| POST | `/api/deposit/usdc` | No | Build Solana Pay deposit TX (USDC) |
| GET | `/api/deposit/usdc/tx` | No | Solana Pay TX callback (wallet fetches serialized TX) |
| GET | `/api/deposit/usdc/confirm` | No | Poll deposit TX confirmation via Solana RPC |
| POST | `/api/deposit/usdc/webhook` | No | Record TX signature, verify on-chain |
| POST | `/api/deposit/thb/upload` | Cookie | Upload PromptPay slip image (THB, attendee's own slip) |
| GET | `/api/deposit/thb/pending` | Cookie + Staff | List pending THB slips |
| POST | `/api/deposit/thb/verify` | Cookie + Staff | Verify/reject THB slip |
| POST | `/api/deposit/hold` | Cookie | Hold deposit as rolling credit for next event |
| GET | `/api/deposit/credit-balance` | Cookie | Check deposit credit balance |

### R2 Storage

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/storage/slips/{event_id}/{attendee_id}` | No | Serve slip image from R2 |
| GET | `/api/storage/refunds/{event_id}/{attendee_id}` | No | Serve refund proof from R2 |
| GET | `/api/storage/badges/{event_id}` | No | Serve badge SVG from R2 |

### Escrow (On-Chain)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/escrow/init` | Cookie + Organizer | Initialize on-chain escrow PDA + vault ATA (single TX) |
| POST | `/api/escrow/confirm-init` | Cookie + Organizer | Confirm escrow init after wallet signature |
| POST | `/api/escrow/mark-checked-in` | Cookie + Organizer | Mark attendee checked-in on-chain |
| POST | `/api/escrow/refund` | No | Build refund TX for attendee's wallet to sign |
| POST | `/api/escrow/claim-forfeited` | Cookie + Organizer | Claim forfeited deposit (no-show) |
| POST | `/api/escrow/deactivate-event` | Cookie + Organizer | Build deactivate escrow TX |
| POST | `/api/escrow/close-event` | Cookie + Organizer | Build close escrow TX (reclaim rent) |
| POST | `/api/escrow/close-deposit` | No | Close individual deposit PDA (rent reclaim) |
| POST | `/api/escrow/rollover-deposit` | Cookie | Build atomic rollover deposit TX (attendee-authed) |
| POST | `/api/escrow/backfill-wallets` | Cookie + Organizer | Backfill wallet addresses from KV to on-chain |
| GET | `/api/escrow/events/{event_id}` | Cookie + Organizer | Get on-chain escrow event data |
| POST | `/api/escrow/sync` | Cookie + Organizer | Sync on-chain escrow events to KV cache |
| POST | `/api/escrow/onchain-webhook` | No | Webhook for on-chain escrow events (Helius) |
| GET | `/api/escrow/health` | Cookie + Staff | Escrow health check |

### Refunds & Cancellation

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/refund/queue` | Cookie + Staff | List pending refunds |
| GET | `/api/refund/refunded` | Cookie + Staff | List already-refunded attendees |
| POST | `/api/refund/mark/{id}` | Cookie + Staff | Mark refund as completed |
| POST | `/api/refund/manual/{attendee_id}` | Cookie + Staff | Mark manual refund |
| POST | `/api/refund/batch-thb` | Cookie + Staff | Batch THB refund for event cancellation |
| GET | `/api/escrow/refund-queue` | Cookie + Staff | USDC refund queue (cancellation workflow) |
| GET | `/api/escrow/cancel-status` | Cookie + Staff | Event cancellation status overview |

## Frontend Routes

| Path | Page | Auth |
|------|------|------|
| `/` | Landing — marketing page, upcoming events, My Registrations (auth-aware nav) | Public |
| `/login` | Login — Google OAuth sign-in (staff/organizer entry point) | Public |
| `/e/{slug}` | Public Event — event details, countdown, registration with Google Sign-In | Public |
| `/deposit/{attendee_id}` | Deposit — wallet adapter + QR for USDC/THB deposit | Public |
| `/ticket/{attendee_id}` | Ticket — QR code slip with check-in status + rollover option | Public |
| `/claim/{token}` | Claim — quiz + NFT badge + refund | Token-gated |
| `/staff` | Scanner — camera QR + manual lookup + walk-in registration | Staff |
| `/admin` | Dashboard — attendee list, stats, escrow, cancellation, walk-in export/sync | Staff |
| `/admin/events` | Events — create, edit, manage events | SuperAdmin |
| `/adventure` | Rust Adventures — educational game | Public |
| `/privacy` | Privacy Policy — PDPA compliance | Public |

## Architecture

```
worker/src/             — Cloudflare Worker
  handlers/             — API endpoints
    deposit/              — Deposit/refund handlers (split by payment track)
      usdc/                — USDC deposit flow: status, initiate, TX callback, confirm, webhook
      thb/                 — THB slip upload/verify, refund queue, batch refund
      escrow/              — On-chain escrow: init, mark-checked-in, close, claim-forfeited, cancel, rollover
    ext.rs              — Shared utilities (EventIdQuery, resolve_event_with_access, resolve_kv)
    contacts.rs         — Master contacts list management (deduplicated cross-event)
    escrow_index.rs     — On-chain escrow event indexing (Helius webhook + RPC poller)
    orgs.rs             — Organization CRUD (SuperAdmin)
    user_log.rs         — User sign-in logging to Google Sheets
  adventure.rs          — Adventure business logic (save progress, check completion)
  auth.rs               — Google OAuth + JWT + role resolution (super_admin/organizer/staff)
  claim/                — Claim logic (lock, mint, orchestrator)
  cleanup.rs            — Cron KV cleanup for expired event data (retention policy)
  db.rs                 — D1 claim lock operations (atomic double-claim prevention)
  error.rs              — Typed AppError → Axum IntoResponse integration
  escrow_indexer/       — On-chain escrow event indexer
    webhook.rs            — Helius enhanced webhook parsing
    poller.rs             — RPC-based event poller
    store.rs              — KV storage for indexed events
  event_store/          — KV event registry CRUD
    read.rs               — Event read operations
    write.rs              — Event write operations + schema
  audit_store.rs        — Append-only audit trail (per-event + global, 27 action types)
  org_store.rs          — Organization KV store CRUD
  quiz.rs               — Quiz business logic (scoring, KV interaction)
  sheets/               — Google Sheets API
    mod.rs                — Access token, column mapping, attendee/staff queries, KV cache
    write.rs              — Sheet mutations: check-in, claim, QR URLs, row append
    contacts.rs           — Master contacts deduplication + sync
    events_tab.rs         — Events tab management
  solana.rs             — Crossmint hosted cNFT minting (fire-and-poll REST, MintRequest struct, KV idempotency marker)
  solana_escrow/        — Solana escrow TX builders
    mod.rs                — Types, constants, EscrowError
    crypto.rs             — SHA-256, base58, PDA/ATA derivation (WASM SubtleCrypto + native)
    wire.rs               — Blockhash cache, tx serialization, message account ordering
    tx_builders/          — Per-instruction builder files
      init.rs               — create_event + init_escrow
      deposit.rs            — deposit
      mark.rs               — mark_checked_in
      refund.rs             — refund + close_deposit
      rollover.rs           — rollover_deposit (atomic cross-vault transfer)
      close.rs              — deactivate_event + close_event + claim_forfeited
  storage.rs            — R2 storage helpers (slip/refund/badge serving)
  crypto.rs             — SubtleCrypto bridge (RSA-SHA256, HMAC-SHA256)
  http.rs               — HTTP client wrapping worker::Fetch
  middleware/           — HTTP middleware
    cache.rs              — Cache-Control layers (public-60, public-120, no-store, no-cache)
    correlation.rs        — Correlation ID propagation
    headers.rs            — Security headers
    rate_limit.rs         — In-memory rate limiting
  state.rs              — AppState from Env bindings (KV, D1, R2 cached in OnceLock)

domain/src/             — Shared (compiles x86_64 + wasm32)
  config/               — AppConfig (grouped: OAuth, Sheets, Solana, Nft, Server, EventDefaults)
  models/               — Attendee, Claims, EventConfig, OrgConfig, Deposit models, AppError, API response types
  qr/                   — QR URL generation + base64 image

frontend-leptos/src/
  pages/                — Landing, Login, Scanner, Admin, Claim, Quiz Editor, Adventure, Privacy
  pages/adventure/      — Game engine, level definitions, types
  pages/deposit/        — Deposit flow subcomponents
  pages/public_event/   — Public event page subcomponents
  pages/ticket/         — Ticket/QR slip subcomponents
  api.rs                — API client types and fetch wrappers
  components.rs         — Shared components + role helpers
  utils.rs              — Helpers (timestamps, badges, participation)
  js/                   — Camera + QR detection + Solana wallet adapter module
```

### Solana Escrow Architecture

The escrow system uses PDAs (Program Derived Addresses) to hold attendee USDC deposits on-chain. The escrow program is deployed on devnet at `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`.

**Escrow Flow (6 steps, all validated on devnet):**

```
1. create_event        →  Organizer signs  →  EventEscrow PDA + Vault ATA initialized (single TX)
2. deposit             →  Attendee signs   →  USDC → vault (Solana Pay)
3. mark_checked_in     →  Organizer signs  →  Attendee checked-in on-chain
4. refund              →  Attendee signs   →  USDC → attendee (after event ends)
5. claim_forfeited     →  Organizer signs  →  Forfeited deposits → organizer (after refund deadline)
6. rollover_deposit    →  Attendee signs   →  Atomic deposit transfer → next event's vault (same organizer)
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

**Transaction building:** All TX builders are in `worker/src/solana_escrow/tx_builders/`. They share an `EscrowCtx` that resolves program IDs + derives PDAs once, and a `finalize_tx()` helper that handles message building → blockhash → serialization → base64. Each instruction has its own builder file for maintainability.

### Performance Layers

| Layer | Mechanism | TTL | Benefit |
|-------|-----------|-----|--------|
| Google access token | KV cache | 3500s | Eliminates RSA-JWT signing per request (~100ms saved) |
| Attendee data | KV cache | 30s | Eliminates Google Sheets full-scan per request (~200-800ms saved) |
| Solana blockhash | KV cache | 30s | Eliminates RPC call per TX build (~50-200ms saved) |
| TX builder context | `EscrowCtx` | Per-request | Resolves 7 program IDs + 3 PDAs once instead of per-instruction (~3-5ms saved) |
| Cache invalidation | On write-through | Immediate | Mutations (check-in, claim) invalidate cache instantly |

## Tests

```bash
# All off-chain tests (245 total)
cargo test -p event-checkin-domain   # 73 tests — shared types, QR logic
cargo test -p event-checkin-worker   # 80 tests — crypto, auth, sheets, events
cd frontend-leptos && cargo test     # 92 tests — Leptos pages, adventure playtests

# On-chain SVM tests (42 total)
cd bethere-escrow && quasar test     # All 9 escrow instructions + rollover lifecycle

# E2E devnet scripts
./scripts/e2e/run_all_e2e.sh                      # Run all E2E scripts
./scripts/e2e/run_all_e2e.sh --only rollover      # Single script
./scripts/e2e/test_escrow_devnet.sh               # 5-step escrow flow
./scripts/e2e/test_rollover_devnet.sh             # Rollover + refund lifecycle
./scripts/e2e/test_rollover_full_lifecycle.sh     # 2-attendee full lifecycle
./scripts/e2e/test_full_e2e.sh                    # Browser E2E
./scripts/e2e/test_devnet.sh                      # API test suite
./scripts/e2e/test_lifecycle.sh                   # Create → close (no deposits)

# Worker WASM build check
cargo check -p event-checkin-worker --target wasm32-unknown-unknown

# Clippy
cargo clippy --all-targets
```

## Features

- **Camera QR Scanner** — BarcodeDetector (Chrome) + jsQR fallback (Firefox/Safari)
- **Staff check-in logging** — Records which staff member checked in each attendee (column J)
- **Sheet-based staff list** — Staff emails loaded from "staff" sheet tab + env var (unioned)
- **Event format model** — In-Person / Online / Hybrid with participation-type badges
- **Admin stats** — Checked-in count, In-Person vs Online breakdown
- **Force QR regenerate** — Admin can regenerate codes per attendee
- **CSP compliant** — Zero `eval()` calls, no `unsafe-eval` directive
- **Edge deployment** — Cloudflare Workers with SubtleCrypto for JWT signing
- **Multi-event support** — KV-based event registry with per-event config, staff, quiz
- **Per-event access control** — 4-tier role system: super_admin → organizer → staff → attendee
- **Google Sign-In for attendees** — Dual-purpose OAuth for staff and attendees; email locked to Google account
- **Self-registration** — Attendees register via public event page (`/e/{slug}`) with Google identity
- **My Registrations** — Signed-in attendees see their events + status on landing page with auth-aware nav
- **Quiz-gated claim** — Attendees complete quiz before claiming NFT badge
- **Landing page** — Persona toggle (Attendees/Organizers), synced tabbed timelines, upcoming events, My Registrations with 4-column card grid, sandbox empty state, waitlist, FAQ, social proof
- **Rust Adventures** — Educational tile-based game teaching Solana/Rust concepts
- **Security hardened** — Cookie Secure flag, secret redaction in Debug, attendee-validated adventure saves
- **Automated E2E tests** — 10-step full E2E suite + 7-test devnet suite
- **PDA escrow deposits** — USDC deposits held in on-chain PDAs, refundable after event
- **Solana Pay integration** — Deposit via QR code scan or wallet adapter (Phantom, Backpack, Solflare)
- **Dual-track deposits** — USDC (on-chain escrow) or THB (PromptPay QR + slip verification)
- **Single-TX escrow init** — Admin creates vault ATA + event escrow in one transaction via wallet signing
- **On-chain check-in** — Staff marks attendees checked in on-chain via wallet-signed TX (escrow refund gate)
- **Wallet adapter interop** — Shared JS module for wallet detection, connection, TX signing across scanner + admin
- **Wallet error recovery** — Structured error classification with user-friendly guidance (wrong network, insufficient funds, user rejected, program error)
- **Escrow lifecycle management** — Full deactivate → close flow in admin UI, rent reclamation
- **Event cancellation workflow** — THB batch refund + USDC refund queue + cancel status (organizer-initiated)
- **Walk-in attendee management** — On-the-spot registration, CSV export, Google Sheet sync with idempotency
- **Force delete for devnet cleanup** — SuperAdmin can hard-delete events with `?force=true`
- **Slug auto-deduplication** — Recurring events get auto-incremented suffix on name collision
- **Audit trail** — Append-only event log tracking all state-changing operations with actor attribution
- **Dev-mode payment gating** — Solana wallet options hidden in production, shown only when `dev_mode: true`
- **Attendee flow persistence** — localStorage resume for partial registrations, auto-redirect to deposit/ticket page
- **Rollover deposits** — Atomic on-chain transfer of checked-in deposit to next event (no withdraw + re-deposit)
- **D1 claim locks** — Atomic double-claim prevention via Cloudflare D1 SQLite
- **R2 asset storage** — Zero-egress slip images, refund proofs, badge SVGs on Cloudflare R2
- **Cron KV cleanup** — Automated retention policy enforcement via Cloudflare Workers cron trigger
- **Organization management** — Multi-org CRUD with KV store (SuperAdmin)
- **Master contacts** — Deduplicated cross-event contact sheet with sync to Google Sheets
- **On-chain event indexer** — Helius webhook + RPC poller for escrow event timeline
- **Individual quiz CRUD** — Per-question add/edit/delete/toggle (Issue 034 Phase 2)
- **Rate limiting** — In-memory middleware for API protection
- **Privacy policy page** — PDPA-compliant `/privacy` route

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
| `attendee` | Register for events, view own registrations, deposit, claim NFT |
| _(unauthenticated)_ | View landing page, public event pages, play adventure, take quiz |

## 🏆 What's Built (Devnet-Validated)

✅ 10 phases complete — from check-in to escrow to attendee identity, on Solana with real wallets.

> **Deployment split (as of 2026-08):** cNFT **badges mint on mainnet** (Crossmint) — the attendance proof is real, on-chain, and DAS-readable by any dApp. The **USDC escrow/deposit program is still on devnet** (`C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`); mainnet USDC is gated on the go/no-go checklist in [`docs/escrow-audit-2026-08-13.md`](docs/escrow-audit-2026-08-13.md) (program deploy → Squads multisig → external audit → canary).

| Core Flow | Status | Details |
|-----------|--------|----------|
| QR check-in | ✅ | Camera scan + manual lookup, staff logging |
| cNFT badges | ✅ **mainnet** | Compressed NFTs via Crossmint hosted mint (airdropped free to attendees), idempotent, PNG badge |
| Quiz gating | ✅ | Per-event quiz before NFT claim |
| Adventure gating | ✅ | Rust-themed educational game (10 levels) |
| Multi-event | ✅ | KV registry, 4-tier roles (super_admin/organizer/staff/attendee) |
| USDC escrow | ✅ | PDA-based deposits, refund, claim forfeited |
| Dual-track payments | ✅ | USDC (on-chain) + PromptPay THB (fiat QR) |
| Wallet adapter | ✅ | Phantom, Solflare, Backpack, Coinbase |
| Attendee identity | ✅ | Google Sign-In for registration, email locked to JWT |
| Self-registration | ✅ | Public event page `/e/{slug}` with countdown + deposit CTA |
| My Registrations | ✅ | Landing page auth-aware nav, event status tracking |
| Landing page UX | ✅ | Persona toggle, synced tabbed timelines (4-step), 4-col card grid, sandbox empty state |
| Walk-in management | ✅ | On-the-spot registration, CSV export, Sheet sync |
| Event cancellation | ✅ | THB batch refund, USDC refund queue, status tracking |
| Wallet error recovery | ✅ | Structured error classification + user-friendly guidance |
| Rollover deposits | ✅ | Atomic on-chain deposit transfer to next event (same organizer) |
| D1 claim locks | ✅ | Atomic double-claim prevention via Cloudflare D1 SQLite |
| R2 asset storage | ✅ | Zero-egress slip images, refund proofs, badge SVGs |
| Cron cleanup | ✅ | Automated KV retention enforcement (Cloudflare Workers cron) |
| Organization management | ✅ | Multi-org CRUD (SuperAdmin) |
| Master contacts | ✅ | Deduplicated cross-event contact sheet + Google Sheet sync |
| On-chain indexer | ✅ | Helius webhook + RPC poller for escrow event timeline |
| Privacy policy | ✅ | PDPA-compliant `/privacy` route |
| Security audit | ✅ | 15 findings, 12 fixed, SEC-001–015 addressed |
| E2E tests | ✅ | 250 tests (54 on-chain + 73 domain + 123 worker) + 147 frontend specs + 16 Kani harnesses, 7 devnet E2E scripts |

## 📈 Competitive Landscape

| Feature | BeThere | Luma | Eventbrite | POAP | Kickback* |
|---------|---------|------|------------|------|-----------|
| On-chain deposits | ✅ USDC escrow | ❌ | ❌ | ❌ | ✅ ETH (defunct) |
| Attendance NFTs | ✅ cNFT | ❌ | ❌ | ✅ (Gnosis) | ❌ |
| Deposit refund | ✅ Auto | ❌ | Manual | ❌ | ✅ Payout pool |
| No-show penalty | ✅ Forfeit to org | ❌ | ❌ | ❌ | ✅ Pool split |
| Quiz/Adventure gating | ✅ Built-in | ❌ | ❌ | ❌ | ❌ |
| Cost per NFT | **$0.001** | N/A | N/A | ~$0.05–0.20 | N/A |
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
| **10.5** | **PDPA Compliance** — consent checkbox + photo consent + privacy policy + deletion API | 📋 Pre-mainnet ([Issue 043](.issues/043_pdpa_consent_data_collection.md)) |
| **11** | Platform fees (1-2% on forfeited deposits) | 📋 Planned |
| **12** | Multi-organizer SaaS | 📋 Planned |
| **13** | **Solana Mobile** — MWA Web + PWA + dApp Store listing (Android) | 📋 Planned ([Issue 042](.issues/042_solana_mobile_support.md)) |
| **14** | Learning & Credentials — reposition adventure + quiz + cNFT as micro-credential system; add credit tracking, stackable certificates | 🔮 Future |
| **15** | Curriculum Design — OBE framework, modular learning units, TQF/AUN-QA compliance, RPL assessment | 🔮 Future |

See **[DISCUSSION.md](./DISCUSSION.md)** for the full architecture direction and decisions.
See **[.issues/038_curriculum_design_vision.md](./.issues/038_curriculum_design_vision.md)** for the curriculum design vision.