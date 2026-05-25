# Issue #032: Rolling Deposit Credit

## Summary

Allow attendees to **hold their deposit** after check-in instead of claiming a refund. The held deposit becomes a **credit** that automatically covers future event deposits — no re-payment needed. This works for both THB (cash) and USDC (on-chain) deposit methods.

## Motivation

1. **Thai Baht friction**: Cash deposits are collected at the door, and refunding cash is awkward (change, receipts, tracking). Rolling deposit eliminates repeated cash exchange for repeat attendees.
2. **Loyalty/retention**: Attendees who show up and prove trustworthy keep "skin in the game" — their deposit rolls forward as a commitment token.
3. **Operational simplicity**: Organizers hold the cash once, BeThere tracks the credit digitally. No re-collection at future events.

## User Stories

**Attendee:**
- As a repeat attendee, I want to hold my deposit after check-in so I don't need to pay again for the next event.
- As an attendee, I want to see my deposit credit balance on my profile/ticket page.
- As an attendee, I want to request a full refund of my held deposit credit at any time.

**Organizer:**
- As an organizer, I want to see which attendees have deposit credit and how much.
- As an organizer, I want to know (per-event) which registrations were covered by credit vs. new payment.

## Architecture: Off-chain First

### Phase 1: Off-chain tracking (this issue)

Track deposit credit in the Google Sheets. No smart contract changes.

### Contacts Sheet (Master) — New Columns

Current: A–J (10 columns)
Proposed: A–M (13 columns)

| Column | Header | Example | Description |
|--------|--------|---------|-------------|
| K | `deposit_credit_thb` | `500` | THB credit balance (integer) |
| L | `deposit_credit_usdc` | `10` | USDC credit balance (smallest unit) |
| M | `deposit_credit_since` | `2026-05-23` | Date credit was first established |

### Attendees Sheet (Per-Event) — Reuse Existing Column

`deposit_method` (column N) already exists. Add new values:

| Value | Meaning |
|-------|---------|
| `usdc` | On-chain USDC deposit (existing) |
| `thb` | Thai Baht cash deposit (existing) |
| `credit_thb` | Covered by held THB credit (new) |
| `credit_usdc` | Covered by held USDC credit (new) |

The `deposit_amount` column still records the amount — just the *source* changes.

### Flow

```
Registration:
1. Attendee registers for event
2. Backend checks contacts sheet: does this email have credit >= required deposit?
3. If yes → skip deposit step, write "credit_thb"/"credit_usdc" to attendee's deposit_method
4. If no → normal deposit flow (USDC or THB)

After check-in (ticket page):
- Show two buttons: "Claim Refund" (existing) | "Hold Deposit" (new)
- "Hold Deposit" → increment contact's credit balance → mark deposit as held
- "Claim Refund" → existing refund flow → reset credit

Exit (any time):
- Attendee requests "Return My Deposit" → organizer refunds held balance → credit reset to 0
```

### Code Changes

**Backend (worker):**
- [x] Add columns K–M to contacts sheet schema (`contacts.rs`)
- [x] Add `credit_thb` / `credit_usdc` to `DepositMethod` enum (`domain/models/deposit.rs`)
- [x] New endpoint: `POST /api/deposit/hold` — marks deposit as held, updates contact credit
- [x] New endpoint: `GET /api/deposit/credit-balance` — returns attendee's current credit
- [x] Upsert contact after "hold" action: increment credit balance
- [ ] Registration handler: check credit before requiring deposit

**Frontend (frontend-leptos):**
- [ ] Ticket page: show "Hold Deposit" button after check-in (alongside "Claim Refund")
- [ ] Ticket page: show credit balance if > 0
- [ ] Registration: if credit covers deposit, show "Deposit covered by your credit" instead of deposit step
- [x] Deposit page: handle `credit_thb`/`credit_usdc` method display

**Domain:**
- [x] Extend `DepositMethod` enum with `CreditThb`, `CreditUsdc` variants
- [x] Update serde tests for new variants

### Phase 2: On-chain vault (future issue)

- Single `AttendeeVault` PDA per attendee (not per event)
- All deposits go to vault instead of event-specific escrow
- "Hold deposit" = no refund instruction called
- Requires smart contract changes — separate issue

## Affected Files

| File | Change |
|------|--------|
| `domain/src/models/deposit.rs` | Add `CreditThb`, `CreditUsdc` to `DepositMethod` |
| `domain/src/models/attendee.rs` | No structural change (deposit_method is `Option<String>`) |
| `worker/src/sheets/contacts.rs` | Add columns K–M, update upsert logic |
| `worker/src/handlers/register.rs` | Check credit before requiring deposit |
| `worker/src/handlers/deposit.rs` | New "hold" endpoint |
| `frontend-leptos/src/pages/ticket.rs` | "Hold Deposit" button + credit display |
| `frontend-leptos/src/pages/deposit.rs` | Credit-covered deposit display |
| `frontend-leptos/src/api/types.rs` | New `DepositMethod` variants |

## Risks / Open Questions

1. **Currency mismatch**: Credit is THB but next event requires USDC (or vice versa) — how to handle? Options: convert at organizer's rate, or keep separate balances (current proposal keeps them separate).
2. **Partial credit**: Credit is 500 THB but next event only requires 200 THB — does it deduct partially? Or only if credit >= required amount?
3. **Organizer liability**: Organizer holds physical cash for THB credits. If organizer goes bankrupt, attendees lose their deposit. Should we cap max credit or add a timeout?
4. **Multi-organizer**: If different organizers run different events, credit with Org A shouldn't cover Org B's deposit. This ties into Issue #029 (per-org contacts).

## Dependencies

- Issue #030 (Master Contacts Sheet) — ✅ activated
- Issue #029 (Per-Org Contacts) — optional, for multi-organizer credit isolation
