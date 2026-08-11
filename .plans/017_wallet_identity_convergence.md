# Plan 017 — Wallet ↔ Email Identity Convergence

> **Status**: scoped — not started. Design doc for review before implementation.
> **Type**: feature (auth/identity) — additive; no breaking change to Google or existing wallet-bind paths.
> **Priority**: P1 — wallet-first users currently create orphaned `wallet:<address>` identities.
> **Depends on**: SIWS verify + wallet-bind (shipped 2026-08-11, commits `ab1988c`, `d0aec11`, `5d4286e3`). Continuation of [Plan 006](006_siws_hybrid_auth.md).
> **Decision**: Converge at reservation — wallet login is standalone, but reserving a spot requires an email and merges wallet→email into one identity.
> **Created**: 2026-08-11

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

## 5. Edge cases

1. **Typed email already belongs to a Google account (theirs).** Desired: merge — bind wallet to that email, reservation joins their existing identity. Risk: someone binds a wallet to an email they don't control. Mitigation options (pick in impl):
   - (a) Only allow binding to an email that has no verified Google login yet (first-writer-wins), OR
   - (b) require email verification (OTP) before binding to a pre-existing email. **Recommend (b) for pre-existing emails, (a)-style silent bind only when the email is brand new.**
2. **Typed email belongs to someone else.** Same mitigation as #1 — never silently attach a wallet to a stranger's established account.
3. **Wallet already bound to a *different* email**, user types a new one. Reject with a clear message ("this wallet is already linked to a****@…"), offer unlink on profile.
4. **Two wallets, one email.** Allowed? Current schema: `developer_profiles.wallet_address` is single-valued, so one wallet per email. Binding a second wallet overwrites. Decide: keep single (simplest) — document it.
5. **User abandons the reservation after typing email but before submit.** No bind happens (bind is on successful submit only). Safe.
6. **Logout/again.** After convergence, wallet login → resolves to email → all good. Before convergence, a wallet-only session that never reserved stays `wallet:<address>` (harmless, no data filed).

---

## 6. Non-goals

- No forced Google-link on wallet login (that was the rejected "force link" option).
- No multi-wallet-per-account support (single wallet per email for now).
- No migration of existing `wallet:<address>` rows (none of substance exist; verify in D1 before shipping).
- No change to staff/organizer auth.

---

## 7. Open questions for review

1. Edge-case #1/#2 mitigation: OK to require **OTP email verification** when a wallet-only user reserves with a **pre-existing** email, and silent-bind only for brand-new emails? (Adds an email-send dependency.)
2. Friendly wallet label format: `0xAB…dead` (4+4) or ENS-style/none?
3. Should Google users' reservations also auto-persist a **connected** wallet (scenario A), or leave wallet purely opt-in via the profile bind button?
