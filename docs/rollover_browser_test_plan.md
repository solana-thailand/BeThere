# BeThere Rollover Deposit — Manual Browser Test Plan

> **Issue**: #040 Phase C
> **Version**: 2026-06-09
> **Cluster**: Solana Devnet
> **Escrow Program**: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
> **USDC Mint (Devnet)**: `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`

---

## Overview

This document provides a step-by-step manual browser test plan for the **Rollover Deposit** feature — the ability for a checked-in attendee to atomically move their USDC deposit from a past event's vault to a new event's vault.

The rollover flow involves **two browser roles** and **three frontend pages**:

| Role | Browser | Purpose |
|------|---------|---------|
| **Organizer** | Admin dashboard (`/admin`) | Create events, init escrow, scan QR (check-in), manage escrow lifecycle |
| **Attendee** | Ticket page (`/ticket/:id`) + Deposit page (`/deposit/:id`) | View ticket, pay deposit, sign rollover TX, verify refund |

---

## Prerequisites

| Requirement | How |
|---|---|
| **Two browser sessions** (or incognito + normal) | One for organizer, one for attendee |
| **Phantom or Solflare wallet** | Install extension, switch to **Devnet** |
| **Devnet SOL** (~0.5 SOL) | `solana airdrop 2 <WALLET>` or [faucet.solana.com](https://faucet.solana.com) |
| **Devnet USDC** (≥ 2 USDC) | [Circle USDC faucet](https://faucet.circle.com/) |
| **Worker running locally** | `cd worker && npx wrangler dev --port 8787` |
| **DEV_MODE=1** in `worker/.dev.vars` | Enables dev-mode bypasses for testing |
| **HELIUS_API_KEY** in `worker/.dev.vars` | Required for transaction building |

### Test Wallets

| Role | Wallet | Notes |
|------|--------|-------|
| Organizer | Your primary Phantom wallet | Must have devnet SOL for TX fees |
| Attendee | Same wallet or separate | Must have ≥ 2 devnet USDC for deposits |

> **Tip**: You can use the same wallet for both roles in devnet. The organizer creates the event, then registers as an attendee via the public event page.

---

## Test Matrix

| # | Test Case | Expected Outcome | Pass? |
|---|-----------|------------------|-------|
| R-01 | Source event setup (create + deposit + check-in) | Event created, escrow initialized, deposit paid, attendee checked in | ☐ |
| R-02 | Target event setup (create + escrow init) | Target event created, escrow initialized, same deposit amount | ☐ |
| R-03 | Rollover card appears on ticket page | `RolloverActionCard` visible with target event name | ☐ |
| R-04 | Wallet detection + connect flow | Wallet detected, connect button works | ☐ |
| R-05 | Rollover TX signing + confirmation | TX succeeds, Solscan link shown | ☐ |
| R-06 | Post-rollover: source vault empty | Source event vault balance = 0 | ☐ |
| R-07 | Post-rollover: target vault has deposit | Target event vault has the rolled-over USDC | ☐ |
| R-08 | Post-rollover: source deposit shows refunded | Source ticket page shows refund status | ☐ |
| R-09 | Refund from target event | Attendee gets USDC back from target event after deadline | ☐ |
| R-10 | Full lifecycle cleanup | Both events deactivated + closed, rent reclaimed | ☐ |
| R-11 | No double-rollover (second attempt fails) | Second rollover attempt shows error | ☐ |
| R-12 | Rollover card hidden for non-eligible attendees | No rollover card when no target event or deposit not verified | ☐ |

---

## Flow 1: Source Event Setup (Tests R-01)

### Step 1.1: Create Source Event (Organizer)

1. Open **Browser A** → navigate to `http://localhost:8787/admin`
2. Log in with dev-mode credentials (or Google auth)
3. Click **"📋 Events"** in the sidebar
4. Click **"+ Create Event"**
5. Fill in the form:
   - **Event Name**: `Rollover Source Event`
   - **Slug**: `rollover-src` (auto-generated)
   - **Event Start**: Today (current time + 5 minutes)
   - **Event End**: Today + 1 hour (must be **past** for rollover eligibility)
   - **Deposit Enabled**: ✅ Check
   - **Deposit Amount USDC**: `1` (1 USDC)
6. Click **Create Event**

**Expected**: Event appears in the events list with `escrow_status: none` badge.

### Step 1.2: Initialize Escrow (Organizer)

1. Click on the newly created event in the list
2. In the event detail view, find the **"Initialize Escrow"** section
3. Click **"Connect Wallet"** — select Phantom/Solflare
4. After wallet connects, click **"Initialize Escrow"**
5. Approve the transaction in the wallet popup

**Expected**:
- TX confirmation with Solscan link
- Event badge updates to `escrow_status: initialized`
- Vault ATA created for the event

### Step 1.3: Register Attendee

1. Open **Browser B** → navigate to `http://localhost:8787/e/rollover-src`
2. Fill in registration form (name, email)
3. Submit registration

**Expected**: Redirected to ticket page (`/ticket/{attendee_id}`)

### Step 1.4: Pay Deposit (Attendee)

1. On the ticket page, find the **"Deposit Required"** action card
2. Click **"Pay Deposit Now"**
3. On the deposit page, select **USDC** payment
4. Click the deposit button → wallet popup appears
5. Approve the deposit transaction

**Expected**:
- Deposit page shows "Deposit: Pending Confirmation"
- After ~10-30s, ticket page auto-refreshes to show "Deposit: Verified ✓"

### Step 1.5: Check In Attendee (Organizer)

1. Return to **Browser A** → navigate to `/staff`
2. The scanner page opens with camera
3. In **Browser B**, expand the QR code on the ticket page
4. Scan the QR code with Browser A's camera (or enter attendee ID manually)

**Expected**:
- Scanner shows "✅ Checked In" with green badge
- Ticket page (Browser B) auto-refreshes within 30s to show "Checked In" hero

---

## Flow 2: Target Event Setup (Tests R-02)

### Step 2.1: Create Target Event (Organizer)

1. In **Browser A** (admin), create a new event:
   - **Event Name**: `Rollover Target Event`
   - **Slug**: `rollover-tgt`
   - **Event Start**: Tomorrow (future date)
   - **Event End**: Tomorrow + 3 hours
   - **Deposit Enabled**: ✅ Check
   - **Deposit Amount USDC**: `1` (same as source — **must match**)
2. Click **Create Event**

### Step 2.2: Initialize Target Escrow (Organizer)

1. Open the target event detail
2. Connect wallet → **Initialize Escrow**
3. Approve TX in wallet

**Expected**:
- Target escrow initialized
- Both events show `escrow_status: initialized`

---

## Flow 3: Rollover in Browser (Tests R-03 through R-08)

### Step 3.1: Verify Rollover Card Appears (Test R-03)

1. Return to **Browser B** → refresh the **source event ticket page**
2. Scroll down to the action cards section

**Expected**: A **"Roll Deposit to Next Event"** card appears with:
- 🔄 Refresh icon
- Title: "Roll Deposit to Next Event"
- Description: "Your deposit is ready to roll over to Rollover Target Event. No extra payment needed..."
- Button: "Roll to Next Event"

**If rollover card does NOT appear**, check:
- Source event has `deposit_info.verified = true`
- Source event attendee is `is_checked_in = true`
- Target event has `rollover_target_event` in API response
- Both events share the same organizer wallet
- Both events have the same USDC deposit amount
- Source event is "past" (event_end has passed)

### Step 3.2: Connect Wallet (Test R-04)

1. Click **"Roll to Next Event"** button
2. Wallet selection panel appears

**Expected**:
- Detected wallets shown as buttons (e.g., "Phantom", "Backpack")
- If no wallets detected: "No wallet detected. Install Phantom/Backpack/Solflare and refresh."
- Cancel button available

3. Click your wallet (e.g., **Phantom**)
4. Approve connection in wallet popup

**Expected**:
- Card updates to show "Connected via Phantom"
- "Sign & Send Rollover" button appears (green)
- Cancel button available

### Step 3.3: Sign and Send Rollover TX (Test R-05)

1. Click **"Sign & Send Rollover"**
2. Card shows "Processing Rollover..." with spinner
3. Wallet popup appears — **approve the transaction**

**Expected**:
- Card transitions through states:
  - `Ready` → `Signing` (spinner + "Please approve the transaction in your wallet...")
  - `Signing` → `Confirmed` (success)
- On success:
  - Title: **"Deposit Rolled Over ✓"** (green)
  - Description: "Your deposit has been moved to Rollover Target Event. TX: {sig_short}"
  - Link: **"View on Solscan →"**

**If TX fails**:
- Card shows "Rollover Failed" with error message
- "Try Again" button available
- Check browser console for `[rollover]` prefixed logs

### Step 3.4: Verify Post-Rollover State (Tests R-06, R-07, R-08)

#### Source Event Verification

1. On the source ticket page, the deposit status should now show:
   - Either "Deposit: Verified ✓" with a refund card, or the rollover confirmation

2. Verify source vault on Solscan:
   - Open the Solscan TX link from the rollover confirmation
   - Verify: source event vault balance = 0 USDC

#### Target Event Verification

3. Navigate to the target event's ticket page (if registered) or check via admin:
   - Target event vault should have 1 USDC from the rollover

---

## Flow 4: Refund from Target Event (Test R-09)

> This step requires the source event's deadline to have passed.

1. Wait for the source event end time to pass (or manually set it to past during setup)
2. On the ticket page (or via admin escrow panel), trigger a refund:
   - The attendee signs a refund TX from the **target** event
   - USDC returns to the attendee's wallet

**Expected**:
- Refund TX confirmed on-chain
- Target event vault = 0 USDC
- Attendee wallet balance restored by 1 USDC
- Ticket page shows "RSVP Deposit Returned ✓" with Solscan link

---

## Flow 5: Full Lifecycle Cleanup (Test R-10)

### Step 5.1: Deactivate Both Events (Organizer)

1. In **Browser A** → admin → select source event
2. Navigate to **Admin Escrow** panel
3. Connect organizer wallet
4. Click **"① Deactivate Event"** → approve TX

5. Repeat for target event

**Expected**:
- Both events show `escrow_status: deactivated`
- Step 1 shows "✓ Done" badge

### Step 5.2: Claim Forfeited (Optional, if no-show deposits exist)

1. If any deposits were not refunded, click **"② Claim Forfeited"**
2. Approve TX in wallet

**Expected**: Organizer receives forfeited USDC

### Step 5.3: Close Both Events

1. Click **"③ Close & Reclaim"** on source event
2. First click shows "⚠ Confirm Close?"
3. Second click executes the close TX → approve in wallet

4. Repeat for target event

**Expected**:
- Both events show `escrow_status: closed`
- Rent reclaimed (small SOL returned to organizer)
- All three steps show "✓ Done" badges

---

## Flow 6: Edge Cases (Tests R-11, R-12)

### Test R-11: Double Rollover Rejected

1. After a successful rollover (Flow 3), refresh the source ticket page
2. Attempt to roll over again (if the card still appears)

**Expected**:
- Either the rollover card no longer appears (deposit already refunded via rollover)
- Or clicking "Roll to Next Event" → signing fails with error: "already refunded" or "deposit not found"

### Test R-12: Rollover Card Hidden for Non-Eligible

Verify that the rollover card does NOT appear when:

| Condition | How to Test |
|-----------|-------------|
| Deposit not verified | Create event, don't pay deposit — check ticket page |
| Attendee not checked in | Pay deposit but don't check in — check ticket page |
| No target event available | Event with no future events from same organizer |
| Different deposit amounts | Source 1 USDC, target 2 USDC — no rollover offered |
| Different organizer | Events from different organizers — no rollover offered |
| Escrow closed | Close source event — no rollover offered |

---

## Console Log Reference

The rollover flow produces these browser console logs (prefixed `[rollover]`):

| Log | Level | When |
|-----|-------|------|
| `wallet connected: {name} ({pk})` | info | Wallet connection succeeds |
| `wallet connect error: {code}` | error | Wallet connection fails |
| `TX build failed: {error}` | error | API `/api/escrow/rollover-deposit` returns error |
| `cluster mismatch: {msg}` | error | Wallet is on wrong cluster (e.g., mainnet instead of devnet) |
| `simulation failed: {error}` | error | Pre-sign TX simulation fails |
| `simulate error (not blocking): {error}` | warn | Simulation error but TX still sent |
| `TX confirmed: {signature}` | info | Rollover TX confirmed on-chain |
| `sign+send error: {code}` | error | Wallet rejects TX or on-chain failure |

---

## API Endpoints Involved

| Endpoint | Method | Role | Purpose |
|----------|--------|------|---------|
| `/api/events` | POST | Organizer | Create source + target events |
| `/api/escrow/init` | POST | Organizer | Initialize escrow on both events |
| `/api/escrow/confirm-init` | POST | Organizer | Confirm escrow initialization |
| `/api/public/ticket/{id}` | GET | Attendee | Fetch ticket data (includes `rollover_target_event`) |
| `/api/deposit/usdc` | POST | Attendee | Initiate USDC deposit |
| `/api/escrow/rollover-deposit` | POST | Attendee | Build rollover TX for wallet signing |
| `/api/escrow/refund` | POST | Attendee | Build refund TX |
| `/api/escrow/deactivate` | POST | Organizer | Deactivate event escrow |
| `/api/escrow/claim-forfeited` | POST | Organizer | Claim forfeited deposits |
| `/api/escrow/close-event` | POST | Organizer | Close event + reclaim rent |

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Rollover card not appearing | Missing `rollover_target_event` in API response | Ensure target event exists, same organizer, same deposit amount, target is active |
| "No wallet detected" | Wallet extension not installed or not injecting | Install Phantom/Solflare, refresh page, wait for extension to inject |
| "Transaction would fail: ..." | On-chain precondition not met | Verify: source deposit verified, attendee checked in, target escrow active |
| "Failed to build transaction" | Worker API error | Check worker console logs, ensure HELIUS_API_KEY is set |
| "Cluster mismatch" | Wallet on wrong network | Switch wallet to Devnet |
| TX SignatureFailure | Wrong wallet signing | Ensure the attendee's deposit wallet is connected (not organizer's) |
| 404 from rollover API | Google Sheets lookup fails in dev mode | Ensure `DEV_MODE=1` is set in worker config |
| Stale blockhash | Cached blockhash expired | Wait 30s and retry |

---

## Acceptance Criteria (Issue #040 Phase C)

- [ ] R-01: Source event created, deposit paid, attendee checked in
- [ ] R-02: Target event created with same deposit amount, escrow initialized
- [ ] R-03: Rollover card appears on source ticket page
- [ ] R-04: Wallet detection and connection works
- [ ] R-05: Rollover TX signed and confirmed on-chain
- [ ] R-06: Source vault empty after rollover
- [ ] R-07: Target vault has deposit after rollover
- [ ] R-08: Source deposit shows as refunded/rolled over
- [ ] R-09: Refund from target event works (optional — requires waiting for deadline)
- [ ] R-10: Both events deactivated and closed successfully
- [ ] R-11: Double rollover rejected
- [ ] R-12: Rollover card correctly hidden for non-eligible attendees
