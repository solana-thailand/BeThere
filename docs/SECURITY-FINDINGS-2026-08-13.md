# Security Findings — 2026-08-13 (develop branch)

Read-only security review of this session's money/auth/claim changes
(`git diff origin/main...develop`). **These are FLAGGED for review, not yet
fixed** — they touch auth / on-chain escrow / credit-ledger logic, which was held
back from autonomous overnight changes. Each has an exact recommended fix.

Severity order. Items marked "fixed this session" were safe enough to patch
(commit-only, see the referenced commits); everything else awaits your review.

---

## 1. HIGH — Wallet-session registration leaks another attendee's `claim_token` + PII (IDOR)

**File:** `worker/src/handlers/register/signup.rs` — identity resolution (~L45-58)
and the duplicate-return branch (~L202-282). **CONFIRMED.**

**What:** For a "wallet session" (JWT `email` claim = synthetic `wallet:<addr>`,
issued by `wallet_verify` for any wallet not yet bound to an email), the register
handler derives the dedup email from the **request body**. On a duplicate hit it
returns the existing attendee's `claim_token`, `attendee_id`, `name`, and `email`.

**Exploit:** Attacker generates a throwaway keypair → `/api/auth/wallet/nonce` +
`/verify` (free, self-signed) → `POST /api/public/register` with
`{ slug, email: "victim@example.com", name, consent_given: true }`. Response hands
back the victim's `claim_token` + `api_id`. With the token they can read the
victim's claim (`GET /api/claim/{token}`) and, if the attendee has no column-P
lock and no linked profile wallet, **mint the victim's badge to their own wallet**
(`POST /api/claim/{token}`). The `credit_identity_ok` gate protects rolling credit
but runs *after and independent of* this leak.

**Fix:** In the duplicate-return branch, when `is_wallet_session &&
!credit_identity_ok` (email ownership unproven), do NOT return
`claim_token`/`api_id`/`name` — return a generic "already registered — sign in
with that email to see your ticket" response. Treat a typed email on an unproven
wallet session as un-owned for every read path, not just credit.

**Note:** legit users are unaffected — a real returning user has a bound email, so
their JWT is a real email (not a `wallet:` session). Low blast-radius fix.

---

## 2. MEDIUM — Escrow refund/close: refundability gate keyed to `attendee_id`, TX built from a different `wallet_address`

**File:** `worker/src/handlers/deposit/escrow/handlers.rs` —
`refund_and_close_tx_handler` (~L167-264); same shape in `close_deposit_tx_handler`
and `mark_checked_in_tx_handler`. Public endpoints. **PLAUSIBLE** (depends on
on-chain enforcement).

**What:** `body.attendee_id` supplies the off-chain `refundable` check;
`body.wallet_address` derives the on-chain deposit PDA. They're never cross-checked.
An overflow-tier (non-refundable) depositor can pass *someone else's*
verified+refundable `attendee_id` with *their own* `wallet_address`; the gate
passes and a refund TX for their own deposit is built. If the on-chain program
doesn't independently enforce the tier (it appears off-chain only), the attacker
reclaims a deposit the organizer expected to forfeit. Funds only ever return to
the depositor's own wallet (not third-party theft) — the loss is organizer
forfeiture revenue.

**Fix:** After loading the deposit for `attendee_id`, require
`status.wallet_address.eq_ignore_ascii_case(&body.wallet_address)` before building
any refund/close TX. Apply to all three handlers.

---

## 3. MEDIUM — Registration spoofing under an arbitrary, unproven email

**File:** `worker/src/handlers/register/signup.rs` (~L47-58, L343-377). **CONFIRMED**
(same root cause as #1).

**What:** When the victim is *not yet* registered, the wallet-session-typed-email
path files a **new** attendee row under `victim@example.com` and returns a fresh
`claim_token` to the attacker — poisoning the victim's identity in that event
(capacity consumption, a reservation they never made, a token bound to their
email). Credit is not spent (gated), but the reservation/token are real.

**Fix:** For wallet sessions where `!credit_identity_ok`, reject filing under an
email that has any prior account the wallet doesn't own; require email
verification (or wallet-email binding) before reserving under a non-owned address.

---

## 4. MEDIUM — Rolling-credit double-spend across concurrent registrations *(FIXED this session)*

**File:** `worker/src/handlers/register/signup.rs` (read ~L302-321, decrement
~L387-413). **PLAUSIBLE → mitigated.**

**Fix applied (serialization, not the D1-authoritative rewrite below):** the credit
spend now takes a short **per-email D1 advisory lock** (`credit-spend:<email>`,
`db::advisory_locks`, migration `0027`) and **re-reads the balance under the lock**
before decrementing. Two concurrent registrations by the same email serialize; the
loser (can't acquire the lock, or finds the credit already spent) falls back to
normal payment and keeps its credit. This closes the race without changing the
credit source of truth, so it carries none of the divergence risk of the
D1-authoritative approach — which is left below as an optional future cleanup, no
longer required for correctness.

**What:** Credit is read (`get_credit_balance`) then decremented
(`decrement_credit`) against the Google **Contacts Sheet**, which has no
atomic compare-and-set. The decrement-before-mark ordering stops *retry* inflation,
but two concurrent registrations by the same verified email (two events, balance
100, each needs 100) can both read `>= required` and both decrement → two
credit-covered deposits from one balance.

**Fix (refined — NOT applied, needs a dedicated reviewed PR):** The D1 `contacts`
table *already* carries `deposit_credit_thb`/`deposit_credit_usdc` (migration
0002), so no new migration is needed — but the spend path in `signup.rs` reads and
decrements the **Google Sheet**, not D1. The fix is to make **D1 authoritative for
credit-spend**: add `try_decrement_credit` in `db/contacts.rs` doing a conditional
`UPDATE contacts SET deposit_credit_thb = deposit_credit_thb - ?1
WHERE lower(email) = ?2 AND deposit_credit_thb >= ?1` and consume only when
`meta().changes > 0` (mirrors `try_settle_hold_credit`); spend via that CAS FIRST,
then best-effort mirror the decrement to the Sheet for display.

**Why this was flagged, not auto-applied — CONFIRMED blocker:** the D1 credit
columns exist but are **never written in production**. The only writer,
`db::contacts::update_deposit_credit`, has **zero callers**; every real credit
increment (`hold_credit.rs` → `sheets::contacts::increment_credit`), read
(`get_credit_balance`), and decrement goes to the **Google Sheet**. So D1 credit is
always the `0` default — a D1-CAS spend gate would reject *all* credit and break the
feature. The prerequisite is a dedicated PR that makes D1 authoritative: dual-write
every increment/decrement to D1, backfill balances from the Sheet, then add the CAS
gate. This is real work on a money ledger and must be reviewed — not an overnight
bolt-on. Exploit remains narrow (same verified email registering for two events
*simultaneously*).

---

## 5. LOW — Crossmint double-mint edge on delayed confirmation *(partially mitigated this session)*

**File:** `worker/src/solana.rs`, `worker/src/claim/mint.rs`. **PLAUSIBLE (narrow).**

**What:** On poll timeout the claim lock releases (retry allowed) and the
idempotency marker `crossmint:pending:<token>` lets an *in-window* retry resume the
same NFT. But if the original submission confirms *after* the poll budget AND the
attendee retries *after* the marker TTL, a second mint fires (duplicate cNFT; no
money).

**Mitigation applied:** marker TTL raised 1h → 24h (commit `test/security` batch),
shrinking the window. **Fuller fix (flagged):** gate re-mint on the finalized
claim-lock / `claimed_at` record rather than solely the KV marker.

---

## 6. LOW — Raw SQL string interpolation for social handles *(FIXED this session)*

**File:** `worker/src/handlers/social_link.rs`. Was: values escaped via
`replace('\'', "''")` and interpolated into single-quoted SQLite literals — not
exploitable, but fragile.

**Fixed:** all four sites (GitHub link, both Telegram saves, social unlink) now use
parameterized `bind_refs`/`?n` placeholders — no attacker-controlled profile text
touches SQL text. The duplicated Telegram INSERT was also collapsed onto the shared
`save_telegram_link` helper.

---

## 7. LOW — Non-locked, non-linked pre-registered claim mints to any client wallet (by design)

**File:** `worker/src/claim/mint.rs` (recipient precedence). For a pre-registered
attendee with neither a column-P lock nor a linked profile wallet, the claim
token alone lets the holder direct the mint to any wallet. Intended bearer-token
model, but it's what makes the #1 token leak impactful.

**Fix (optional):** when the attendee HAS a linked profile wallet, prefer the
server-resolved linked wallet and treat an explicit override as a last resort.

---

## 8. MEDIUM — Wallet could bind to multiple emails; resolution was nondeterministic *(fixed this session)*

**File:** `worker/src/db/contacts.rs` (`link_wallet_to_email`, `find_email_by_wallet`),
`worker/src/handlers/auth.rs` (`wallet_bind`). **CONFIRMED.**

**What:** `link_wallet_to_email` upserts on **email** (not wallet), and there's no
uniqueness on `developer_profiles.wallet_address` — so two different emails could
each bind the **same** wallet. `find_email_by_wallet` then did `LIMIT 1` with **no
ORDER BY**, so wallet-login and `credit_identity_ok` resolved to an *arbitrary* one
of the bound emails.

**Not a theft vector:** binding requires proving BOTH wallet ownership (SIWS) and
email ownership (session), so you can only bind wallets you own to emails you own —
an attacker can neither bind their wallet to a victim's email nor bind a victim's
wallet. But it's an identity-integrity bug: logging in with a shared wallet could
land you in either account nondeterministically, and the credit gate could
false-negative.

**Fixed:** `wallet_bind` now refuses to bind a wallet already linked to a
*different* account (exclusive binding; re-binding the same email is idempotent),
via a new bindings-only `find_bound_email_by_wallet` (developer_profiles only — a
badge-mint recipient wallet is not an identity binding). `find_email_by_wallet` now
orders by `updated_at DESC` so any legacy multi-binding resolves deterministically
(most recent wins).

**Residual (flagged):** existing duplicate bindings in prod aren't retro-cleaned,
and the Plan-017 auto-bind path (`signup.rs`, new-email wallet sessions) only runs
for *unbound* wallets so it can't create new duplicates — but a one-off audit for
pre-existing `developer_profiles` rows sharing a `wallet_address` is worth running.

---

## Reviewed and found SAFE (no action)

- **SIWS verify** — real ed25519 `verify_strict` over a server-stored, single-use
  KV challenge; fail-closed; the old `"siws_verified"` bypass is gone + regression-
  tested. `wallet_bind` requires a fresh SIWS proof.
- **THB hold vs refund CAS** (`db/thb_deposits.rs`) — single-statement conditional
  UPDATEs, genuinely mutually exclusive + idempotent; handlers settle-before-credit
  fail-closed.
- **Concurrent claim double-mint** — guarded by the DO/D1 lock
  (`INSERT ... ON CONFLICT DO NOTHING`); finalized lock persists.
- **JWT** — HS256 pinned, constant-time compare, `exp` enforced, blacklist by hash.
- **Cross-event staff scoping** — global `staff` excluded from `check_event_access`.
- **Quiz** — no answers leaked; event_id coalesced from token; exhausted → clean 4xx.
- **Event duplicate** — Organizer+ gate; escrow zeroed; title template reset.

---

## Also flagged (from the docs pass — correctness, not security)

- **Credit-label heuristic mismatch:** `already_deposited.rs` detects rolling
  credit by `"CREDIT"` in tx_signature/wallet, but signup marks auto-applied credit
  with `slip_url = "ROLLING_CREDIT_AUTO_APPLIED"` / `verified_by =
  "SYSTEM_ROLLING_CREDIT"`. *(Fixed this session — cosmetic display.)*
- **Slip size numbers disagree:** doc says 5MB decoded / 7MB encoded, check is 5 MiB
  encoded, user-facing error says 3MB. Pick one and align.
- **CAS fallback returns `Ok(true)` when D1 absent:** double-payout protection only
  holds with D1 configured (always true in prod). Consider fail-closed if D1 is None.
