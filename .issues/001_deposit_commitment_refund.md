# Issue 001: Deposit Commitment & Refund System

## Problem

BeThere currently only offers free check-in + NFT minting. This doesn't solve the real organizer pain point: **no-shows waste money** (food, venue, swag for people who register but don't attend).

## Proposed Solution

A **deposit-commitment escrow system** on Solana:

### Flow
1. **Commit** — Attendee locks a SOL deposit (set by organizer) to reserve a spot
2. **Show Up** — Staff scans QR at event → on-chain check-in verification
3. **Get Refunded** — Smart contract auto-refunds deposit after check-in
4. **Keep NFT** — Compressed NFT as proof of attendance (existing feature)

If attendee **doesn't show up** → deposit forfeited to organizer (or protocol treasury).

### Smart Contract Architecture

```
Escrow Program (Solana)

Accounts:
- EventEscrow (PDA) — holds deposits, configured by organizer
  - deposit_amount: u64 (lamports)
  - organizer: Pubkey
  - event_start: i64
  - event_end: i64
  - refund_deadline: i64 (event_end + grace period)
  - total_deposited: u64
  - total_refunded: u64
  - total_forfeited: u64

- AttendeeDeposit (PDA) — per-attendee deposit record
  - attendee: Pubkey
  - event: Pubkey (EventEscrow)
  - amount: u64
  - deposited_at: i64
  - checked_in: bool
  - refunded: bool

Instructions:
- create_event(organizer, deposit_amount, start, end) → EventEscrow
- deposit(attendee, event) → AttendeeDeposit + transfer SOL to EventEscrow
- check_in(attendee, event) → mark AttendeeDeposit.checked_in = true
- refund(attendee, event) → transfer SOL back if checked_in
- claim_forfeited(organizer, event) → after refund_deadline, claim all unclaimed deposits
```

### Design Decisions Needed

| Decision | Options | Recommendation |
|----------|---------|---------------|
| Deposit currency | SOL only, SPL token, or both | Start with SOL only (simplest) |
| Deposit amount | Fixed per event, or tiered | Fixed per event (organizer sets) |
| Refund mechanism | Auto (CPI on check-in) or claim-based | Claim-based (attendee claims refund post-event) |
| Cancellation window | Before event starts? | Yes — organizer-configurable cutoff |
| Escrow authority | Organizer multisig or program PDA | Program PDA (trustless) |
| Forfeited funds | To organizer, or split with protocol | To organizer (simplest for v1) |

### Integration Points with Existing System

| Component | Current | After |
|-----------|---------|-------|
| Registration | Google Sheet import | Sheet import + optional deposit requirement |
| Check-in | Scanner verifies name in sheet | Same scanner → also marks on-chain deposit as checked_in |
| Claim page | Free NFT mint | Refund claim + NFT mint (combined transaction) |
| Wallet | Paste address (post-event) | Wallet Standard connection (pre-event, for deposit) |

### Implementation Phases

**Phase 1 — Program (Anchor/Rust)**
- [ ] Escrow program with deposit/check_in/refund/claim_forfeited instructions
- [ ] Unit tests with LiteSVM or Mollusk
- [ ] Deploy to devnet

**Phase 2 — Backend Integration**
- [ ] Add `deposit_required` + `deposit_amount` to event config
- [ ] Worker API: `POST /deposit` → build transaction for frontend to sign
- [ ] Worker API: `POST /refund` → build refund transaction
- [ ] Check-in scanner also calls `check_in` instruction on-chain

**Phase 3 — Frontend**
- [ ] Wallet Standard connection (replace paste-address flow)
- [ ] Deposit flow: connect wallet → sign transaction → confirm
- [ ] Claim page: combined refund + NFT mint in single transaction
- [ ] Admin: see deposit status per attendee (deposited / checked-in / refunded / forfeited)

**Phase 4 — Production**
- [ ] Mainnet deploy
- [ ] Security audit (escrow holds real funds)
- [ ] Organizer documentation

## Risks

| Risk | Mitigation |
|------|-----------|
| Smart contract vulnerability (real SOL at stake) | Audit, timelock, max deposit cap |
| Attendee doesn't have Solana wallet pre-event | Fallback: free registration (no deposit) option |
| Refund fails (insufficient rent-exempt balance) | Program checks rent-exempt minimum |
| Organizer sets unreasonable deposit | Protocol-level max cap (e.g., 0.5 SOL) |

## Estimated Effort

| Phase | Time |
|-------|------|
| Phase 1 — Program | 3-5 days |
| Phase 2 — Backend | 2-3 days |
| Phase 3 — Frontend | 3-4 days |
| Phase 4 — Production | 2-3 days |
| **Total** | **10-15 days** |

## Refs

- Existing claim flow: `frontend-leptos/src/pages/claim.rs`
- Scanner check-in: `frontend-leptos/src/pages/scanner.rs`
- Worker API: `worker/src/`
- Domain layer: `domain/src/`
