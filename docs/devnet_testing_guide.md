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
| Escrow Program | `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` |
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
solana program show 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo --url devnet
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

**What this does:** Transfers all USDC from vault to organizer's USDC token account. Closes the vault token account. Only works after `refund_deadline`.

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

## Quick CLI Verification Commands

```bash
# Check program is deployed
solana program show 2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo --url devnet

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
[ ] Test event created with deposit enabled
[ ] Wallet connected in Events page
[ ] Vault ATA created (Step 2a)
[ ] Event escrow initialized (Step 2b)
[ ] Escrow fields saved in event config
[ ] Escrow verified on Solscan/Explorer
[ ] Escrow Management page opened
[ ] Wallet connected in Escrow Management
[ ] Deactivate event succeeded (Step 6)
[ ] Claim forfeited succeeded (Step 7)
[ ] Close event succeeded (Step 8)
[ ] Wrong wallet rejection tested
[ ] Order enforcement tested
[ ] All Solscan links verified
```
