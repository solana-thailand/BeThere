# 010 — Deposit/Refund with PDA Escrow + Dual-Track Payment

> Supersedes Issue 001 (SOL-only escrow). Updated for USDC + PromptPay dual-track.

## Summary

Automate the deposit/refund cycle for BeThere events. Attendees lock a fixed deposit at registration (USDC via Solana Pay **or** THB via PromptPay). After check-in, deposits are refunded automatically (USDC) or via admin-managed queue (THB). No-show deposits forfeit to the organizer.

## Current Manual Flow (Event 1)

```
Luma RSVP → export CSV → upload Google Sheet → DM for payment →
bank transfer → BeThere check-in → manual refund one-by-one
```

**Pain**: 6 human touchpoints, manual bank transfers, no-shows get refunded or organizer chases them.

## Target Flow

```
Registration                Event Day              After Event
    │                          │                       │
  Pay deposit ──────→  Check in (scanner) ──→  Refund (auto or queue)
  (USDC or THB)       (already built)         (to wallet or bank)
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     DEPOSIT (registration)                   │
│                                                             │
│   Has wallet? ──YES──→ Solana Pay QR ──→ USDC to PDA escrow │
│       │                                      │              │
│       NO                                     │              │
│       │                                      │              │
│   PromptPay QR ──→ Upload slip ──→ Admin verifies in DB     │
│                   (THB to organizer bank)                   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      REFUND (after event)                    │
│                                                             │
│   Paid USDC? ──YES──→ Auto-refund USDC from PDA to wallet   │
│       │              (on-chain `refund` instruction)         │
│       NO                                                   │
│       │                                                    │
│   Refund queue in admin dashboard                           │
│   → Admin sees slip + bank info                             │
│   → Admin transfers 500 THB via PromptPay banking app       │
│   → Admin pastes transfer receipt URL                       │
│   → Clicks "✓ Confirm Refund"                               │
└─────────────────────────────────────────────────────────────┘
```

### Why match deposit medium for refund?

| Concern | THB→THB | THB→USDC |
|---------|---------|----------|
| FX risk | None | Organizer absorbs rate change |
| Regulatory | Normal bank transfer | Looks like unlicensed exchange |
| Attendee expectation | "500 THB in, 500 THB out" ✅ | "I got internet money back?" 😕 |
| Fees | Free (PromptPay) | On-ramp/off-ramp 1-3% each way |

### Why USDC (not SOL) for on-chain deposits?

- **Price stability**: 15 USDC = 500 THB today, tomorrow, always. SOL swings 20% in a week.
- **Attendee trust**: "I put in $15, I get back $15." SOL could be worth $12 or $18 at refund time.
- **cNFT gas is already free**: Bubblegum V2 minting is paid by the organizer via Helius. No need for attendees to hold SOL.

## On-Chain Program: Escrow (Quasar)

> **Framework choice**: Quasar over Anchor for CU efficiency (zero-copy, no Borsh deserialization),
> cleaner SPL token CPI (method-style `.transfer().invoke_signed()`), and built-in Mollusk testing.
> Quasar's [Build an Escrow](https://quasar-lang.com/docs/guides/build-an-escrow) guide maps 1:1 to our design.
> Fallback: if Quasar blocks us (beta risk), port to Anchor is mechanical — same `#[derive(Accounts)]`, same constraints.

### Accounts

```rust
use quasar_lang::prelude::*;

/// PDA — one per event, holds all USDC deposits in a token account.
/// Space: 1 (discriminator) + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 1 = 147 bytes
#[account(discriminator = 1)]
pub struct EventEscrow {
    /// Event organizer (can claim forfeited deposits).
    pub organizer: Address,
    /// USDC mint address.
    pub usdc_mint: Address,
    /// Token account owned by this PDA — holds deposited USDC.
    pub vault: Address,
    /// Fixed deposit amount in USDC smallest unit (6 decimals → $15 = 15_000_000).
    pub deposit_amount: u64,
    /// Event end timestamp (unix seconds). Refunds allowed after this.
    pub event_end: i64,
    /// Refund deadline (event_end + grace period, e.g., +7 days).
    pub refund_deadline: i64,
    /// Total USDC deposited across all attendees.
    pub total_deposited: u64,
    /// Total USDC refunded.
    pub total_refunded: u64,
    /// Total USDC claimed by organizer (forfeited no-show deposits).
    pub total_forfeited: u64,
    /// Whether the event is active (deposits accepted).
    pub is_active: bool,
    /// Bump seed for PDA.
    pub bump: u8,
}

/// PDA — one per attendee per event.
/// Space: 1 (discriminator) + 32 + 32 + 8 + 8 + 1 + 1 + 1 = 84 bytes
#[account(discriminator = 2)]
pub struct AttendeeDeposit {
    /// Attendee's wallet address.
    pub attendee: Address,
    /// Reference to EventEscrow.
    pub event: Address,
    /// Amount deposited (USDC smallest unit).
    pub amount: u64,
    /// Deposit timestamp.
    pub deposited_at: i64,
    /// Whether attendee checked in (set by organizer authority).
    pub checked_in: bool,
    /// Whether refund has been claimed.
    pub refunded: bool,
    /// Bump seed for PDA.
    pub bump: u8,
}
```

### Instructions

| Instruction | Who calls | When | What it does |
|-------------|-----------|------|-------------|
| `create_event` | Organizer | Before registration | Initialize `EventEscrow` PDA + vault token account |
| `deposit` | Attendee | At registration | Transfer USDC to vault, create `AttendeeDeposit` PDA |
| `mark_checked_in` | Organizer (authority) | At event (scanner) | Set `checked_in = true` on attendee's deposit account |
| `refund` | Attendee (after event) | After check-in + event end | Transfer USDC from vault back to attendee |
| `claim_forfeited` | Organizer | After refund deadline | Transfer all unclaimed USDC to organizer's wallet |
| `close_event` | Organizer | After all refunds/claims | Close escrow account, reclaim rent |

### PDA Seeds

```
EventEscrow:     ["escrow", event_id.as_bytes()]
AttendeeDeposit: ["deposit", event_escrow, attendee]
```

Quasar uses field names directly in `seeds` (no `.key().as_ref()` needed).

### Key Quasar Patterns for This Program

**Token CPI — deposit (attendee → vault):**
```rust
self.token_program
    .transfer(self.attendee_ta, self.vault_ta, self.attendee, amount)
    .invoke()
```

**Token CPI — refund (vault → attendee, PDA-signed):**
```rust
let seeds = bumps.event_escrow_seeds();

self.token_program
    .transfer(self.vault_ta, self.attendee_ta, self.event_escrow, amount)
    .invoke_signed(&seeds)?;
```

**Token CPI — claim forfeited (vault → organizer, PDA-signed):**
```rust
let seeds = bumps.event_escrow_seeds();

self.token_program
    .transfer(self.vault_ta, self.organizer_ta, self.event_escrow, self.vault_ta.amount())
    .invoke_signed(&seeds)?;

self.vault_ta
    .close(self.token_program, self.organizer, self.event_escrow)
    .invoke_signed(&seeds)?;
```

**Account constraints:**
```rust
#[account(
    has_one = organizer,
    constraint = event_escrow.is_active,
    constraint = !attendee_deposit.checked_in,
    close = attendee,  // reclaim rent on refund
    seeds = [b"deposit", event_escrow, attendee],
    bump = attendee_deposit.bump
)]
pub attendee_deposit: &'info mut Account<AttendeeDeposit>,
```

### Access Control

- `deposit`: anyone (open event)
- `mark_checked_in`: organizer only (verified via scanner auth)
- `refund`: the attendee themselves (signed by their wallet)
- `claim_forfeited`: organizer only, only after `refund_deadline`

## Off-Chain: BeThere Worker + Frontend

### New Event Config Fields

```rust
// In EventConfig (domain/src/models/event.rs)
pub deposit_enabled: bool,
pub deposit_amount_usdc: u64,       // e.g., 15_000_000 = $15
pub deposit_amount_thb: u64,        // e.g., 500
pub escrow_address: Option<String>, // EventEscrow PDA (set after create_event)
pub refund_deadline_hours: u32,     // Hours after event end (default: 168 = 7 days)
```

### New API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/deposit/status/{attendee_id}` | Check if attendee has deposited (USDC or THB) |
| `POST` | `/api/deposit/usdc` | Build Solana Pay TX for USDC deposit (returns serialized TX) |
| `POST` | `/api/deposit/thb/upload` | Upload PromptPay payment slip image |
| `POST` | `/api/deposit/thb/verify` | Admin verifies/rejects a slip |
| `GET` | `/api/deposit/thb/pending` | List unverified slips (admin) |
| `POST` | `/api/refund/{token}` | Build refund TX (for checked-in attendees) |
| `GET` | `/api/refund/queue` | Refund queue for admin (THB refunds pending) |
| `GET` | `/api/refund/refunded` | List already-refunded THB deposits |
| `POST` | `/api/refund/mark/{attendee_id}` | Mark THB refund as done — requires `refund_proof_url` (admin) |
| `POST` | `/api/refund/manual/{attendee_id}` | Set refund status for any attendee — e.g., VIP who didn't deposit (admin) |

### Database / KV Storage

**THB deposits** (no on-chain record — tracked in KV):

```
Key: "event:{id}:deposit:thb:{attendee_api_id}"
Value: {
  "attendee_id": "abc123",
  "amount_thb": 500,
  "slip_url": "https://r2.../slip-abc123.jpg",
  "verified": false,
  "verified_by": null,
  "verified_at": null,
  "uploaded_at": "2025-06-01T10:00:00Z",
  "refunded": false,
  "refunded_at": null,
  "bank_account": "123-4-56789-0",       // Required since handover #074
  "bank_name": "KBank",                // Required since handover #074
  "account_name": "John Doe",          // Required since handover #074
  "refund_proof_url": null             // Transfer receipt URL (set on refund)
}
```

**Deposit status** (per attendee, cached):

```
Key: "event:{id}:deposit:status:{attendee_api_id}"
Value: {
  "method": "usdc" | "thb",
  "amount": 15.0,          // USDC or THB
  "currency": "USDC" | "THB",
  "tx_signature": "...",   // USDC only
  "verified": true,
  "deposited_at": "..."
}
```

### Frontend Changes

**Deposit flow page** (new route: `/deposit/{attendee_id}`):

1. "Choose payment method"
   - 🔑 "I have a Solana wallet" → shows Solana Pay QR + wallet adapter
   - 🏦 "I'll pay via Thai bank" → shows PromptPay QR + slip upload form
2. After deposit: redirect to confirmation page
3. Status polling: "Waiting for payment..." → "Deposit confirmed ✅"

**Claim page** (existing, enhanced):

- After check-in + cNFT mint: "Claim your refund" button
- USDC: sign transaction → USDC arrives in wallet
- THB: "Your refund is being processed. The organizer will send 500 THB to your bank within 7 days."

**Admin dashboard** (existing, new tabs):

- "Deposits" tab: table of all attendees with deposit status (USDC ✅ / THB pending / Not deposited)
- "Refund Queue" tab: checked-in THB depositors awaiting refund, shows slip + bank info, "Mark Refunded" button requires transfer receipt URL
- "Refunded" tab: all refunded deposits with slip, bank info, refund proof link, and timestamp
- "Escrow" tab: on-chain stats (total deposited, total refunded, total forfeited, available for claim)
- Bulk action: "Set Refund Status" — marks refund_status (+ optional refund_link) in Google Sheet for any attendee, including VIPs who didn't deposit

**Ticket page** (attendee-facing):
- Shows "Refund: Processed ✓" with "View Refund Receipt →" link (from `refund_proof_url` in KV)
- Shows "Organizer Refund Link" card when organizer fills in `refund_link` column in Google Sheet

### PromptPay Slip Verification (Free)

No paid OCR service needed — admin verifies manually:

1. Attendee scans PromptPay QR (static QR for organizer's bank)
2. Attendee uploads screenshot of payment confirmation + fills in bank account info (required for refund)
3. Admin sees slip in dashboard, verifies amount + name match
4. Admin clicks "Verify" → deposit status updated
5. After event, admin sees refund queue with slip + bank info
6. Admin transfers THB, pastes transfer receipt URL, clicks "✓ Confirm Refund"
5. **Auto-actions on verify** (non-fatal if they fail):
   - Sheet columns N (deposit_method), O (deposit_amount), Q (deposit_verified) written to Google Sheet
   - QR code auto-generated for the attendee if they don't have one yet

**Same auto-actions** occur for USDC on-chain confirmation (both in the confirm handler and the Helius background webhook).

**Ticket page status flow** (for deposit events):
```
Registered → Awaiting Deposit Verification → Ready for Check-In → Checked In
```
- "Awaiting Deposit Verification": is_approved && deposit_info exists && !deposit_info.verified
- "Ready for Check-In": only appears after deposit is verified (or when no deposit required)

**Registration form**: Inline per-field validation (name, contact_channel, contact_handle, deposit_agreed) — errors clear on input change. No more full-form error replacement.

**Returning registered users**: 5-second countdown auto-redirect to deposit/ticket page, with "Continue now" and "Share Event" buttons.

**iOS QR save**: Detects iOS Safari → opens QR in new tab with dark background + hint to long-press → Save to Photos. Desktop/Android uses standard `<a download>`.

Image storage: Cloudflare R2 (free tier: 10 GB storage, 10M reads/month).

## Implementation Phases

### Phase 1 — Quasar Escrow Program (~3 days)

- [x] Install Quasar CLI: `cargo install --path cli` (from source)
- [x] `quasar init bethere-escrow --toolchain solana --framework quasarsvm-rust --template full`
- [x] `EventEscrow` + `AttendeeDeposit` account structs (zero-copy, `#[account(discriminator = N, set_inner)]`, `#[seeds(...)]`)
- [x] `create_event` instruction — init escrow PDA + vault token account
- [x] `deposit` instruction — USDC SPL token transfer to vault via `quasar-spl`
- [x] `mark_checked_in` instruction — organizer authority, set `checked_in = true`
- [x] `refund` instruction — attendee claims USDC after event_end + checked_in (PDA-signed CPI)
- [x] `claim_forfeited` instruction — organizer claims unclaimed USDC after refund_deadline
- [x] `close_event` instruction — close escrow + vault, reclaim rent
- [x] Unit tests with QuasarSVM (no validator needed, pure Rust) — 16/16 passing
- [ ] Deploy to devnet: `quasar deploy --url devnet` — deployed to `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo`
- [x] Note: fallback to Anchor is mechanical if Quasar beta blocks us — Quasar compiles clean (58KB .so)

### Phase 2 — Worker Deposit/Refund API (~3 days)

- [x] Add deposit config fields to `EventConfig`
- [x] `GET /api/deposit/status/{attendee_id}` — check deposit status
- [x] `POST /api/deposit/usdc` — build Solana Pay deposit TX
- [x] Slip upload endpoint (KV storage)
- [x] Slip verify/reject endpoint (admin only)
- [x] Refund queue endpoints
- [x] KV schema for THB deposit tracking

### Phase 3 — Frontend Deposit/Refund Flow (~3 days)

- [x] Deposit page (`/deposit/{attendee_id}`)
- [x] Solana Pay QR generation (USDC path)
- [x] Wallet adapter frontend (Phantom/Backpack/Solflare/Coinbase) — Phase 5.3
- [x] Direct TX signing + sending via connected wallet — Phase 5.3
- [x] Deposit status polling + confirmation (2s interval, 30 attempts) — Phase 5.4
- [x] On-chain deposit verification via Solana RPC (`getSignatureStatuses`) — Phase 5.4
- [x] Deposit confirmed view with Solscan TX link — Phase 5.4
- [x] PromptPay QR display + slip upload (THB path)
- [x] Claim page: "Claim Refund" button (USDC auto-refund)
- [x] Admin: Deposits tab (all deposit statuses)
- [x] Admin: Refund Queue tab (THB refund management)
- [x] Admin: Slip verification UI

### Phase 4 — Integration + Devnet E2E (~2 days)

- [ ] End-to-end: register → deposit USDC → check-in → claim refund → verify on-chain
- [ ] End-to-end: register → deposit THB → upload slip → admin verify → check-in → admin refund
- [ ] Test no-show scenario: deposit → don't check in → organizer claims forfeited
- [ ] Test edge cases: double deposit, refund before event ends, refund after deadline

### Phase 5 — Mainnet (~2 days)

- [ ] Security review of escrow program
- [ ] Mainnet deploy (program + worker)
- [ ] USDC mainnet mint: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`
- [ ] Run next Solana Thailand event on BeThere with deposits

## Effort Estimate

| Phase | Days | Notes |
|-------|------|-------|
| Phase 1 — Quasar program | 3 | Greenfield, Quasar escrow (zero-copy, Mollusk tests) |
| Phase 2 — Worker API | 3 | Extends existing worker |
| Phase 3 — Frontend | 3 | Extends existing Leptos app |
| Phase 4 — E2E testing | 2 | Devnet integration |
| Phase 5 — Mainnet | 2 | Deploy + security review |
| **Total** | **13 days** | ~2.5 weeks part-time |

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| USDC vault drained by smart contract bug | Low | Devnet testing, max deposit cap per event, timelock on `claim_forfeited` |
| Quasar beta breaks API / missing feature | Medium | Fallback: mechanical port to Anchor (same `#[derive(Accounts)]`, same constraints, 2 days). Off-chain code is framework-agnostic |
| Attendee can't figure out Solana Pay | Medium | Fallback to PromptPay THB path (no wallet needed at registration) |
| PromptPay slip fraud (photoshopped) | Low | Organizer recognizes name + amount; for larger events, add OCR auto-verify later |
| FX rate changes between deposit and refund | None | USDC deposits refund exact USDC. THB deposits refund exact THB. No cross-currency. |
| Refund deadline passes before attendee claims | Medium | Clear UI messaging: "Claim your refund before [date]". Email reminder 48h before deadline. |

## Refs

- Original escrow plan: `.issues/001_deposit_commitment_refund.md`
- Event config model: `domain/src/models/event.rs`
- Claim flow: `worker/src/handlers/claim.rs`, `frontend-leptos/src/pages/claim.rs`
- cNFT minting scripts: `scripts/cnft/`
- USDC mint (mainnet): `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`
- USDC mint (devnet): `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`
- [Quasar docs](https://quasar-lang.com/docs) — framework reference
- [Quasar: Build an Escrow](https://quasar-lang.com/docs/guides/build-an-escrow) — step-by-step guide
- [Quasar: SPL Token](https://quasar-lang.com/docs/spl-tokens/token-program) — token CPI reference
- [Quasar: Migrating from Anchor](https://quasar-lang.com/docs/getting-started/migrating-from-anchor) — concept mapping
- [Quasar GitHub](https://github.com/blueshift-gg/quasar) — source (beta, no crates.io release yet)

## Status

🟢 Phase 1–2 complete — escrow program on devnet, worker API done.
🟢 Phase 3 complete — USDC wallet adapter + on-chain confirmation + PromptPay QR + slip upload + admin deposit/refund UI + claim page inline refund button.
🟢 Phase 4 complete — Full 5-step escrow flow validated on devnet (create_vault_ata → create_event → deposit → mark_checked_in → refund). 37/37 unit tests pass, clippy clean.
🟢 Phase 5a — Security audit + hardening complete (22/22 tests, all HIGH findings fixed).
🟡 Phase 5b — Deploy hardened program to devnet, then mainnet.

### Key Commits
| Commit | Description |
|--------|-------------|
| `69d0f0d` | PromptPay QR + file upload for THB + USDC refund TX builder (Phase 3/4) |
| `a813585` | Deposit info on claim page — show deposit status + link after NFT claim |
| `a64f3f2` | Dev-mode auth bypass (DEV_MODE=1) + docs update |
| `42a1bd7` | Fix `find_program_address` — add missing `ProgramDerivedAddress` suffix |
| `44025d9` | Fix `is_on_ed25519_curve` — replace hand-rolled field arithmetic with `curve25519-dalek` |
| `325b737` | Wallet adapter frontend + on-chain deposit confirmation (Phase 5.3-5.4) |
| `21eb1b2` | E2E auth fixes, separate USDC/THB attendee IDs, correct devnet USDC mint |
| `d2a5b3e` | Fix refund TX builder 6-account ordering bug + E2E script fixes (Phase 4) |
| `18685a1` | Fix verify_tx_on_chain: add searchTransactionHistory + debug logging |
| `3b2fdab` | Reject USDC deposits after event has ended |
| `4670d6a` | Refactor: extract build_message_accounts helper (-257 lines) |

See `.handovers/035_fix_refund_tx_builder_account_ordering.md` for Phase 4 E2E validation details.
See `.handovers/034_fix_illegal_owner_dual_instruction_bug.md` for create_vault_ata/create_event fixes.
See `.handovers/033_complete_escrow_tx_builders.md` for all TX builder implementations.
