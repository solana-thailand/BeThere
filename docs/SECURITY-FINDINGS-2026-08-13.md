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

## 4. MEDIUM — Rolling-credit double-spend across concurrent registrations

**File:** `worker/src/handlers/register/signup.rs` (read ~L302-321, decrement
~L387-413). **PLAUSIBLE.**

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

**Why this was flagged, not auto-applied:** credit is currently dual-stored
(Sheet master + D1 mirror) and the two can diverge if any prior increment/decrement
hit only one. Switching the spend gate to D1 without first reconciling the two
stores risks rejecting a legit credit or honoring a stale one — a money bug. This
needs a reconciliation step + review, so it's left as a scoped follow-up. Exploit
is narrow (same verified email registering for two events *simultaneously*).

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

## 6. LOW — Raw SQL string interpolation for social handles

**File:** `worker/src/handlers/social_link.rs` (~L309-322, L474-534, L644-664).
**CONFIRMED not exploitable**, but fragile. Values are escaped via
`replace('\'', "''")` and executed as single-quoted SQLite literals — breakout
isn't achievable, but it's one refactor away from a hole.

**Fix:** Use parameterized `bind`/`?n` placeholders as elsewhere in `db/`.

---

## 7. LOW — Non-locked, non-linked pre-registered claim mints to any client wallet (by design)

**File:** `worker/src/claim/mint.rs` (recipient precedence). For a pre-registered
attendee with neither a column-P lock nor a linked profile wallet, the claim
token alone lets the holder direct the mint to any wallet. Intended bearer-token
model, but it's what makes the #1 token leak impactful.

**Fix (optional):** when the attendee HAS a linked profile wallet, prefer the
server-resolved linked wallet and treat an explicit override as a last resort.

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
