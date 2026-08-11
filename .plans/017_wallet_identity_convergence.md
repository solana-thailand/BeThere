# Plan 017 — Wallet ↔ Email Identity Convergence

> **Status**: scoped — not started. Design doc for review before implementation.
> **Type**: feature (auth/identity) — additive; no breaking change to Google or existing wallet-bind paths.
> **Priority**: P1 — wallet-first users currently create orphaned `wallet:<address>` identities.
> **Depends on**: SIWS verify + wallet-bind (shipped 2026-08-11, commits `ab1988c`, `d0aec11`, `5d4286e3`). Continuation of [Plan 006](006_siws_hybrid_auth.md).
> **Decision**: Converge at reservation — wallet login is standalone, but reserving a spot requires an email and merges wallet→email into one identity. **Reservation and binding are decoupled** (see §5); no OTP in v1.
> **Created**: 2026-08-11
> **Resolved**: 2026-08-11 — decisions in §5/§7 below.

---

## 1. Context — how identity works today

Everything is keyed by the **`email` claim** in the session JWT. The two login paths fill it differently:

- **Google login** → `email` = real verified Google address.
- **Wallet login** (`wallet_verify`) → `find_email_by_wallet(address)`:
  1. `developer_profiles.wallet_address` match (a previously **bound** wallet) → that email, else
  2. `attendees.wallet_address` fallback (rarely set — self-registration doesn't write it), else
  3. **synthetic identity `wallet:<address>`.**

### The problem

1. **Orphaned identities.** A wallet never bound to an email logs in as `wallet:<address>` — a separate pseudo-person with no name/email/history. The same human using Google has a *different* profile.
2. **Reservations under fake emails.** Registration files the attendee under the session email. Wallet-only → filed under `wallet:<address>` — no real email to contact, no name.
3. **Registration doesn't capture the wallet** (`upsert_attendee` ignores `wallet_address`), so registering via Google never links a wallet for future wallet logins.
4. **Ugly UI.** Wallet-only sessions render `👤 wallet:<address>`.
5. **Login ≠ reserve** is invisible to users — they log in and think they're registered.

---

## 2. Chosen model — converge at reservation

A real reservation needs an email anyway (events must contact attendees). So the reservation step is the natural place to merge identities.

**Principles**
- Wallet login remains a valid standalone session (no forced Google link up front).
- The **first time a wallet-only user reserves a spot**, they supply an email; the backend **binds wallet→email** and files the reservation under the *email*, not `wallet:<address>`.
- After convergence, future wallet logins resolve straight to the email (via existing `find_email_by_wallet` step 1).
- Wallet sessions render a friendly identity, never the raw `wallet:<address>` string.

---

## 3. Scenarios (target behavior)

| # | Scenario | Session identity | Reserve behavior |
|---|---|---|---|
| A | Google login | real email | form pre-fills email (read-only); files under email. **Also persists wallet if one is connected.** |
| B | Wallet login, wallet already bound | real email (resolved) | same as A — email known, pre-filled |
| C | **Wallet login, new wallet** | `wallet:<address>` (friendly UI) | form **requires** an email; on submit → bind wallet→email, file under email, session continues |
| D | Google user connects wallet on profile | real email | existing verified bind flow (shipped) |

Edge within C: the email the user types **already exists** (their own Google account, or someone else's) — see §5.

---

## 4. Changes required

### 4.1 Backend

- **`upsert_attendee`** — add `wallet_address` param; write it when the session has a wallet (`claims.sub` holds the wallet address for wallet sessions). Fixes gap #3 for all scenarios.
- **Registration handler (`register_attendee`)** — when `claims.email` starts with `wallet:` :
  - require a non-empty, valid `email` field in the request body (currently email comes only from the JWT),
  - bind `wallet(claims.sub) → email` via `link_wallet_to_email` (reuse; already verified-ownership at login),
  - file the attendee/contact under the supplied email, not the synthetic one.
- **Identity helper** — a small `is_wallet_identity(email)` = `email.starts_with("wallet:")` used by handlers and surfaced in `/api/auth/me` as a boolean (e.g. `wallet_only: true`) plus `wallet_address` so the frontend can render a friendly label without string-parsing.

### 4.2 Frontend

- **`/api/auth/me` consumers** — when `wallet_only`, display shortened address (`0xAB…dead` style) instead of the email. Affects slug header (`👤 …`) and profile page email row.
- **Registration form** — when `wallet_only`, show an **email input** (required) with copy explaining it's how the organizer reaches them; otherwise keep the current read-only pre-filled email.
- **Slug page** — make "logged in" vs "reserve your spot" visibly two steps (minor copy/CTA change) so users don't think login = reserved.

### 4.3 Data

- No schema change — `attendees.wallet_address` and `developer_profiles.wallet_address` already exist (per Plan 006). Only write paths change.

---

## 5. Edge cases — RESOLVED: decouple reservation from binding

**Core rule:** *filing a reservation under an email* (RSVP contact info, zero risk) is separate from *binding wallet→email* (grants future login-as-that-email, high risk). At reservation the user has proven wallet control (signed in) but NOT email control (just typed it), so we only bind when it's safe.

- **Reservation** is always filed under the typed email.
- **Bind wallet→email happens only when the email is brand-new** — no existing `contacts` / `attendees` / `developer_profiles` row. Fresh email ⇒ safe to attach the proven wallet.
- **Email already exists** ⇒ file the reservation but **skip the bind**; return a flag so the UI says "This email already has an account — sign in and link your wallet from your profile" (routes to the shipped, ownership-verified profile bind flow).

Concrete cases:
1. **Typed email is their own existing Google account** → reservation filed under it; no auto-bind; prompt to link via profile. (They log in with Google there, proving ownership, then bind.)
2. **Typed email belongs to someone else (existing)** → same: no auto-bind. Stranger's account never gets a silent wallet attached.
3. **Wallet already bound to a *different* email**, user types a new one → reject at login-resolution time already (login resolves the wallet to its bound email via `find_email_by_wallet`), so this can't reach reservation as wallet-only. If it somehow does, reject the bind with "this wallet is already linked".
4. **Two wallets, one email** → `developer_profiles.wallet_address` is single-valued: one wallet per email; re-binding overwrites. Keep single for v1.
5. **Abandon before submit** → no bind (bind only on successful reservation). Safe.
6. **Residual: brand-new-email squatting** — an attacker could bind their wallet to an email they don't own but that no one has registered yet; if the real owner later signs in with Google, the pre-bound wallet is attached. Low severity for an event app (no financial control — deposits are separate on-chain signatures). Accepted for v1. Clean upgrade if ever needed: OTP-verify the email before binding. **Not built now** (no transactional-email infra configured).

---

## 6. Non-goals

- No forced Google-link on wallet login (that was the rejected "force link" option).
- No multi-wallet-per-account support (single wallet per email for now).
- No migration of existing `wallet:<address>` rows (none of substance exist; verify in D1 before shipping).
- No change to staff/organizer auth.

---

## 7. Decisions (resolved 2026-08-11)

1. **No OTP.** Decouple reservation from binding (§5): auto-bind only to brand-new emails; existing emails reserve without binding and are prompted to link via profile. Avoids email-send infra; residual squatting risk accepted for v1.
2. **Label format:** Solana base58, first-4…last-4, **no `0x` prefix** — e.g. `7Xk9…Qm3p`. Copy button yields the full address. (The doc's earlier `0x…` was an Ethereum-ism; corrected.)
3. **Google users:** wallet stays **opt-in** via the profile "Connect Wallet" button (already shipped, ownership-verified). Reservation persists a wallet on the attendee row **only for wallet sessions** (`claims.sub` = the address); Google sessions carry no wallet, so nothing to persist.
