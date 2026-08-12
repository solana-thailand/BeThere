# Deposit, Refund & Rolling-Credit Model

BeThere runs a **dual-track RSVP deposit**: attendees back their in-person spot
with either on-chain **USDC** (Solana escrow, attendee-signed) or off-chain
**THB** (PromptPay slip, admin-verified). This doc is the end-to-end map of how
each method is initiated, stored, verified, refunded, and (for THB) rolled
forward as credit — plus which page owns which action.

> Scope note: deposits only apply to **in-person** participation. Online
> attendees skip the deposit gate entirely (`signup.rs` §3e, `is_online_participation`).

---

## 1. The two deposit methods

| | USDC | THB |
|---|---|---|
| Rail | Solana escrow program (`bethere-escrow`), attendee signs on-chain | PromptPay bank transfer, attendee uploads slip |
| Initiation endpoint | `POST /api/deposit/usdc` (attendee-authed) | `POST /api/deposit/thb/upload` (attendee-authed) |
| Verification | On-chain TX confirmed + signer cross-check (automatic) | Admin approves the slip (`POST /api/deposit/thb/verify`) |
| Source of truth | On-chain `AttendeeDeposit` PDA; mirrored to KV + D1 `DepositStatus` | D1 `thb_deposits` table (Phase 3d), mirrored to KV |
| Domain enum | `DepositMethod::Usdc` | `DepositMethod::Thb` |

Rolling credit (a past THB/USDC deposit re-applied to a new event) surfaces as
two extra pseudo-methods, `DepositMethod::CreditThb` / `CreditUsdc`
(`domain/src/models/deposit.rs:14`).

### 1.1 Deposit page state machine

The deposit page (`frontend-leptos/src/pages/deposit/`) is a wizard driven by
`DepositPageState` (`pages/deposit/types.rs:66`). Relevant states:

- Entry: `Loading` → `ChoosePayment` (pick USDC or THB) — or `NotEnabled`,
  `AlreadyDeposited`, `Error`.
- USDC flow: `WalletConnected` → `AwaitingConfirmation` / `UsdcQrReady` →
  `DepositConfirmed`.
- THB flow: `ThbUploading` → `ThbUploaded` / `ThbRejected` / `ThbAuthRequired`
  (401 — session expired; the deposit page loads publicly but upload is
  identity-gated).
- Refund flow (USDC): `RefundChooseWallet` → `RefundWalletConnected` →
  `RefundSigning` → `RefundConfirmed`.
- Close-deposit / reclaim-rent flow: `CloseDeposit*` variants.

### 1.2 USDC deposit — Solana Pay round trip

`worker/src/handlers/deposit/usdc/handlers.rs`:

1. `deposit_usdc_handler` (`:172`, attendee-authed) validates the event, rejects
   deposits after `event_end`, runs the deposit-deadline reclaim check, assigns
   a **deposit order** via an atomic counter, computes the **refundable tier**
   (`order <= max_refundable_deposits`, or unlimited when `max_refundable == 0`),
   saves a *pending* `DepositStatus` (`verified=false`, no signature), and
   returns a `solana:<callback_url>` Solana Pay URL.
2. The wallet fetches the actual transaction from `deposit_usdc_tx_handler`
   (`GET /api/deposit/usdc/tx`, **public**, `:452`), which builds the on-chain
   `deposit` instruction against the PDA-derived `EventEscrow` + `AttendeeDeposit`
   accounts and the vault ATA. The attendee signs and submits.
3. Confirmation is dual-pathed:
   - `confirm_deposit_handler` (`GET /api/deposit/usdc/confirm`, `:567`) is
     polled by the frontend; it verifies the TX on-chain and **cross-checks the
     signer** against the declared wallet before flipping `verified=true`. It
     also self-heals: recovers a missing signature from PDA history
     (`discover_deposit_tx_on_chain`).
   - `deposit_webhook_handler` (`POST /api/deposit/usdc/webhook`, **public**,
     `:923`) accepts either a `WEBHOOK_SECRET` bearer (Helius) or a valid JWT
     (frontend) and records the signature, then detaches verification.
4. On verification: dual-write to D1 (`verify_deposit`), detach Google Sheets
   write, and auto-generate the attendee QR if missing.

An orphan pending record (user rejected the wallet prompt) is re-usable — the
handler reuses the existing `deposit_order` so the refundable tier isn't
double-assigned (`:308`, `:357`).

### 1.3 THB deposit — slip upload + admin verify

`worker/src/handlers/deposit/thb/handlers/slip_upload.rs`:

1. `upload_thb_slip_handler` (`:100`, attendee-authed) enforces **VULN-012**
   ownership (JWT email must match the attendee row), validates the slip
   (`validate_slip_url` — JPEG/PNG/WebP only, SVG rejected as XSS, size-capped),
   uploads the data URL to R2, **requires bank info** (account / bank / name for
   later refund), runs the same deadline-reclaim check, then writes a
   `ThbDeposit` (`verified=false`) and a pending `DepositStatus`
   (`method=Thb`). Deposit order + refundable tier assigned as for USDC.
2. Admin records-on-behalf via `admin_upload_thb_slip_handler`
   (`POST /api/deposit/thb/admin-upload`, staff) — skips the email-match gate
   (admin-authed + audited) for attendees who couldn't upload themselves.
3. `verify_thb_slip_handler` (`POST /api/deposit/thb/verify`, staff,
   `slip_verify.rs:17`) flips `verified` (approve) or leaves it false + sets
   `rejected` (reject), mirrors to the `DepositStatus`, dual-writes D1, writes
   the deposit columns + QR to the sheet, and audits
   (`DepositVerified` / `DepositRejected`).

THB deposits live **exclusively in D1** (`thb_deposits` table) as the settleable
record; KV is a mirror (`worker/src/db/thb_deposits.rs:1-4`).

---

## 2. Registration credit auto-apply (rolling credit)

A held THB deposit from a *previous* event becomes **rolling credit** on the
contact, and is spent automatically the next time that person registers for a
deposit-gated event.

Flow in `worker/src/handlers/register/signup.rs`:

- **§5c (`:287`)** — After capacity checks, if the event is deposit-gated, the
  attendee is in-person, and `credit_identity_ok`, read the contact's credit
  balance (`sheets::contacts::get_credit_balance`). If `credit_thb >=
  deposit_amount_thb` (or the USDC equivalent), mark the deposit as
  credit-covered (`credit_covered_method = "credit_thb" | "credit_usdc"`).
- **`credit_identity_ok` gate (`:89`)** — Rolling credit is stored value tied to
  an email, so it is only spendable by a Google-verified session *or* a wallet
  session whose wallet is already bound to that email (Plan 017). A wallet
  session that merely *types* an email cannot drain another email's credit.
- **§7b (`:343`)** — Persist the attendee to D1 **fatally** *before* spending any
  credit (so credit is never consumed for a reservation that didn't durably save).
- **Auto-apply, fail-closed & correctly ordered (`:387`)** — **Decrement credit
  first** (`decrement_credit`); only if that succeeds record a covered, verified
  `ThbDeposit` with `slip_url = "ROLLING_CREDIT_AUTO_APPLIED"`, `verified = true`,
  `verified_by = "SYSTEM_ROLLING_CREDIT"`. If the decrement fails,
  `credit_covered_method` is cleared and the attendee falls back to the normal
  payment path (credit untouched). The reverse order would double-spend real
  money on retry.
- **next_step (`:665`)** — When credit covered the deposit, the response routes
  straight to `/ticket/...` instead of the deposit page.

### Credit fields

- **D1 `contacts`** (`worker/src/db/contacts.rs`): `deposit_credit_thb`,
  `deposit_credit_usdc`, `deposit_credit_since` (cols K–M), written by
  `update_deposit_credit` (`:80`) / the increment path. Plus the Phase-3 exit
  flag `credit_refund_requested` / `credit_refund_requested_at`
  (`set_/clear_/get_credit_refund_requested`, `:335`–`:438`) and the aggregate
  `credit_liability` (`:301`).
- The **Master Contacts Sheet** is the human-readable master; D1 is the read
  source of truth for the liability chip and the request-flag reads.

---

## 3. USDC refund — attendee-signed refund + close

Endpoint: `POST /api/escrow/refund` → `refund_and_close_tx_handler`
(**public**, `worker/src/handlers/deposit/escrow/handlers.rs:168`).

It builds a **single atomic transaction** combining `refund` +
`close_deposit`, so one wallet signature both returns the USDC and reclaims the
`AttendeeDeposit` PDA rent. The on-chain program *enforces* this pairing:
`bethere-escrow`'s refund instruction requires a sibling `close_deposit`
(discriminator 7) in the same transaction via instruction introspection —
`SEC-010`, `bethere-escrow/src/instructions/refund.rs:87` / `require_close_deposit_pair`.

### Preconditions (checked server-side before building the TX)

- `deposit_enabled` and `escrow_address` set (`:183`, `:187`)
- deposit status found and `verified` (`:206`)
- `method == DepositMethod::Usdc` — THB is rejected with a pointer to the THB
  flow (`:212`)
- `refundable` tier — overflow-tier (non-refundable) deposits are rejected
  (`:219`). Non-refundable deposits are simply forfeited; no on-chain check-in
  is even needed for them (`mark_checked_in_tx_handler:330`).

### Refund-window model (two-path)

The on-chain `refund` instruction is the source of truth
(`bethere-escrow/src/instructions/refund.rs:64-104`):

- Refund requires `clock >= event_end`, else `RefundNotYetAllowed`.
- If the attendee was **not checked in**, refund additionally requires
  `clock < refund_deadline`, else `RefundDeadlinePassed`. After the deadline the
  organizer can `claim_forfeited` the no-show deposit.
- **Checked-in attendees can refund anytime after `event_end`** — they showed up.

So the window is:

- **checked-in → `[event_end, ∞)`**
- **no-show → `[event_end, refund_deadline)`**

The frontend mirrors this advisory-only in `event_refund_window_open`
(`frontend-leptos/src/pages/deposit/types.rs:222`) to hide a CTA that would
revert; it fails safe on missing `event_end_ms` / `refund_deadline_ms`. The
backend surfaces both inputs — `checked_in` and `refund_deadline_ms` — in
`DepositStatusResponse` (`usdc/handlers.rs:108-142`); `checked_in` reflects the
off-chain (Sheets/D1) state the organizer keeps in sync with the on-chain
`mark_checked_in` instruction.

### Related on-chain actions

- `POST /api/escrow/close-deposit` (public, `handlers.rs:835`) — reclaim rent
  standalone (e.g. after a forfeit) without a refund.
- `POST /api/escrow/rollover-deposit` (attendee-authed,
  `escrow/status.rs:424`) — the **USDC counterpart of THB hold-as-credit**:
  atomically moves a verified deposit from a past event's escrow to a new event
  from the **same organizer** (both escrows must exist; organizer wallets must
  match). No off-chain "hold" is allowed for USDC — the on-chain move is the
  only settleable path, preventing double-credit (`hold_credit.rs:97`).
- Organizer lifecycle (staff): `escrow/init`, `mark-checked-in`,
  `deactivate-event`, `close-event`, `claim-forfeited` (batch, excludes
  refunded wallets and checked-in PDAs, and drops indexer-lag ghosts via
  `filter_forfeitable_deposits`).

---

## 4. THB refund / credit

THB has **three** post-event resolutions, and they are mutually exclusive per
deposit.

### 4.1 Hold as rolling credit (attendee)

`POST /api/deposit/hold` → `hold_deposit_handler`
(`worker/src/handlers/deposit/thb/handlers/hold_credit.rs:32`, attendee-authed).

- VULN-012 ownership check, must be `verified`, `method == Thb`, not already
  refunded/held.
- **Atomic CAS then increment** — `try_settle_hold_credit`
  (`db/thb_deposits.rs:163`) flips `held_as_credit 0→1` in one conditional
  UPDATE that only matches `verified=1 AND held_as_credit=0 AND refunded=0`.
  Returns true only if *this* call flipped it, so two concurrent `/hold`
  requests can't both grant credit. **Settle before incrementing credit**
  (`hold_credit.rs:139`): if settle succeeds but the credit increment fails, no
  money is created (admin reconciles); the reverse would allow infinite credit
  via retry.
- On success, `increment_credit` on the contacts sheet, audit
  `DepositHeldAsCredit`. The credit is then auto-applied at the next
  registration (§2).

### 4.2 Request return of held credit (attendee → organizer)

`POST /api/deposit/request-credit-refund` → `request_credit_refund_handler`
(`hold_refund_request.rs:75`, attendee-authed, JWT email only — no body).

- **Visibility-only flag**, *not* a payout (Issue #061 §D3). Sets
  `credit_refund_requested` on the contact (cross-event, since credit is a
  rolling balance across events), dual-written to D1 + Sheets. Idempotent — a
  re-call just re-stamps the timestamp.
- The organizer then pays out through the *existing* THB refund tooling
  (`/refund/mark` or `/refund/batch-thb`) and clears the flag.
- Reads/admin: `GET /api/deposit/credit-refund-request` (attendee's own flag),
  `GET /api/deposit/credit-refund-requests` (admin queue),
  `POST /api/deposit/clear-credit-refund-request` (admin clears after payout).

### 4.3 Organizer cash refund (staff)

`POST /api/refund/mark/{attendee_id}` → `mark_refund_handler`
(`thb/handlers/refund.rs:20`, staff).

- Must be `verified`, not already refunded, **not held as credit** (money-safety
  guard, `:53`), and `refund_proof_url` is required (uploaded to R2).
- **Atomic CAS** — `try_settle_refund` (`db/thb_deposits.rs:200`) flips
  `refunded 0→1` only when `verified=1 AND refunded=0 AND held_as_credit=0`.
- Writes refund status + link to the sheet (detached), dual-writes D1, audits
  `RefundMarked`.
- Batch: `POST /api/refund/batch-thb` (`refund.rs:249`) — per-deposit CAS, skips
  already-refunded / held / unverified.
- Manual: `POST /api/refund/manual/{attendee_id}` (`refund.rs:412`) — sets a
  refund status on the sheet for someone with no deposit record (e.g. a VIP).

### 4.4 Hold-vs-refund mutual exclusion (the CAS invariant)

The two settlement CAS statements are deliberately complementary:

- hold requires `refunded = 0`
- refund requires `held_as_credit = 0`

So a THB deposit can be **held XOR cash-refunded, never both**, even under
concurrent attendee/admin actions — no double-payout
(`db/thb_deposits.rs:154-230`; both guards echoed in the handlers). Admin can
also hold on an attendee's behalf via `POST /api/refund/hold/{attendee_id}`
(`admin_hold_deposit_handler`), which shares the same invariants.

---

## 5. Where each action lives in the UI

| Surface | File | Owns (per method) |
|---|---|---|
| **Deposit page — AlreadyDeposited** | `pages/deposit/already_deposited.rs` | **USDC**: the "Claim Refund" CTA (→ `RefundChooseWallet`) when refundable + window open; else "available after event" / "non-refundable" notices. **THB**: no action here — points the attendee to the ticket page ("manage it from your ticket"). |
| **Ticket page — action cards** | `pages/ticket/action_cards.rs` | `DepositActionCard` (pay), `DepositVerifiedCard`, `DepositPendingCard`, `RefundCard` (return receipt), `ReclaimActionCard` / `MovedOnlineCard` (deadline). **USDC**: `RolloverActionCard` (self-contained wallet sign → `/escrow/rollover-deposit`). **THB**: `HoldDepositCard` (→ `/deposit/hold`) and `RequestCreditRefundCard` (→ `/deposit/request-credit-refund`, only rendered once `held_as_credit == true`). |
| **Claim success screen** | `pages/claim.rs` | `ClaimState::Success(ClaimMintData)` — post-check-in NFT mint result (asset ID + explorer link). Not a deposit action, but the terminal step of the attendee journey. |

Rule of thumb: **USDC refund/rollover is owned by the deposit page + the ticket
rollover card** (both are wallet-signing flows); **THB hold / credit-refund is
owned by the ticket page** (authenticated POSTs, no wallet). The deposit page
never mislabels a ฿ deposit as USDC.

---

## 6. Per-method summary table

| Method | Initiate | Verify | Refund / credit | Who acts | On-chain vs manual |
|---|---|---|---|---|---|
| **USDC** | `POST /api/deposit/usdc` → Solana Pay; wallet signs `deposit` TX | Automatic — TX confirmed on-chain + signer cross-check (`/confirm` poll or `/webhook`) | `POST /api/escrow/refund` (refund+close, atomic); `/escrow/rollover-deposit` to move to next event; `/escrow/close-deposit` to reclaim rent | **Attendee** signs every money-moving TX; organizer only runs escrow lifecycle | **On-chain** (attendee-signed; escrow program is source of truth) |
| **THB** | `POST /api/deposit/thb/upload` (slip + bank info → R2); or admin `/thb/admin-upload` | **Organizer** approves `POST /api/deposit/thb/verify` | Hold: attendee `POST /api/deposit/hold` (→ rolling credit, auto-applied next event). Request-return: attendee `POST /api/deposit/request-credit-refund` (flag only). Cash refund: organizer `POST /api/refund/mark/{id}` (+ `/batch-thb`, `/manual`) | Upload + hold + request-return = **attendee**; verify + cash refund = **organizer** | **Manual** (bank transfer; D1 CAS settlement + Sheets mirror) |
| **Rolling credit** (`CreditThb`/`CreditUsdc`) | Auto-applied at registration (`signup.rs` §5c/§7b) from a prior held balance | Recorded as a pre-verified `ThbDeposit` (`SYSTEM_ROLLING_CREDIT`) | Exit via `request-credit-refund` (organizer pays out through THB tooling) | System applies; attendee requests exit; organizer pays out | **Manual** (balance on the contact) |

---

## 7. Notes / inconsistencies worth flagging

- **Credit-detection heuristic on the deposit page.** `already_deposited.rs:103`
  decides `is_credit` by looking for the substring `"CREDIT"` in the deposit's
  `tx_signature` or `wallet_address`. But the auto-applied credit record written
  by `signup.rs:421` marks the credit via `slip_url =
  "ROLLING_CREDIT_AUTO_APPLIED"` (and `verified_by = "SYSTEM_ROLLING_CREDIT"`),
  not those fields. Worth verifying the credit label actually renders for
  auto-applied credit deposits, or align the detection on `DepositMethod::Credit*`.
- **Slip size message vs check.** `slip_upload.rs:22-58` caps the encoded data
  URL at `5 * 1024 * 1024` and the doc-comment says "decoded ≤ 5MB, encoded ≤
  7MB", but the user-facing error says "max 3MB". Cosmetic, but the three
  numbers disagree.
- **Non-atomic fallback when D1 is absent.** The settlement CAS helpers fall
  back to `Ok(true)` when `d1` is `None` (tests/local) — safe there, but it
  means the double-payout protection only holds when D1 is configured (it always
  is in prod).
