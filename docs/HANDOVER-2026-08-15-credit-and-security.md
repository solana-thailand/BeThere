# Handover — Rolling-Credit Rebuild + Admin Security (2026-08-14 → 08-15)

A large session that (1) discovered + fixed a real money-loss bug in rolling
deposit credit, (2) rebuilt the credit system on a proper ledger with a rolling
("Model B") lifecycle, (3) closed every admin IDOR/PII hole from a two-agent
review, and (4) added the admin visibility to see how attendees got in.

> **Deployment:** everything below is committed on `develop` and **deployed to
> production** unless noted. Worker + frontend deploy together via
> `worker/deploy.sh`. Latest deploy this session: `9046cbd`.

---

## 1. Rolling deposit credit — the incident + the rebuild

### What was broken
Credit lived as a mutable cell in the Google "Contacts" sheet. Two defects
silently destroyed money (incident 2026-08-14):
- **Non-atomic hold:** hold flipped `thb_deposits.held_as_credit` in D1, then
  wrote the sheet best-effort — a failed/rate-limited sheet write left the deposit
  "held" with **no credit** (6 of 10 holders, ฿3,000 lost).
- **Duplicate-row shadowing:** the Master Contacts sheet had duplicate rows per
  email (58/151); `find_contact_row` reads the FIRST match, so credit written to
  one row was shadowed by an empty row above it (4 holders, ฿2,000 invisible).
- **Blind admin liability:** the chip summed a D1 column hold never wrote → always ฿0.

### The fix — D1 credit ledger (source of truth)
`worker/migrations/0028_credit_ledger.sql` + `worker/src/db/credit_ledger.rs`:
- Append-only, **org-scoped**: balance = `SUM(delta)` over `(email, organization_id, currency)`.
- `UNIQUE(deposit_id, reason) WHERE deposit_id IS NOT NULL` for idempotency —
  **ON CONFLICT must repeat that WHERE predicate** (partial index) or it errors.
- Functions: `record` (hold/return), `try_spend` (atomic conditional insert —
  spends only if balance ≥ amount), `balance`, `liability`, `reconcile`,
  `thb_balances_by_email`, `emails_applied_credit`, `remove_return`.
- Backfilled all affected holders from `thb_deposits.held_as_credit=1` truth
  (฿5,000 restored, idempotent).

### Model B — rolling lifecycle (the intended economics)
- **Apply** (auto at registration or admin) → ledger `−฿` (a commitment).
- **Check-in** → ledger `+฿` (`REASON_RETURN`, in-person, `is_credit_covered()`,
  idempotent) → rolls to the next event. **No-show never returns → forfeited.**
- **Undo check-in** → `remove_return` (DELETE) so a re-check-in re-adds it.
- **Exit to cash** → the existing request-credit-refund flow (organizer PromptPays;
  clearing the flag writes a `−฿` refund entry, idempotent per `requested_at`).
- Net: **pay ฿500 once, it rolls forward as long as they keep showing up.**

### Safety nets
- Daily **reconcile cron** (`scheduled` in `lib.rs`) → alerts Slack on orphan
  holds (held deposit with no ledger entry) or negative balances.
- Source-scan **guard tests** (`worker/tests/credit_ledger_guards.rs`): ON CONFLICT
  predicate present, balance read org-scoped, `try_spend` keeps its `>= amount` guard.

### DepositSource enum (GOAT structural)
`domain/src/models/deposit.rs`: `DepositSource {Cash|Credit|Comp}` +
`ThbDeposit::source()` = the single classification. `is_non_cash()` /
`is_credit_covered()` delegate to it. **Non-cash deposits are excluded from the
refund queue and rejected in `mark_refund`/`batch_refund`/`admin_hold`** — they
were never funded with cash, so refunding/re-holding would pay out / mint money.

### Bugs fixed along the way
- **Slip-link:** credit/comp deposits store a sentinel in `slip_url`
  (`ROLLING_CREDIT_AUTO_APPLIED` / `STAFF_COMP_WAIVED`); the Refund Queue rendered
  it as an href → broken. Frontend now only links real `/api/…`/`http` slips.
- **Ticket QR "waiting for verify":** credit registrations wrote only
  `thb_deposits`, never a verified `deposit_status` (what the ticket gates the QR
  on). Both signup + `apply-credit` now write it; `apply-credit` is
  idempotent/self-healing (re-run heals an already-applied attendee).

---

## 2. Admin visibility (frontend)
- In-person roster (`admin.rs`): per-row **credit badges** — 🔵 Credit ✓ /
  🟢 Deposit ✓ / ⚪ Comp / 🔴 Refunded — plus a **"฿N credit"** (remaining) chip and
  an **Apply Credit** button (gated on `credit_thb > 0`, backend-enforced).
  Powered by `AttendeeListItem.credit_thb` + `used_credit`, annotated by the list
  handler from the ledger.
- Deposits page: a **Cash/Credit/Comp summary chip** (`GET /api/deposit/credit-used`)
  so the money reconciles at a glance.

---

## 3. Admin security — all IDOR/PII holes closed
From a two-agent deep-dive review. The tell was `Extension(_claims)` (received but
never checked). Every fix uses helpers that already existed.
- **S1** contacts/community: `/contacts*`, `/contacts/audience` (CSV of every
  email/wallet), `/community/*` were readable by ANY staff across ALL orgs →
  `auth::require_super_admin` gate.
- **Slip IDOR:** `/storage/slips` + `/storage/refunds` (bank details) were only
  identity-gated → moved to the staff `protected` router.
- **S2** quiz/adventure/form-config: admin handlers used `resolve_event` (no check)
  while sharing it with public endpoints → the ADMIN ones now use
  `resolve_event_with_access` (get_admin_quiz leaked the answer key).
- **S3** campaigns: trusted a caller-supplied `organization_id` → new
  `auth::require_org_access` (super-admin OR `org.owner_emails`) on all 10 handlers.
- **S5** live dashboard: added `resolve_event_with_access`.

Plus a `waive-deposit` path for staff/organizer/super-admin (no more fake slips).

---

## 4. Still open (next sessions)
- **IMPROVE (small):** admin.rs roster double-escape (needs move/clone care),
  Hold-as-Credit 2-step confirm, 401→login redirect. verify-state guard +
  Deposits-page double-escape already done.
- **GOAT leftovers:** Credit-Used *list* tab on the Deposits page (endpoint
  already returns the list), `api_get_json<T>` (~250 lines of GET boilerplate),
  typed deposit fields (kill `deposit_verified == Some("true")` stringly-typing),
  relabel "Comp" → "ฟรี/Staff".
- **Contacts sheet:** 58 duplicate rows remain (harmless now — credit is off the
  sheet). Sheet credit columns are a non-authoritative mirror.
- **Mainnet USDC (user-owned):** fund the deployer key
  `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN` (~1.3 SOL), deploy the escrow
  program, Squads multisig, external audit, canary. See
  `docs/escrow-audit-2026-08-13.md`.
- **Product idea (parked):** turn completed events into evergreen learning
  modules/paths (quiz + badge + campaigns already exist) — recommended as a
  1-event spike, not a platform. Content curation is the real cost.

## Key files
- `worker/src/db/credit_ledger.rs`, `worker/migrations/0028_credit_ledger.sql`
- `domain/src/models/deposit.rs` (`DepositSource`, `ThbDeposit::source/is_non_cash/is_credit_covered`)
- `worker/src/handlers/deposit/thb/handlers/{hold_credit,hold_admin,slip_list,slip_verify,refund,hold_refund_request}.rs`
- `worker/src/handlers/register/signup.rs`, `worker/src/handlers/checkin.rs` (Model B return)
- `worker/src/auth.rs` (`require_super_admin`, `require_org_access`)
- `worker/src/handlers/{contacts,community,campaigns,quiz,adventure,dashboard}.rs`, `events/audit.rs` (authz)
- `frontend-leptos/src/pages/{admin,admin_deposit}.rs`, `frontend-leptos/src/api/deposit.rs`
- Memory: `rolling-deposit-credit-flow.md`
