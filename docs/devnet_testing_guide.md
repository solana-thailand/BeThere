# BeThere Escrow — Devnet Testing Guide

> Complete walkthrough to test the full escrow lifecycle on devnet before mainnet deployment.

## Prerequisites

| Requirement | How |
|---|---|
| **Phantom or Solflare wallet** | Install browser extension, switch to **Devnet** network |
| **Devnet SOL** | Airdrop from [faucet.solana.com](https://faucet.solana.com) or `solana airdrop 2 <YOUR_WALLET>` |
| **Devnet USDC** | Use the [Circle USDC faucet](https://faucet.circle.com/) or swap SOL → USDC on a devnet DEX |
| **BeThere admin access** | Log in at the admin dashboard (dev mode email or Google auth) |
| **Worker deployed to devnet** | `npx wrangler dev` or deployed worker with `DEV_MODE=1` |

### Key Devnet Addresses

| Item | Address |
|---|---|
| Escrow Program | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |
| USDC Mint (devnet) | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` |
| Solscan (devnet) | `https://solscan.io/tx/{SIG}?cluster=devnet` |
| Solana Explorer (devnet) | `https://explorer.solana.com/tx/{SIG}?cluster=devnet` |

---

## Test Flow Overview

There are **two separate flows** to test:

### Flow A: Event Setup (Events Page)
Create an event → connect wallet → create vault ATA → create event escrow

### Flow B: Escrow Lifecycle Management (Admin Escrow Page)
Deactivate event → claim forfeited → close event

---

## Flow A: Event Setup with Escrow

### Step 1: Create a Test Event

1. Open the admin dashboard
2. Click **"📋 Events"** in the sidebar
3. Click **"+ Create Event"** button
4. Fill in the form:
   - **Event Name**: `Devnet Test Event`
   - **Slug**: `devnet-test-1` (auto-generated, editable)
   - **Event Start**: Set to a future date (e.g., tomorrow)
   - **Event End**: Set to a few hours after start
   - **Deposit Enabled**: ✅ Check this box
   - **Deposit Amount (USDC)**: `1.0`
   - **Refund Deadline Hours**: `48` (hours after event end)
5. Click **"Save Event"**
6. ✅ Verify: Event appears in the events list

### Step 2: Initialize On-Chain Escrow

1. Find your new event in the events list
2. Click the **edit (pencil)** button on the event
3. Scroll down to the **"⛓ On-Chain Escrow Setup"** section
4. **Step 2a — Connect Wallet:**
   - Click **"🔗 Connect Phantom"** (or your wallet name)
   - Approve the connection request in your wallet popup
   - ✅ Verify: Shows wallet name + truncated public key (e.g., `Phantom (AxK3...`)
5. **Step 2b — Create Vault ATA:**
   - Click **"🏦 Create Vault Token Account"**
   - Approve the transaction in your wallet
   - ✅ Verify: Shows "✅ Vault ATA created" with vault address + signature
   - 🔍 Click the Solscan link to verify on-chain
6. **Step 2c — Create Event Escrow:**
   - Click **"⛓ Initialize Event Escrow"**
   - Approve the transaction in your wallet
   - ✅ Verify: Shows "✅ Escrow created!" with escrow address + signature
   - 🔍 Verify the escrow fields auto-populated: `escrow_address` + `on_chain_event_id`
7. Click **"Save Event"** to persist the escrow data
8. ✅ Verify: Event is saved with escrow_address and on_chain_event_id filled

### Step 3: Verify Escrow On-Chain

Open Solana Explorer and check:
- The `EventEscrow` PDA account exists
- Fields: `organizer = your wallet`, `is_active = true`, `deposit_amount = 1_000_000` (1 USDC = 1M lamports)
- Vault token account is created and empty (0 USDC)

```bash
# Alternative: CLI verification
solana program show C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T --url devnet
```

---

## Flow B: Escrow Lifecycle Management

### Step 4: Navigate to Escrow Management

1. Go back to admin dashboard (click **"📊 Attendance"** or logo)
2. Select your test event from the event dropdown at the top
3. Click **"⛓ Escrow Management"** in the sidebar
4. ✅ Verify: Shows "⛓ Escrow Management" header with 3 lifecycle cards

### Step 5: Connect Organizer Wallet

1. Click **"🔗 Connect Phantom"** (or your wallet)
2. Approve connection in wallet popup
3. ✅ Verify: Shows wallet bar with name + public key + Disconnect button
4. ✅ Verify: 3 lifecycle cards are visible (Deactivate, Claim Forfeited, Close)

### Step 6: Deactivate Event

**What this does:** Marks `is_active = false` on the EventEscrow PDA. No new deposits accepted. Refunds still allowed.

1. Click **"⚡ Sign TX"** on **Step 1: Deactivate**
2. ✅ Verify: Button changes to spinner "Signing..."
3. Approve the transaction in your wallet popup
4. ✅ Verify: Card border turns green, shows "✅ Done"
5. ✅ Verify: Success banner with Solscan link appears above the cards
6. 🔍 Click Solscan link → verify `deactivate_event` instruction (discriminator `5`) succeeded

**Edge case to try:** Click "Sign TX" again → should still work (idempotent deactivation)

### Step 7: Claim Forfeited

**What this does:** Transfers all forfeited USDC from vault to organizer's USDC token account. Does **not** close the vault — that is done by `close_event` (Step 8). Only works after `refund_deadline`.

> ⚠️ **Devnet timing note:** The refund_deadline is `event_end + refund_deadline_hours`. If your event hasn't ended yet, this transaction will fail with a constraint error. Either:
> - Wait for the event to end + refund_deadline to pass
> - Or edit the event to set `event_end` to a past time and `refund_deadline_hours` to `0`

1. Click **"⚡ Sign TX"** on **Step 2: Claim Forfeited**
2. ✅ Verify: Spinner appears
3. Approve in wallet
4. ✅ Verify: Green border + "✅ Done" + Solscan link
5. 🔍 Verify on-chain: USDC transferred to your wallet's USDC token account

### Step 8: Close Event

**What this does:** Closes the EventEscrow PDA account. Reclaims rent (SOL). Account is zeroed out.

1. Click **"⚡ Sign TX"** on **Step 3: Close Event**
2. ✅ Verify: Spinner appears
3. Approve in wallet
4. ✅ Verify: Green border + "✅ Done" + Solscan link
5. 🔍 Verify on-chain: EventEscrow PDA account no longer exists (shows "Account does not exist")

---

## Additional Scenarios to Test

### Scenario: Disconnect & Reconnect

1. After connecting wallet, click **"Disconnect"**
2. ✅ Verify: Wallet bar disappears, "Connect" buttons reappear
3. Connect again → ✅ Verify: Wallet reconnects, no errors

### Scenario: Wrong Wallet

1. Connect a different wallet than the one that created the escrow
2. Try to deactivate → ❌ Should fail with "organizer mismatch" error
3. ✅ Verify: Error banner appears with red border

### Scenario: No Event Selected

1. Go to Escrow Management without selecting an event
2. ✅ Verify: Shows "Select an event with escrow enabled to manage."

### Scenario: Order Enforcement

1. Skip deactivation and try Claim Forfeited first
2. ❌ Should fail if event is still active
3. ✅ Verify: Error banner with constraint message

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "No Solana wallet detected" | Install Phantom/Solflare extension, ensure it's enabled |
| Wallet popup doesn't appear | Check browser popup blocker, try refreshing page |
| TX rejected in wallet | Ensure wallet is on **Devnet** network, check you have enough SOL for tx fee |
| "organizer mismatch" error | You connected the wrong wallet. Disconnect and use the organizer wallet |
| "constraint failed: is_active" | Event already deactivated, skip to next step |
| "refund deadline not passed" | Wait for event_end + refund_deadline_hours, or edit event dates |
| Worker returns 500 | Check worker logs: `npx wrangler tail` |
| Frontend not loading | Ensure `trunk build` ran successfully, check browser console |

---

## Flow C: Attendee Deposit → Refund → Reclaim Rent

> Tests the full attendee-facing escrow experience including the new Phase 4 rent reclamation.

### Step 9: Open Deposit Page

1. Navigate to the deposit page for your test event (use the deposit link from the events page or go directly to `/deposit?event=<event_id>`)
2. ✅ Verify: Page shows deposit info (amount, event name)
3. Enter your attendee ID and the email/name used for registration
4. Click **"Check Deposit Status"**
5. ✅ Verify: Shows "No deposit found" or deposit status

### Step 10: Connect Wallet & Deposit

1. Click **"🔗 Connect Phantom"** (or your wallet)
2. Approve the connection request
3. ✅ Verify: Shows wallet name + truncated public key
4. Click **"💰 Deposit USDC"**
5. Approve the transaction in your wallet popup
6. ✅ Verify: Shows "✅ Deposit confirmed" with Solscan link

### Step 11: Mark Attendee as Checked In

> The organizer must mark the attendee as checked in before refund is possible.

1. Open the scanner/check-in page for the event
2. Scan the attendee's QR code or manually check them in
3. ✅ Verify: Attendee shows as checked in

### Step 12: Refund Deposit

1. Return to the deposit page
2. The status should now show **"Already Deposited"** with checked-in status
3. Click **"💸 Refund Deposit"**
4. Approve the transaction in your wallet
5. ✅ Verify: Shows "✅ Refund confirmed" with Solscan link
6. 🔍 Verify on Solscan: USDC returned to your wallet's token account

### Step 13: Reclaim Rent (Phase 4 — SEC-010)

**What this does:** Closes the `AttendeeDeposit` PDA, reclaiming ~0.002 SOL of rent-exempt balance.

1. After refund confirmation, a **"♻️ Reclaim Rent"** button appears
2. Click **"♻️ Reclaim Rent"**
3. ✅ Verify: Shows wallet connection prompt
4. Connect the same wallet used for deposit
5. Click **"♻️ Close Deposit Account"**
6. Approve the transaction in your wallet
7. ✅ Verify: Shows "✅ Rent reclaimed" with Solscan link
8. 🔍 Verify on Solscan: `close_deposit` instruction (discriminator `7`) succeeded
9. 🔍 Verify: AttendeeDeposit PDA no longer exists on-chain

### Alternative: Reclaim Rent from Already Deposited View

If you've already refunded and return to the deposit page later:

1. Open the deposit page and check status
2. Status shows **"Already Deposited"** (refunded: true)
3. The **"♻️ Reclaim Rent"** button is also visible here
4. Follow the same flow as Step 13

---

## Flow D: Rollover Deposit

> Tests the atomic deposit rollover from one event to another.

### Prerequisites

- Two events with escrow enabled (Event A with an existing deposit, Event B as rollover target)
- Attendee has a deposited + refunded (or forfeited) deposit on Event A

### Step 14: Rollover Deposit to New Event

1. On the deposit page for Event A (after refund deadline or refund), click **"🔄 Rollover to Next Event"**
2. Select Event B as the rollover target
3. Connect wallet and approve the transaction
4. ✅ Verify: Shows "✅ Deposit rolled over" with Solscan link
5. 🔍 Verify on-chain: `rollover_deposit` instruction transferred USDC from Event A's vault to Event B's vault
6. 🔍 Verify: New `AttendeeDeposit` PDA created for Event B with correct amount

### Edge Cases to Test

- Rollover to event with different deposit amount (should fail or handle partial)
- Rollover when no target event is configured
- Rollover when source vault is empty (already claimed by organizer)

---

## Quick CLI Verification Commands

```bash
# Check program is deployed
solana program show C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T --url devnet

# Check your wallet balance
solana balance <YOUR_WALLET> --url devnet

# Check your USDC balance (need spl-token CLI)
spl-token accounts --url devnet

# Airdrop devnet SOL
solana airdrop 2 <YOUR_WALLET> --url devnet

# Watch worker logs
cd worker && npx wrangler tail
```

---

## Checklist (Copy & Paste)

```
Devnet Testing Checklist
========================
[ ] Phantom/Solflare installed and on Devnet
[ ] Devnet SOL airdropped to wallet
[ ] Devnet USDC obtained from Circle faucet

Flow A: Event Setup
[ ] Test event created with deposit enabled
[ ] Wallet connected in Events page
[ ] Vault ATA created (Step 2a)
[ ] Event escrow initialized (Step 2b)
[ ] Escrow fields saved in event config
[ ] Escrow verified on Solscan/Explorer

Flow B: Admin Escrow Lifecycle
[ ] Escrow Management page opened
[ ] Wallet connected in Escrow Management
[ ] Deactivate event succeeded (Step 6)
[ ] Claim forfeited succeeded (Step 7)
[ ] Close event succeeded (Step 8)
[ ] Wrong wallet rejection tested
[ ] Order enforcement tested

Flow C: Attendee Deposit Lifecycle
[ ] Deposit page opened and status checked (Step 9)
[ ] Wallet connected and USDC deposited (Step 10)
[ ] Attendee checked in by organizer (Step 11)
[ ] Deposit refunded after check-in (Step 12)
[ ] Rent reclaimed via close_deposit (Step 13)
[ ] AttendeeDeposit PDA verified closed on Solscan

All Links
[ ] All Solscan links verified (correct cluster=devnet)
```
