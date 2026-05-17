# 020 — Event Cancellation UI & Batch Refund

## Summary

The escrow program supports deactivating events and refunding individual depositors, but there's no UI workflow for cancellation. Organizers must refund attendees one by one. This issue adds a "Cancel Event" flow that orchestrates deactivation + batch refund + cleanup.

## Problem

### No cancellation workflow

Currently, canceling an event requires:
1. Admin calls `POST /api/escrow/deactivate-event` (stops new deposits)
2. For EACH USDC depositor, the attendee must visit the refund page and sign their own TX
3. For EACH THB depositor, admin marks refund one at a time
4. After all refunds settle, admin calls `POST /api/escrow/close-event`

There's no orchestration, no progress tracking, no single "Cancel Event" button.

### On-chain constraint: attendee signs refunds

The `refund` instruction requires the **attendee** as signer — the organizer cannot unilaterally refund. This is by design (anti-rug-pull: organizer cannot steal funds). But it means:
- USDC refunds: organizer must broadcast cancellation → each depositor claims refund individually
- THB refunds: admin CAN batch-mark (pure KV state change, no on-chain component)

### Current escrow instructions

```
deactivate_event  — organizer signs, stops new deposits
refund            — attendee signs, returns USDC from vault
close_deposit     — attendee signs, reclaims rent from PDA
claim_forfeited   — organizer signs, claims forfeited deposits after deadline
close_event       — organizer signs, closes vault + PDA (requires vault balance = 0)
```

## Proposed Solution

### A. Cancellation API — orchestration endpoint

```
POST /api/escrow/cancel-event
```

Steps:
1. Validate: event has `escrow_status == Initialized` and is not already deactivated
2. Build `deactivate_event` TX for organizer to sign
3. Return the TX + a summary of affected deposits (USDC count, THB count, total amounts)
4. After deactivation is confirmed:
   - Batch-mark all THB deposits as refunded (KV update)
   - Build individual `refund + close_deposit` TXs for USDC depositors
   - Return all TXs for the frontend to present as a queue

### B. THB batch refund endpoint

```
POST /api/refund/batch-thb
Body: { event_id: string }
```

- Scans all THB deposits for the event
- Marks each as `refunded = true, refunded_at = now()`
- Returns summary: `{ refunded: N, total_amount: X }`
- This is a pure KV operation — instant, no on-chain TX needed

### C. USDC refund queue endpoint

```
GET /api/escrow/refund-queue?event_id=xxx
```

- Lists all USDC deposits with `verified = true`, `refundable = true`, not yet refunded
- For each: attendee_id, email, wallet_address, deposit_amount
- Used by admin UI to show who still needs to claim their refund

### D. Frontend cancellation workflow

**Step 1: Confirm cancellation**
- Admin clicks "Cancel Event" → modal shows impact: "X USDC depositors, Y THB depositors, Z total USDC, W total THB"
- Admin confirms → signs `deactivate_event` TX

**Step 2: Batch THB refund**
- After deactivation confirmed, automatically batch-refund all THB deposits
- Show progress: "Refunded 15/15 THB deposits"

**Step 3: USDC refund queue**
- Show list of USDC depositors who need to claim refunds
- Generate cancellation message with claim link for each depositor
- Status tracking: "5/12 USDC refunds claimed"

**Step 4: Close event**
- After all refunds settle, show "Close Event" button
- Admin signs `close_event` TX → escrow PDA + vault closed
- Event status updated to `Cancelled`

### E. Attendee-facing refund claim page

For USDC depositors, enhance the existing deposit page to show:
- "This event has been cancelled. Claim your refund."
- One-click refund (already exists — just needs better messaging)

## Files to Create/Modify

| File | Change |
|------|--------|
| `worker/src/handlers/deposit.rs` | Add `cancel_event_handler`, `batch_thb_refund_handler`, `refund_queue_handler` |
| `worker/src/handlers/mod.rs` | Register cancellation routes on admin router |
| `frontend-leptos/src/pages/admin.rs` | Cancellation workflow UI (confirm → deactivate → THB batch → USDC queue → close) |
| `frontend-leptos/src/pages/deposit.rs` | Show cancellation messaging when event is deactivated |
| `domain/src/models/event.rs` | Add `Cancelled` variant to `EscrowStatus` |

## Acceptance Criteria

- [ ] "Cancel Event" button in admin dashboard (visible when escrow is active)
- [ ] Confirmation modal shows impact summary before proceeding
- [ ] `deactivate_event` TX built and signed by organizer
- [ ] THB deposits batch-refunded automatically after deactivation
- [ ] USDC refund queue visible with per-depositor status
- [ ] Attendees see cancellation message with refund CTA on deposit page
- [ ] Event can be closed after all refunds settle
- [ ] Cancellation updates `escrow_status` to `Cancelled`

## Limitations

- **USDC refunds still require attendee signature** — this is an on-chain constraint. The organizer cannot force-refund. The UI provides a queue and messaging, but each attendee must claim individually.
- **Future improvement**: Add `organizer_refund` instruction to the escrow program allowing the organizer to initiate refunds for attendees (requires program modification + redeployment).

## Dependencies

- Escrow program deployed on-chain — ✅
- `worker/src/solana_escrow.rs` — TX builders for all instructions — ✅
- Admin wallet connection — ✅

## Refs

- `docs/escrow_protocol.md` — Q6 (cancel_event)
- `bethere-escrow/src/lib.rs` — on-chain program instructions
- `worker/src/solana_escrow.rs` — TX builders
- `.issues/013_escrow_rug_pull_prevention.md` — anti-rug-pull design (attendee signs refund)
