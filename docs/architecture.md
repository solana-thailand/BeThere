# BeThere — System Architecture

BeThere is an event check-in + proof-of-attendance platform: attendees register,
optionally back their spot with a refundable deposit (USDC on-chain or THB
PromptPay), check in at the event, and claim a compressed-NFT (cNFT) badge. It
runs as a single **Cloudflare Worker** (Rust → WASM) serving both a JSON API and
the **Leptos WASM** single-page frontend.

This is a navigable map — file/dir pointers over exhaustive detail.

---

## 1. Crates (workspace)

| Crate | Path | Role |
|---|---|---|
| `worker` | `worker/` | The Cloudflare Worker: Axum router, handlers, storage layers, Solana/Sheets/Crossmint integrations, the Durable Object. Compiles to a WASM cdylib. Entry: `worker/src/lib.rs`. |
| `event-checkin-domain` (`domain`) | `domain/` | Shared, JS-free domain types: models (`models/`), config (`config/`), wire protocol (`wire.rs`), QR generation. Depended on by both worker and frontend, so it stays `Send + Sync` pure-Rust. |
| `frontend-leptos` | `frontend-leptos/` | Leptos client-side-rendered SPA (`trunk build`). Pages in `pages/`, API client in `api/`, wallet interop in `wallet*.rs` + `js/`. |
| `bethere-escrow` | `bethere-escrow/` | The on-chain Solana escrow program (Quasar/Pinocchio-style). Instructions in `src/instructions/`. |
| `flow-harness` | `flow-harness/` | Test/automation harness. |

The frontend is embedded into the worker at build time via
`include_str!("../../frontend-leptos/dist/index.html")` (`lib.rs:45`) and served
as an SPA fallback for any non-`/api/` route (`lib.rs:78`, `spa_fallback`).

---

## 2. Storage layers

Four layers, with a deliberate source-of-truth split:

| Layer | Binding | Used for |
|---|---|---|
| **Google Sheets** | service account | Human-readable master for attendees + contacts. Organizers live in the sheet. `worker/src/sheets/` (reads, `write/`, `bg_sync/`). |
| **D1** (SQLite) | `DB` | Structured mirror + source of truth for money-critical rows: `thb_deposits`, `contacts` (incl. rolling credit), `attendees`, claim locks, audit, escrow index, campaigns, developers. `worker/src/db/`. |
| **KV** | `EVENTS`, `QUIZ` | Event registry + per-event config cache, deposit-status mirror, SIWS nonces, claim locks, session progress. `worker/src/event_store/`, `storage.rs`. |
| **Durable Objects** | `EVENT_DO` | Serialized ACID writes (check-in, claim-lock finalize) per event; syncs the changed row back to D1. `worker/src/durable_objects/event_do/`. |
| **R2** | `ASSETS_BUCKET` | NFT badges, posters, THB slip images, refund proofs. `worker/src/storage.rs`. |

**Write pattern (recurring):** write D1 first (source of truth) or the DO for
ACID, then **detach** the Google Sheets write via `ctx.wait_until(...)` so the
HTTP response returns immediately (`state.worker_ctx`, `state.rs:74`). Reads are
typically **D1-first with KV/Sheets fallback** (`*_with_fallback` in
`event_store/`). THB deposits are D1-exclusive as the settleable record
(`db/thb_deposits.rs`).

Bindings and parsed config are cached once per isolate behind `OnceLock`
(`state.rs`, `CachedBindings` / `CACHED_CONFIG`).

---

## 3. Main flows

```
register ──▶ (deposit) ──▶ check-in ──▶ claim ──▶ (refund / rollover / credit)
```

- **Register** — `handlers/register/` (`signup.rs` is the core). JWT-gated
  self-registration: validate → resolve event by slug (KV→D1) → dedup → enforce
  capacity → **auto-apply rolling credit if available** → fatal D1 attendee write
  → detached Sheets append → return `next_step`.
- **Deposit** — `handlers/deposit/` split into `usdc/`, `thb/`, `escrow/`. See
  [`deposit-refund-flows.md`](./deposit-refund-flows.md).
- **Check-in** — `handlers/checkin.rs` (staff scans QR) + `walkin.rs` (on-the-spot
  registration). Writes route through the Durable Object for ACID
  (`durable_objects/event_do/checkin.rs`). NFC path: `checkin/nfc/verify`.
- **Claim** — `handlers/claim.rs` + `claim/` (`lookup` → gates: quiz / adventure
  / check-in → per-event lock → mint). Mint via Crossmint (`claim/mint.rs:786`).
- **Refund / rollover / credit** — USDC on-chain (attendee-signed refund+close,
  or rollover to next event); THB hold-as-credit / cash refund. Full detail in
  the deposit-refund doc.

Post-event: event summary freeze, public recap, PR pack
(`handlers/events.rs`, Plan 008).

---

## 4. External integrations

| Integration | Where | Purpose |
|---|---|---|
| **Google Sheets API** | `worker/src/sheets/` | Attendee/contact master; service-account JWT auth (`state.rs:115` `GoogleServiceAccountConfig`). |
| **Google OAuth** | `handlers/auth.rs` (`auth_url`, `auth_callback`) | Sign-in → verified email → session JWT. |
| **Solana escrow program** | `worker/src/solana_escrow/` builds TXs; `bethere-escrow/` is the program | USDC deposit/refund/close/rollover/checkin/forfeit. Instructions keyed by discriminator 0–8 (`escrow_indexer/mod.rs`, `EscrowInstruction`). |
| **Helius** | `state.rs:163` (`HELIUS_RPC_URL` + `HELIUS_API_KEY`) | RPC for TX build/verify; DAS API for reading wallet NFT inventories + leaderboard (`handlers/wallet.rs`); enhanced webhooks + RPC poller for on-chain event indexing (`escrow_indexer/webhook.rs`, `poller.rs`). |
| **Crossmint** | `claim/mint.rs`, `solana.rs` (`mint_compressed_nft`) | Custodial minting of the compressed proof-of-attendance NFT (tree + fees + signer). Host/cluster selected from the Helius cluster; `staging.crossmint.com` on devnet (`state.rs:168`). |

On-chain events flow back in via the **escrow indexer**: Helius webhook
(`POST /api/escrow/onchain-webhook`, public) or RPC poller → parsed
(`parse_helius_transaction`) → stored in D1 (`escrow_indexer/store.rs`) →
consumed by e.g. `claim_forfeited` candidate filtering.

---

## 5. Auth model

`worker/src/auth.rs` + `handlers/auth.rs`. All sessions are **JWT**
(`create_session_jwt` / `verify_session_jwt`), carried as a bearer/cookie and
checked by middleware.

- **Google OAuth** — `GET /api/auth/url` → `GET /api/auth/callback` mints a
  session JWT from a verified Google email.
- **SIWS (Sign-In With Solana)** — `POST /api/auth/wallet/nonce` issues a signed
  challenge stored in KV (`siws_msg_<addr>`); `POST /api/auth/wallet/verify`
  checks the ed25519 signature (`solana::verify_siws_signature`) and issues a
  JWT with a synthetic `wallet:<address>` identity. `POST /api/auth/wallet/bind`
  binds a proven wallet to an existing account. Wallet↔email convergence is
  Plan 017 (see `signup.rs` credit-identity gate).
- **Social linking** — GitHub (`/auth/github` start, public
  `/auth/github/callback`) and Telegram (`/auth/telegram/*`, identity via signed
  state so it survives the cross-site redirect). `handlers/social_link.rs`.
- **Roles** — `resolve_user_role` / `check_event_access` (`auth.rs:506`) resolve
  in order: **super-admin** (env `SUPER_ADMIN_EMAILS`) → event organizer → Sheet
  organizer → event staff → Sheet staff. `is_staff` combines the env allowlist
  (`STAFF_EMAILS`) with the Sheets "staff" tab.
- **Dev mode** — `DEV_MODE=1` bypasses JWT verification (accepts `dev-token`);
  refused on the live production domain (`state.rs:242`).

JWT revocation: `db/jwt_blacklist.rs` (SHA-256 keyed, VULN-007).

---

## 6. Request routing

`worker/src/handlers/mod.rs` — everything is nested under `/api` and composed
from four router groups by increasing privilege
(`routes()`, `mod.rs:37`, merged at `:536`):

| Router | Guard | Examples |
|---|---|---|
| **Public** (`public`, `:104`) | none (Cloudflare rate-limits) | health, public event list/detail, auth endpoints, claim/quiz/adventure play, `deposit/usdc/tx`, `deposit/usdc/webhook`, `escrow/refund`, `escrow/close-deposit`, `escrow/onchain-webhook`, badge/poster serving, wallet NFT reads, `deposit/status/{id}`. |
| **Attendee-authed** (`attendee_authed`, `:178`) | `require_identity` — valid JWT, verified email, **not** necessarily staff | `public/register`, `my-registration(s)`, `deposit/usdc`(+`/confirm`), `deposit/thb/upload`, `deposit/hold`, `deposit/credit-*`, `escrow/rollover-deposit`, `my-profile`, social linking, privacy self-service. |
| **Staff/Protected** (`protected`, `:288`) | `require_auth` — valid JWT **and** staff | attendee list/CRUD, `checkin/{id}`, walk-ins, quiz/adventure admin, event CRUD, `deposit/thb/verify` + `admin-upload`, refund queue / `refund/mark` / `batch-thb` / `hold`, escrow lifecycle (`init`, `mark-checked-in`, `deactivate`, `close-event`, `claim-forfeited`), contacts, orgs, campaigns, community insights. |
| **Live dashboard** (`protected_no_store`, `:283`) | `require_auth` + `no-store` | `dashboard/live` (polled every 2.5s). |

There is no separate "admin" *router*: **admin/super-admin gating is
per-handler**, done inside protected handlers via `resolve_event_with_access` /
`resolve_user_role` / `check_event_access` (`auth.rs`), keyed on
`SUPER_ADMIN_EMAILS` and per-event organizer assignment. Cache policy is applied
per sub-router (`middleware/cache.rs`): 60s public list, 120s public detail,
`no-store` for user-specific/auth/dashboard, `no-cache` for health.

Global middleware stack (outermost → in, `lib.rs:134`): security headers →
correlation id → rate limit → the router. Cron cleanup runs daily via
`#[event(scheduled)]` (`lib.rs:154`, `cleanup.rs`).

---

## 7. Where to look first

- **A new API endpoint** → `handlers/mod.rs` (pick the router), then the handler
  module under `handlers/`.
- **A storage question** → `event_store/` (KV + fallback logic), `db/` (D1),
  `durable_objects/event_do/` (ACID), `sheets/` (master + bg sync).
- **Money / deposits** → [`deposit-refund-flows.md`](./deposit-refund-flows.md)
  and `db/thb_deposits.rs` (the CAS invariants).
- **On-chain** → `solana_escrow/` (TX builders), `bethere-escrow/src/instructions/`
  (the program), `escrow_indexer/` (events back in).
- **Config / secrets / bindings** → `state.rs` (`build_config`, `from_env`) and
  `domain/src/config/`.
