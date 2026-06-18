# Plan 006 — Sign-in with Solana (Hybrid Auth)

> **Status**: scoped — not started. Blocked on plan 005 (staging worker + flow harness) landing first, because every worker change in this plan is gated by the harness.
> **Type**: feature (auth) — additive, no breaking changes to Google auth path
> **Priority**: P1 (unblocks plan 007 mobile; required for Web3-native identity)
> **Decision**: 1c — hybrid. SIWS becomes the *primary* auth path going forward; Google auth kept as legacy fallback for existing records. No attendee migration required.
> **Dependencies**: plan 005 (staging worker + automated flow harness), plan 007 (consumer)
> **Created**: 2026-06-17

---

## 1. Context

### Current auth model (from `worker/src/auth.rs` + `handlers/auth.rs`)

- **JWT-based sessions**: `create_session_jwt(email, sub, secret)` issues a JWT; `verify_session_jwt(token, secret)` validates. Claims = `{ email, sub }`.
- **Google OAuth path**: `/api/auth/url` → Google consent → `/api/auth/callback` → `create_session_jwt` → `Set-Cookie`.
- **Identity is email-keyed** across the schema:
  - `contacts.email TEXT PRIMARY KEY` (lowercased)
  - `attendees.email` (indexed)
  - `staff.email TEXT PRIMARY KEY` — **already has `wallet_address TEXT` column**
  - `developer_profiles.email TEXT PRIMARY KEY` — **already has `wallet_address TEXT` column**
- **Auth surface used by frontend**: `GET /api/auth/url`, `GET /api/auth/callback`, `GET /api/auth/me`, `POST /api/auth/logout`. All Leptos pages call `/api/auth/me` on mount via `crate::components::require_auth`.

### What SIWS (Sign-In with Solana) is

A wallet-based auth standard: the client constructs a SIWS message, the wallet signs it (Ed25519), the server verifies the signature and issues a session. Replaces the "Google OAuth issues a JWT" path with "wallet signature issues a JWT" — same JWT shape, same downstream session handling.

### Why hybrid (decision 1c)

- Existing attendees are keyed by email and onboarded via Google auth. Forcing wallet-only would orphan their records.
- New Web3-native users should be able to authenticate with just a wallet (no Google account required).
- Hybrid = SIWS as the primary path for new/returning wallet users; Google auth remains as a fallback for legacy email-keyed accounts.
- Account linking: a user who has both (Google account + wallet) can link them — the wallet becomes an alternate credential for the same `contacts`/`attendees` row.

---

## 2. Scope

### In scope

- Backend: SIWS message verification + JWT issuance as an **additive** auth path (new endpoints, Google endpoints untouched).
- Backend: account-linking model — link a wallet to an existing email-keyed account, OR provision a wallet-only account.
- Backend: feature flag (`SIWS_AUTH_ENABLED`, default off) gating the new endpoints in production.
- Frontend: Wallet Standard integration on the web frontend (`@solana/wallet-standard` + Wallet Standard UI) to replace raw `connectWallet`/`signAndSendTransaction` `wasm_bindgen` calls. SIWS message construction + sign flow.
- Frontend: SIWS login button alongside (not replacing) the Google login button.
- Schema: new `wallet_accounts` table (wallet → contact/attendee mapping) and `wallet_links` audit table (link events).
- Staging worker validation before production deploy (per plan 005).

### Out of scope (deferred)

- Migrating existing Google-only users to wallets — no forced migration. Users opt in to linking.
- Removing Google auth — stays indefinitely as legacy fallback.
- Mobile wallet integration (MWA) — that's plan 007. This plan is web-only SIWS, but the backend endpoints are mobile-ready.
- Replacing the existing raw `wasm_bindgen` wallet interop for *deposit/sign tx* flows — that's a separate refactor. This plan only adds SIWS sign-in; the deposit flows keep working as-is.
- Passkey / biometric auth — not Solana-native, defer.

---

## 3. Architecture

### 3.1 Hybrid identity model

```/dev/null/siws-identity.md#L1-30
┌────────────────────────────────────────────────────────────┐
│ contacts (existing, email-keyed)                           │
│   email TEXT PRIMARY KEY                                    │
│   ...                                                       │
├────────────────────────────────────────────────────────────┤
│ wallet_accounts (NEW — wallet-keyed, links to contacts)    │
│   wallet_address  TEXT PRIMARY KEY  (base58, 32 bytes)      │
│   contact_email   TEXT NOT NULL REFERENCES contacts(email)  │
│   linked_at       TEXT NOT NULL                             │
│   link_method     TEXT NOT NULL  ('siws_new' | 'siws_link') │
│   last_used_at    TEXT                                       │
│   revoked_at      TEXT                                       │
│                                                            │
│   INDEX(contact_email)                                      │
├────────────────────────────────────────────────────────────┤
│ wallet_links_audit (NEW — append-only audit log)           │
│   id              INTEGER PRIMARY KEY AUTOINCREMENT         │
│   wallet_address  TEXT NOT NULL                             │
│   contact_email   TEXT NOT NULL                             │
│   action          TEXT NOT NULL  ('link' | 'unlink' | ...)  │
│   performed_at    TEXT NOT NULL                             │
│   ip_hash        TEXT                                       │
│   user_agent_hash TEXT                                      │
└────────────────────────────────────────────────────────────┘
```

Key invariant: **one wallet maps to exactly one contact email**. A contact can have multiple wallets (e.g. Phantom + Solflare). This keeps email-keyed code paths working unchanged — they resolve via `wallet_accounts.contact_email`.

### 3.2 JWT reuse (no new session model)

The new SIWS path issues the **same JWT shape** as Google auth:

```/dev/null/siws-jwt.rs#L1-12
// Existing (Google auth):
create_session_jwt(email, sub = google_sub, secret)
  → claims: { email, sub: "<google_sub>", auth_method: "google" }

// NEW (SIWS):
create_session_jwt(email, sub = wallet_address, secret)
  → claims: { email, sub: "<wallet_address>", auth_method: "siws" }
```

Add an `auth_method` claim so downstream handlers can distinguish if needed (e.g. require Google for staff operations, allow SIWS for attendee operations). Default policy: SIWS is valid anywhere Google is, unless explicitly restricted.

### 3.3 SIWS message format (Solana standard)

Per the Solana SIWS spec (CAIP-122 / SIP-xx), the message contains: domain, issued-at, nonce, statement, chain ID. We construct a nonce server-side (prevents replay), the wallet signs the message + nonce, we verify.

```/dev/null/siws-message.txt#L1-8
bethere.solana-thailand.workers.dev wants you to sign in with your Solana account:
<base58 wallet address>

Statement: Sign in to BeThere

URI: https://bethere.solana-thailand.workers.dev
Chain ID: mainnet-beta
Nonce: <server-issued 16-byte base64>
Issued At: 2026-06-17T12:00:00Z
Expiration Time: 2026-06-17T12:05:00Z  // 5 min validity
```

### 3.4 Endpoint additions (additive — Google endpoints untouched)

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/auth/siws/challenge` | Issue nonce + canonical SIWS message for a given wallet address. Nonce stored in KV with 5min TTL keyed by `wallet:nonce:<addr>`. |
| `POST` | `/api/auth/siws/verify` | Verify signature over signed message; issue JWT; return same response shape as `/api/auth/callback`. |
| `GET` | `/api/auth/siws/account` | For a logged-in session, return linked wallet (if any) and link status. |
| `POST` | `/api/auth/siws/link` | Link a wallet to an existing email-keyed account (requires Google-authenticated session + wallet signature). |
| `POST` | `/api/auth/siws/unlink` | Remove a wallet link (requires auth from either credential). |

All under the `SIWS_AUTH_ENABLED` feature flag.

---

## 4. Implementation

### Phase 1 — Backend SIWS verify + JWT issue (on staging)

**Files**:

- `worker/migrations/0026_wallet_accounts.sql` — new tables (above schema).
- `worker/src/auth/siws.rs` — new module: `verify_siws_signature(message, signature, pubkey) -> Result<(), Error>` using `ed25519-dalek`. Pure crypto, no I/O — unit-testable.
- `worker/src/auth/nonce.rs` — nonce generation, KV storage (5min TTL), single-use enforcement.
- `worker/src/handlers/auth/siws_handlers.rs` — 5 new handlers above. All gated by `env.var("SIWS_AUTH_ENABLED") == "true"` — return 404 if off.
- `worker/src/handlers/mod.rs` — register new routes (additive, do not modify existing `/api/auth/url`, `/api/auth/callback`).
- `worker/src/auth.rs` — extend `create_session_jwt` to accept `auth_method` claim. Existing Google callers pass `auth_method = "google"` — no behavior change.

**Crypto dependency**: `ed25519-dalek = "2.1"` for signature verification. Solana pubkey → ed25519 public key is a 32-byte reinterpretation. Verify on the **canonical SIWS message bytes** (not a hash — Dalek does that internally).

**Nonce storage**: KV (not D1) — TTL is native, no GC needed. Key: `siws:nonce:<addr>`, value: nonce + issued_at + client_ip_hash. Single-use: deleted on consume.

### Phase 2 — Account linking model

- `POST /api/auth/siws/link`: requires a valid existing session (Google-authed) AND a fresh wallet signature over `Link wallet <addr> to <email>`. Creates `wallet_accounts` row, writes `wallet_links_audit`.
- Conflict rules:
  - If wallet already linked to a different email → 409, must unlink first.
  - If wallet already linked to same email → 200, idempotent.
- After link, future SIWS logins with that wallet resolve to the same `contacts` row → same attendee records.

### Phase 3 — Frontend Wallet Standard integration (web)

**Files**:

- `frontend-leptos/package.json` — add `@solana/wallet-standard` and a Wallet Standard React-less adapter (we're Leptos, not React).
- `frontend-leptos/js/wallet_standard.js` — thin JS shim: listens for `registerWallet` events, exposes a `getWallets()` function callable from Rust via `wasm_bindgen`. Replaces the current raw `window.solana` / `window.phantom` probing.
- `frontend-leptos/src/auth/siws.rs` — new module: construct SIWS message, request challenge, request signature via Wallet Standard, submit to `/verify`.
- `frontend-leptos/src/pages/landing.rs` — add "Sign in with Solana" button alongside existing Google button. Both buttons end at the same post-auth state.
- Existing `connectWallet` / `signAndSendTransaction` calls in deposit flows: **untouched** in this plan. Wallet Standard will eventually replace them, but that's a separate refactor — not blocking SIWS.

### Phase 4 — Feature flag rollout

1. Deploy to staging worker with `SIWS_AUTH_ENABLED=true` (per plan 005 staging env).
2. Run plan 005 flow harness on staging — confirm all existing flows pass (Google auth unaffected).
3. Manual SIWS flow on staging (Phantom + Solflare both).
4. Roll to production worker with `SIWS_AUTH_ENABLED=false` initially (no behavior change).
5. Flip flag to `true` via `wrangler secret put SIWS_AUTH_ENABLED` — no redeploy needed.
6. Monitor `/api/auth/siws/*` usage and error rates for 48h before declaring stable.

---

## 5. Testing

### Unit

- `worker/src/auth/siws.rs::verify_siws_signature` — happy path + tampered message + wrong signer + malformed signature + expired nonce.
- `worker/src/auth/nonce.rs` — issue/verify/single-use/expiry.
- Link-conflict resolution (409 on cross-email link, 200 idempotent on re-link).
- JWT carries `auth_method` claim correctly.

### Integration (against staging worker)

- SIWS login → `/api/auth/me` returns session with `auth_method: "siws"`.
- SIWS login → access protected attendee route (`/api/my-registrations`) → works.
- Google login still works unchanged (regression test).
- Link wallet to Google account → SIWS login with same wallet resolves to same contact.
- Unlink wallet → SIWS login fails with 401.
- Replay attack: reuse a nonce → 401.
- Cross-domain attack: SIWS message for wrong domain → 401.

### E2E (browser, staging)

- Phantom + Solflare: SIWS sign-in flow end to end, session persists across reload.
- Mixed flow: Google sign-in, link wallet, sign out, SIWS sign-in with linked wallet, verify same attendee record.

### Plan 005 harness gates

Before **every** worker deploy in this plan, plan 005's automated harness must pass on staging. If it fails, deploy is blocked. No exceptions.

---

## 6. Rollout

- [ ] Phase 1 backend on staging (migration + handlers + flag on).
- [ ] Plan 005 harness green on staging.
- [ ] Phase 2 linking handlers on staging.
- [ ] Phase 3 frontend Wallet Standard on staging (Leptos build + worker deploy).
- [ ] Manual SIWS flow on staging (2 wallets minimum).
- [ ] Deploy production worker with flag OFF (regression check, 24h).
- [ ] Flip flag ON in production.
- [ ] Monitor 48h.
- [ ] Update `.handovers/` with results.

**Rollback**: flip flag back to `false` (no redeploy). SIWS endpoints return 404; existing Google flow unaffected. Linked wallets remain in DB (dormant) until re-enabled.

---

## 7. Files Touched

| File | Change |
|------|--------|
| `worker/migrations/0026_wallet_accounts.sql` | NEW — `wallet_accounts`, `wallet_links_audit` tables |
| `worker/src/auth/siws.rs` | NEW — SIWS signature verification |
| `worker/src/auth/nonce.rs` | NEW — nonce issuance / KV storage / single-use |
| `worker/src/auth.rs` | Extend `create_session_jwt` with `auth_method` claim (additive) |
| `worker/src/handlers/auth/siws_handlers.rs` | NEW — 5 SIWS endpoints |
| `worker/src/handlers/mod.rs` | Register new routes (additive) |
| `frontend-leptos/package.json` | `@solana/wallet-standard` dependency |
| `frontend-leptos/js/wallet_standard.js` | NEW — Wallet Standard shim |
| `frontend-leptos/src/auth/siws.rs` | NEW — SIWS client flow |
| `frontend-leptos/src/pages/landing.rs` | Add SIWS button alongside Google button |
| `worker/wrangler.toml` | Add `[vars] SIWS_AUTH_ENABLED = "false"` (production default) |

Zero changes to:
- `worker/src/handlers/auth.rs` (Google handlers) — untouched.
- `worker/src/auth.rs::auth_callback` — untouched.
- Any existing `/api/auth/*` route behavior.
- `frontend-leptos/src/pages/deposit/*` wallet interop — untouched in this plan.

---

## 8. Acceptance Criteria

- [ ] New user can sign in with Phantom wallet only (no Google account), session persists, attendee record auto-provisions.
- [ ] Existing Google-authed user can link a wallet; subsequent SIWS login resolves to the same attendee record.
- [ ] Unlinking a wallet breaks SIWS login for that wallet; Google login still works.
- [ ] Replay attack (reused nonce) is rejected with 401.
- [ ] Cross-domain SIWS message (wrong domain) is rejected with 401.
- [ ] Google auth flow is byte-identical (regression: same endpoints, same response shapes, same cookie behavior).
- [ ] Plan 005 flow harness passes on staging before AND after each phase.
- [ ] Feature flag OFF in production → all `/api/auth/siws/*` endpoints return 404; zero behavior change visible to existing users.
- [ ] Feature flag ON → SIWS login works on production with the same session semantics as Google auth.

---

## 9. Risks / Notes

- **`ed25519-dalek` on `wasm32-unknown-unknown`**: the worker target. Dalek 2.1 supports `wasm32` but verify with a quick compile-test before committing. Fallback: `solana-sdk` `ed25519_instruction` (uses the Solana VM, but the worker isn't Solana VM — that's not viable). Dalek is the only realistic option; verify it compiles to the worker target in Phase 1 step 1.
- **Nonce replay window**: 5min validity + single-use. If a legitimate user signs slowly (mobile wallet cold start), they might exceed 5min. Make TTL configurable (env var `SIWS_NONCE_TTL_SECONDS`, default 300) and consider 10min.
- **Wallet Standard on Leptos**: Wallet Standard is designed for React. Using it from Leptos requires a JS shim — straightforward but not zero-effort. The shim is small (~100 lines) and contained in `frontend-leptos/js/wallet_standard.js`.
- **Account-linking abuse**: a malicious actor could try to link their wallet to a victim's email account. Mitigation: link requires the victim's existing Google session AND the attacker's wallet signature — both credentials required. Can't link to an unauthenticated session.
- **`staff.email` and `developer_profiles.email` already have `wallet_address` columns** — those are pre-existing (used for NFT minting target). This plan introduces a separate `wallet_accounts` table for auth linking to keep concerns separated. Future work may consolidate, but not in this plan.
- **Cross-event identity**: `contacts.email` is global (one email across all events). `wallet_accounts.wallet_address` should also be global. A user linking the same wallet in two different event contexts should resolve to the same contact. This is handled naturally by the `wallet_address PRIMARY KEY` constraint.
