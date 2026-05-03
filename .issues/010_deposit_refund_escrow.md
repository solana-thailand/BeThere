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
│   → Admin transfers 500 THB via PromptPay banking app       │
│   → Clicks "Mark Refunded"                                 │
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

## On-Chain Program: Escrow (Anchor)

### Accounts

```rust
/// PDA — one per event, holds all USDC deposits in a token account.
#[account]
pub struct EventEscrow {
    /// Event organizer (can claim forfeited deposits).
    pub organizer: Pubkey,
    /// USDC mint address (e.g., EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m on mainnet).
    pub usdc_mint: Pubkey,
    /// Token account owned by this PDA — holds deposited USDC.
    pub vault: Pubkey,
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
#[account]
pub struct AttendeeDeposit {
    /// Attendee's wallet address.
    pub attendee: Pubkey,
    /// Reference to EventEscrow.
    pub event: Pubkey,
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
AttendeeDeposit: ["deposit", event_escrow.key().as_bytes(), attendee.key().as_bytes()]
```

### Access Control

- `deposit`: anyone (attends open event)
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
| `POST` | `/api/refund/mark/{attendee_id}` | Mark THB refund as done (admin) |

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
  "refunded_at": null
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
- "Refund Queue" tab: checked-in THB depositors awaiting refund, "Mark Refunded" button
- "Escrow" tab: on-chain stats (total deposited, total refunded, total forfeited, available for claim)

### PromptPay Slip Verification (Free)

For MVP, no banking API needed:

1. Attendee scans PromptPay QR (static QR for organizer's bank)
2. Attendee uploads screenshot of payment confirmation
3. Admin sees slip in dashboard, verifies amount + name match
4. Admin clicks "Verify" → deposit status updated

Image storage: Cloudflare R2 (free tier: 10 GB storage, 10M reads/month).

## Implementation Phases

### Phase 1 — Anchor Escrow Program (~4 days)

- [ ] Initialize Anchor project in `program/` directory
- [ ] `EventEscrow` + `AttendeeDeposit` account structs
- [ ] `create_event` instruction
- [ ] `deposit` instruction (USDC SPL token transfer to vault)
- [ ] `mark_checked_in` instruction (organizer authority)
- [ ] `refund` instruction (attendee claims after event_end + checked_in)
- [ ] `claim_forfeited` instruction (organizer claims after refund_deadline)
- [ ] Unit tests with `liteSVM` or `mollusk`
- [ ] Deploy to devnet + verify

### Phase 2 — Worker Deposit/Refund API (~3 days)

- [ ] Add deposit config fields to `EventConfig`
- [ ] `GET /api/deposit/status/{attendee_id}` — check deposit status
- [ ] `POST /api/deposit/usdc` — build Solana Pay deposit TX
- [ ] Slip upload endpoint (R2 storage)
- [ ] Slip verify/reject endpoint (admin only)
- [ ] Refund queue endpoints
- [ ] KV schema for THB deposit tracking

### Phase 3 — Frontend Deposit/Refund Flow (~3 days)

- [ ] Deposit page (`/deposit/{attendee_id}`)
- [ ] Solana Pay QR generation (USDC path)
- [ ] PromptPay QR display + slip upload (THB path)
- [ ] Deposit status polling
- [ ] Claim page: "Claim Refund" button (USDC auto-refund)
- [ ] Admin: Deposits tab (all deposit statuses)
- [ ] Admin: Refund Queue tab (THB refund management)
- [ ] Admin: Slip verification UI

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
| Phase 1 — Anchor program | 4 | Greenfield, new Anchor project |
| Phase 2 — Worker API | 3 | Extends existing worker |
| Phase 3 — Frontend | 3 | Extends existing Leptos app |
| Phase 4 — E2E testing | 2 | Devnet integration |
| Phase 5 — Mainnet | 2 | Deploy + security review |
| **Total** | **14 days** | ~3 weeks part-time |

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| USDC vault drained by smart contract bug | Low | Devnet testing, max deposit cap per event, timelock on `claim_forfeited` |
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

## Status

🔵 Not started
