# BeThere Devnet E2E Walkthrough — Manual Browser Testing Guide

> **Version**: 2025-05-09
> **Deployment (devnet)**: `https://bethere.solana-thailand.workers.dev`
> **Localhost**: `http://localhost:8787` (run `cd worker && ./deploy.sh dev`)
> **Cluster**: Solana Devnet
> **Escrow Program**: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
> **USDC Mint (Devnet)**: `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`

---

## Table of Contents

0. [Localhost API Test Results (automated)](#0-localhost-api-test-results)
1. [Prerequisites & Environment Setup](#1-prerequisites--environment-setup)
2. [Getting Devnet SOL & USDC](#2-getting-devnet-sol--usdc)
3. [Test Accounts & Login](#3-test-accounts--login)
4. [Console Log Patterns](#4-console-log-patterns)
5. [Test Matrix Overview](#5-test-matrix-overview)
6. [Flow 1 — Create Event Without Deposit](#6-flow-1--create-event-without-deposit)
7. [Flow 2 — Create Event With Deposit, No Wallet](#7-flow-2--create-event-with-deposit-no-wallet)
8. [Flow 3 — Create Event With Deposit + Wallet Connected](#8-flow-3--create-event-with-deposit--wallet-connected)
9. [Flow 4 — Edit Event: Add Deposit After Creation](#9-flow-4--edit-event-add-deposit-after-creation)
10. [Flow 5 — Edit Event: Escrow Already Initialized](#10-flow-5--edit-event-escrow-already-initialized)
11. [Flow 6 — Events List: Visual Indicators](#11-flow-6--events-list-visual-indicators)
12. [Flow 7 — Validation: Missing Required Fields](#12-flow-7--validation-missing-required-fields)
13. [Flow 8 — Admin Escrow Actions](#13-flow-8--admin-escrow-actions)
14. [Flow 9 — Attendee Deposit Flow](#14-flow-9--attendee-deposit-flow)
15. [Flow 10 — Scanner: On-Chain Check-In](#15-flow-10--scanner-on-chain-check-in)
16. [Full Lifecycle Walkthrough](#16-full-lifecycle-walkthrough)
17. [Troubleshooting & Common Failures](#17-troubleshooting--common-failures)

---

## 0. Localhost API Test Results

> These tests were run via `curl` against `http://localhost:8787` with `Authorization: Bearer dev-token`.
> All backend validations confirmed working.

### 0.1 How to Run Locally

```bash
# 1. Build frontend
cd frontend-leptos && trunk build && cd ..

# 2. Start worker dev server
cd worker && ./deploy.sh dev

# 3. Verify
http://localhost:8787/api/health  # {"cluster":"devnet","status":"ok"}
```

### 0.2 API Test Results (19 tests)

| # | Test | Method | Endpoint | Expected | Result |
|---|------|--------|----------|----------|--------|
| 1 | Health check | GET | `/api/health` | `{"cluster":"devnet","status":"ok"}` | ✅ Pass |
| 2 | Auth required (no token) | GET | `/api/events` | `missing authentication token` | ✅ Pass |
| 3 | Dev token works | GET | `/api/events` | Event list JSON | ✅ Pass |
| 4 | Events list has deposit/escrow fields | GET | `/api/events` | `deposit_enabled`, `escrow_address` in each event | ✅ Pass |
| 5 | Create event without deposit | POST | `/api/events` | Success, id auto-generated | ✅ Pass |
| 6 | Create event with deposit + 0 USDC | POST | `/api/events` | Success (event created, escrow init blocked later) | ✅ Pass |
| 7 | Escrow init with 0 deposit | POST | `/api/escrow/init` | `deposit amount not configured` | ✅ Pass |
| 8 | Create event with valid deposit | POST | `/api/events` | Success | ✅ Pass |
| 9 | Escrow init with valid deposit | POST | `/api/escrow/init` | Success, TX built, escrow_address returned | ✅ Pass |
| 10 | Update event deposit fields | PUT | `/api/events/{id}` | Success | ✅ Pass |
| 11 | Validation: empty name | POST | `/api/events` | `event name is required` | ✅ Pass |
| 12 | Validation: empty sheet_id | POST | `/api/events` | `google sheet_id is required` | ✅ Pass |
| 13 | Validation: negative start_ms | POST | `/api/events` | `event_start_ms must be positive` | ✅ Pass |
| 14 | Validation: end before start | POST | `/api/events` | `event_end_ms must be after event_start_ms` | ✅ Pass |
| 15 | SEC-003: USDC > $1,000 cap | POST | `/api/events` | `deposit_amount_usdc exceeds maximum cap` | ✅ Pass |
| 16 | SEC-002: Update deposit after escrow set | PUT | `/api/events/{id}` | `cannot change ... after escrow` | ✅ Pass |
| 17 | SEC-004: Archive with active escrow | DELETE | `/api/events/{id}` | `cannot archive ... with active on-chain escrow` | ✅ Pass |
| 18 | Archive without escrow | DELETE | `/api/events/{id}` | Success, status=archived | ✅ Pass |
| 19 | Duplicate slug check | POST | `/api/events` | `event with id already exists` | ✅ Pass |

### 0.3 Automated Test Results

All workspace tests pass (run `cargo test --workspace --quiet` for current count).
```
cargo check --quiet       → clean compile, zero warnings
```

### 0.4 Not Tested via API (requires browser + wallet)

These flows require Phantom wallet interaction and cannot be tested via curl:

- Wallet connection + signing
- SEC-014 cluster detection (needs wallet provider)
- Frontend validation (9 new rules on save)
- Visual indicator badges rendering
- Inline warning display
- Toast messages
- QR code generation / scanning
- Attendee deposit flow (Solana Pay)

These are covered in Flows 1–10 below for manual browser testing.

---

## 1. Prerequisites & Environment Setup

### Browser Requirements

- **Chrome, Brave, or Edge** (Chromium-based for best wallet extension support)
- **Phantom Wallet Extension** installed: https://phantom.app/download
- DevTools open (F12) for console log monitoring

### Phantom Wallet Devnet Setup

1. Open Phantom extension → click Settings (gear icon)
2. Click **"Change Network"** → select **"Devnet"**
3. Verify your wallet shows "Devnet" label at the top
4. Your devnet wallet address is displayed under the account name — copy it for later

### Verify Deployment is Live

```bash
curl -s https://bethere.solana-thailand.workers.dev/api/health
```

Expected response:
```json
{"cluster": "devnet", "status": "ok"}
```

If this fails, the deployment may be down — check the handover notes for the latest deployment status.

---

## 2. Getting Devnet SOL & USDC

### Get Devnet SOL (Airdrop)

You need SOL for transaction fees. Each airdrop gives 2 SOL (max per request).

**Option A: Phantom Built-in Faucet**
1. Open Phantom → Settings → **"Devnet Faucet"**
2. Click **"Request Airdrop"** — 2 SOL deposited

**Option B: CLI**
```bash
solana airdrop 2 <YOUR_WALLET_ADDRESS> --url devnet
```

**Option C: Web Faucet**
- https://faucet.solana.com — paste your wallet address, request airdrop

> **Tip**: Airdrop 4-6 SOL total. Each escrow-related transaction costs ~0.000005 SOL, but you need more for account creation (rent-exempt deposits).

### Get Devnet USDC

Devnet USDC is available via the official Token Faucet:

**Method 1: CLI**
```bash
# Create a USDC ATA for your wallet if you don't have one
spl-token create-account 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU --url devnet

# Mint 100 USDC to your account (100,000,000 lamports = 100 USDC)
spl-token mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU 100000000 <YOUR_ATA_ADDRESS> --url devnet
```

**Method 2: Phantom**
1. Open Phantom → Devnet network
2. Go to the USDC token (if visible) or search for the devnet mint
3. Use the "Receive" or built-in faucet feature

> **Note**: If you can't find devnet USDC via Phantom, use the CLI method. The devnet USDC mint is **not** the same as mainnet USDC.

### Verify Your Balances

```bash
solana balance <YOUR_WALLET_ADDRESS> --url devnet
spl-token accounts --url devnet --owner <YOUR_WALLET_ADDRESS>
```

You should see:
- At least 3 SOL
- At least 10 USDC (10,000,000 lamports with 6 decimals)

---

## 3. Test Accounts & Login

### SuperAdmin Account

The deployment has `DEV_MODE=1` enabled with a developer bypass:

| Field | Value |
|-------|-------|
| **Dev Email** | `ratchapon.poc@gmail.com` |
| **Login Method** | Google OAuth (Sign In button) |
| **Role** | SuperAdmin (full access) |

### Login Steps

1. Open https://bethere.solana-thailand.workers.dev
2. Click **"Sign In"** button (top-right)
3. Google OAuth flow — sign in with the admin Google account
4. After redirect, you should land on the admin dashboard at `/admin`
5. You should see the admin sidebar with: Events, Deposits, Escrow, Adventure tabs

### Staff Account (for Scanner testing)

Staff access is controlled by the `STAFF_EMAILS` Cloudflare secret. Add your test email via:
```bash
cd worker && npx wrangler secret put STAFF_EMAILS
# Enter comma-separated emails
```

Or use the existing staff emails configured in the secret.

---

## 4. Console Log Patterns

Open browser DevTools (F12) → Console tab. Watch for these log prefixes:

### Event Management Logs
| Pattern | Source | Meaning |
|---------|--------|---------|
| `[escrow-init] connected ... pk=...` | `escrow_init.rs` | Wallet connected successfully |
| `[escrow-init] cluster check passed` | `escrow_init.rs` | Wallet on correct cluster (devnet) |
| `[escrow-init] cluster mismatch` | `escrow_init.rs` | Wallet on wrong network |
| `[escrow-init] escrow TX built, signing...` | `escrow_init.rs` | Transaction built, waiting for wallet signature |
| `[escrow-init] escrow TX confirmed: ...` | `escrow_init.rs` | On-chain escrow creation success |
| `[escrow-init] escrow TX rejected` | `escrow_init.rs` | User rejected wallet popup |
| `[escrow-init] init_escrow failed: ...` | `escrow_init.rs` | Backend error building TX |
| `[escrow-init] escrow already exists, reloading...` | `escrow_init.rs` | Duplicate escrow attempt (auto-recovery) |
| `[events-page] detected wallets: [...]` | `events_page.rs` | Wallet extensions found |

### Deposit Flow Logs
| Pattern | Source | Meaning |
|---------|--------|---------|
| `[deposit] wallet connected: ...` | `deposit.rs` | Attendee wallet connected |
| `[deposit] TX sent, signature: ...` | `deposit.rs` | Deposit TX submitted |
| `[deposit] confirmed on-chain: ...` | `deposit.rs` | Deposit confirmed via polling |
| `[deposit] USDC QR deposit initiated` | `deposit.rs` | Solana Pay QR generated |
| `[deposit] THB slip uploaded successfully` | `deposit.rs` | THB payment slip accepted |
| `[deposit] refund TX sent, signature: ...` | `deposit.rs` | Refund TX submitted |

### Scanner Logs
| Pattern | Source | Meaning |
|---------|--------|---------|
| `[scanner] QR code detected: ...` | `scanner.rs` | QR scanned from camera |
| `[scanner] check-in successful: ...` | `scanner.rs` | Off-chain check-in success |
| `[scanner] event '...' escrow_enabled=...` | `scanner.rs` | Escrow status loaded for event |
| `[scanner] organizer wallet connected: ...` | `scanner.rs` | Organizer wallet for on-chain check-in |
| `[scanner] on-chain check-in TX sent: ...` | `scanner.rs` | `mark_checked_in` TX submitted |
| `[scanner] mark_checked_in API failed: ...` | `scanner.rs` | TX build error |

### Admin Escrow Logs
| Pattern | Source | Meaning |
|---------|--------|---------|
| `[admin-escrow] detected wallets: [...]` | `admin_escrow.rs` | Wallet scan on page load |
| `[admin-escrow] wallet connected: ...` | `admin_escrow.rs` | Organizer wallet connected |
| `[admin-escrow] Deactivate Event TX built, signing...` | `admin_escrow.rs` | Lifecycle step 1 |
| `[admin-escrow] Claim Forfeited TX built, signing...` | `admin_escrow.rs` | Lifecycle step 2 |
| `[admin-escrow] Close Event TX built, signing...` | `admin_escrow.rs` | Lifecycle step 3 |
| `[admin-escrow] ... TX confirmed: ...` | `admin_escrow.rs` | Step completed |

---

## 5. Test Matrix Overview

| # | Flow | Prerequisites | Complexity |
|---|------|---------------|------------|
| 1 | Create Event — No Deposit | Logged in | Easy |
| 2 | Create Event — With Deposit, No Wallet | Logged in | Easy |
| 3 | Create Event — With Deposit + Wallet | Logged in + Phantom connected | Medium |
| 4 | Edit Event — Add Deposit After Creation | Event from Flow 1 | Easy |
| 5 | Edit Event — Escrow Already Initialized | Event from Flow 3 | Easy |
| 6 | Events List — Visual Indicators | Events with various states | Easy |
| 7 | Validation — Missing Required Fields | Logged in, on Create form | Medium |
| 8 | Admin Escrow Actions | Event with active escrow | Hard |
| 9 | Attendee Deposit Flow | Event with escrow + attendee link | Hard |
| 10 | Scanner — On-Chain Check-In | Event with escrow + deposited attendee | Hard |

---

## 6. Flow 1 — Create Event Without Deposit

### Prerequisites
- Logged in as SuperAdmin
- No wallet connection needed

### Steps

1. Navigate to **Admin → Events** tab
2. Click **"+ Create Event"** button
3. The Create Event form loads with these defaults:
   - Deposit toggle = OFF
   - Basic Info section: EXPANDED
   - Schedule section: EXPANDED
   - All other sections: COLLAPSED
4. Fill in **Basic Info** (REQUIRED — red badge):
   - **Event Name**: `E2E Test No Deposit`
   - **Slug**: auto-generated as `e2e-test-no-deposit` (editable)
   - **Description** (optional): any text
5. Expand **Schedule** section (REQUIRED — red badge):
   - **Event Start**: pick a future date/time
   - **Event End**: pick a later date/time
6. Expand **Google Sheets** section (REQUIRED — red badge):
   - **Sheet ID**: paste any valid Google Sheet ID (e.g., `1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms`)
7. Click **"Create Event"** button

### Expected Result

- Toast message (green): **"Event 'E2E Test No Deposit' created"**
- Page redirects back to the Events list
- New event card appears in the list with:
  - Name: "E2E Test No Deposit"
  - Status: "Draft" badge
  - **No escrow badge** (no yellow/green badge next to it)

### What to Verify
- [ ] Event card visible in list
- [ ] No "No Escrow" or "Escrow" badge on the card
- [ ] Can click "Edit" to reopen the form
- [ ] All fields saved correctly

### Common Failures
| Symptom | Cause | Fix |
|---------|-------|-----|
| "Event name is required" toast | Name field left empty | Fill the name |
| "Google Sheet ID is required" toast | Sheet ID empty | Expand Sheets section, fill Sheet ID |
| "Event start date is required" toast | Schedule not set | Expand Schedule, set both dates |
| 401 Unauthorized | Session expired | Re-login via Sign In |

---

## 7. Flow 2 — Create Event With Deposit, No Wallet

### Prerequisites
- Logged in as SuperAdmin
- **Do NOT connect** any wallet extension (or disconnect if auto-connected)

### Steps

1. Navigate to **Admin → Events** tab
2. Click **"+ Create Event"** button
3. Fill in **Basic Info**:
   - **Event Name**: `E2E Test Deposit No Wallet`
   - **Slug**: auto-generated
4. Fill in **Schedule**:
   - Set future start/end dates
5. Fill in **Google Sheets**:
   - **Sheet ID**: any valid sheet ID
6. Toggle the **"Deposit"** switch to ON (between Settings and People sections)
7. The **Deposit Details** section appears
8. Fill in deposit fields:
   - **USDC Amount**: `0.5` (half a USDC)
   - **THB Amount**: leave empty (or set to `100`)
   - **PromptPay ID**: leave empty (unless THB set)
   - **Refund Deadline (hours)**: `48`
9. Scroll down to **Escrow Setup** panel — it should show:
   - "No Solana wallets detected" message (if no extension installed)
   - OR wallet connect buttons if Phantom is installed but not connected
10. Click **"Create Event"** button (should NOT say "Initialize Escrow")

### Expected Result

- Toast message (green): **"Event 'E2E Test Deposit No Wallet' created"**
- Return to Events list
- Event card shows: **yellow "No Escrow" badge**

### What to Verify
- [ ] Event card has yellow "No Escrow" badge
- [ ] Button said "Create Event" (not "Create Event + Initialize Escrow")
- [ ] No wallet popup appeared during creation
- [ ] Editing the event shows deposit fields populated

### Common Failures
| Symptom | Cause | Fix |
|---------|-------|-----|
| "At least one deposit amount is required" toast | Both USDC and THB = 0 | Set at least one amount |
| "Minimum deposit is 0.01 USDC" toast | USDC value between 0 and 0.01 | Set USDC to at least 0.01 |
| "Maximum deposit is 1,000 USDC" toast | USDC > 1000 | Lower the amount |
| "Refund deadline must be at least 1 hour" toast | Deadline = 0 | Set at least 1 |

---

## 8. Flow 3 — Create Event With Deposit + Wallet Connected

### Prerequisites
- Logged in as SuperAdmin
- **Phantom wallet** installed, set to **Devnet** network
- At least 3 SOL and 1 USDC in the Phantom devnet wallet

### Steps

1. Navigate to **Admin → Events** tab
2. Click **"+ Create Event"** button
3. Fill in **Basic Info**:
   - **Event Name**: `E2E Test Full Escrow`
   - **Slug**: auto-generated
4. Fill in **Schedule**:
   - Set start date: 1 hour from now
   - Set end date: 2 hours from now
5. Fill in **Google Sheets**:
   - **Sheet ID**: any valid sheet ID
6. Toggle **"Deposit"** switch to ON
7. Fill in deposit fields:
   - **USDC Amount**: `0.5`
   - **THB Amount**: leave empty
   - **Refund Deadline (hours)**: `48`
8. Scroll to **Escrow Setup** panel
9. Click **"Connect Phantom"** button
   - Phantom popup appears → click **"Connect"**
   - Panel shows: "Phantom connected: ABC...XYZ" with wallet address
10. The save button should now read: **"Create Event + Initialize Escrow"**
11. Click **"Create Event + Initialize Escrow"**

**What happens internally (4 steps):**
1. `POST /api/events` → backend creates event → returns `event_id`
2. `POST /api/escrow/init` → backend builds unsigned escrow TX → returns transaction
3. Phantom popup appears → approve the transaction
4. `PUT /api/events/{id}` → saves `escrow_address`, `organizer_wallet`, `on_chain_event_id`

### Expected Result (Success Path)

- Toast message (green): **"Event 'E2E Test Full Escrow' created + escrow initialized"**
- Return to Events list
- Event card shows: **green "Escrow" badge**
- Console shows: `[escrow-init] escrow TX confirmed: <signature>`

### What to Verify
- [ ] Green "Escrow" badge on event card
- [ ] Button was labeled "Create Event + Initialize Escrow"
- [ ] Phantom popup appeared with a transaction to sign
- [ ] Click "Edit" on the event → escrow fields show as locked:
  - Escrow Address: `[base58...] 🔒 Locked`
  - Organizer Wallet: `[base58...] 🔒 Locked`
  - On-Chain Event ID: `[number] 🔒 Locked`
- [ ] Green banner: "Escrow initialized: [address]"
- [ ] Solscan link appears next to "Escrow Address" label (opens on devnet)

### Sub-Scenario: User Rejects Wallet Signature

1. Repeat steps 1-11 above, but **reject** the Phantom popup
2. Expected toast (yellow): **"Event '...' created, but escrow TX was rejected. Edit event to retry."**
3. Event card shows: **yellow "No Escrow" badge**
4. Console shows: `[escrow-init] escrow TX rejected`

### Sub-Scenario: Backend Error During Init

1. If the init_escrow API fails (e.g., wrong cluster, malformed request)
2. Expected toast (yellow): **"Event '...' created, but escrow init failed: {error}. Edit event to retry."**
3. Event exists without escrow — can retry via Edit

### Common Failures
| Symptom | Cause | Fix |
|---------|-------|-----|
| Phantom popup never appears | Wallet extension not installed or on wrong network | Install Phantom, switch to Devnet |
| "Wallet is on mainnet-beta but app expects devnet" | Phantom on mainnet | Switch Phantom to Devnet network |
| TX fails in Phantom | Insufficient SOL for rent | Airdrop more SOL |
| "USDC deposit amount is required to initialize escrow" | USDC amount = 0 | Set USDC amount > 0 |
| "Failed to build escrow TX: ..." | Backend can't reach Solana RPC | Check health endpoint, retry |

---

## 9. Flow 4 — Edit Event: Add Deposit After Creation

### Prerequisites
- Event created without deposit (from Flow 1)
- No wallet connection needed for this step

### Steps

1. On the Events list, find the event created in Flow 1 (`E2E Test No Deposit`)
2. Click **"Edit"** button on the event card
3. The Edit form loads with all existing data
4. Toggle the **"Deposit"** switch to ON
5. The **Deposit Details** section appears
6. Fill in deposit fields:
   - **USDC Amount**: `1.0`
   - **Refund Deadline (hours)**: `24`
7. Click **"Update Event"** button

### Expected Result

- Toast message (green): **"Event 'E2E Test No Deposit' updated"**
- Return to Events list
- Event card now shows: **yellow "No Escrow" badge** (deposit enabled but no on-chain escrow yet)

### What to Verify
- [ ] Yellow "No Escrow" badge appeared (wasn't there before)
- [ ] Editing again shows deposit fields populated with values you entered
- [ ] Escrow Setup panel shows "Connect wallet to initialize escrow"

### Follow-Up: Initialize Escrow on Edit

1. Edit the same event again
2. Scroll to **Escrow Setup** panel
3. Click **"Connect Phantom"** → approve in wallet popup
4. Click **"Create & Sign"** button
5. Phantom popup appears → approve the transaction
6. Expected: Green success panel with escrow address + Solscan link
7. Click **"Update Event"** to save the final state
8. Events list now shows: **green "Escrow" badge**

---

## 10. Flow 5 — Edit Event: Escrow Already Initialized

### Prerequisites
- Event with escrow already initialized (from Flow 3 or Flow 4 follow-up)

### Steps

1. On the Events list, find the event with green "Escrow" badge
2. Click **"Edit"** button
3. The Edit form loads

### What to Verify

- [ ] **Escrow Address** field shows base58 address with **🔒 Locked** badge
- [ ] **Organizer Wallet** field shows public key with **🔒 Locked** badge
- [ ] **On-Chain Event ID** field shows number with **🔒 Locked** badge
- [ ] Green banner visible: "Escrow initialized: [address]"
- [ ] **Solscan link** appears as a pill next to the "Escrow Address" label
- [ ] Deposit amounts (USDC/THB) are still **editable**
- [ ] PromptPay ID is still **editable**
- [ ] Refund deadline is still **editable**
- [ ] Clicking "Update Event" works (saves editable fields)

### What Should NOT Happen
- [ ] Escrow address field should NOT be an editable text input
- [ ] Organizer wallet field should NOT be modifiable
- [ ] On-chain event ID should NOT be modifiable

### Locked Fields Explanation

These fields are locked because they were set by the on-chain escrow initialization and are **immutable** on-chain (SEC-002 security fix). The backend rejects any attempt to change them after `escrow_address` is set.

---

## 11. Flow 6 — Events List: Visual Indicators

### Prerequisites
- At least 3 events in different states (create them from previous flows):
  1. Event without deposit (Flow 1)
  2. Event with deposit but no escrow (Flow 2)
  3. Event with escrow initialized (Flow 3)

### Steps

1. Navigate to **Admin → Events** tab
2. Observe the event cards in the list

### What to Verify Per Event

| Event State | Badge Expected | Color |
|-------------|---------------|-------|
| `deposit_enabled=false` | **No badge** | — |
| `deposit_enabled=true`, `escrow_address=""` | **"No Escrow"** | Yellow |
| `deposit_enabled=true`, `escrow_address≠""` | **"Escrow"** | Green |

### Search Filter

3. Type in the **search input** (next to "Create Event" button)
4. Type the name of one of your test events
5. Verify: list filters to show only matching events
6. Clear the search — all events reappear

### What to Verify
- [ ] Three different badge states visible
- [ ] Search filters events by name, slug, or Sheet ID (case-insensitive)
- [ ] Clearing search restores full list
- [ ] Event status badges (Draft/Active/Completed/Archived) also visible

---

## 12. Flow 7 — Validation: Missing Required Fields

### Prerequisites
- Logged in as SuperAdmin
- On the Create Event form

> **Note on Collapsible Sections**: The form uses collapsible sections. **Basic Info** and **Schedule** are expanded by default (required). All other sections (Google Sheets, Deposit Settings, etc.) start collapsed. Expand the relevant section before testing each case below.

### Test Cases

Perform each test individually. After each failed validation, verify the toast message, then fix the field for the next test.

| # | Test Action | Expected Toast Message | Type |
|---|-------------|----------------------|------|
| 1 | Clear Event Name → click Create | "Event name is required" | Error |
| 2 | Clear Slug → click Create | "Event slug is required" | Error |
| 3 | Clear Sheet ID → click Create | "Google Sheet ID is required" | Error |
| 4 | Clear Event Start date → click Create | "Event start date is required" | Error |
| 5 | Clear Event End date → click Create | "Event end date is required" | Error |
| 6 | Set Event End before Event Start → click Create | "Event end must be after event start" | Error |
| 7 | Enable deposit, set USDC = 0.001 → click Create | "Minimum deposit is 0.01 USDC" | Error |
| 8 | Enable deposit, set USDC = 1500 → click Create | "Maximum deposit is 1,000 USDC" | Error |
| 9 | Enable deposit, set both USDC and THB = 0 → click Create | "At least one deposit amount (USDC or THB) is required when deposit is enabled" | Error |
| 10 | Enable deposit, set THB > 0 but no PromptPay → click Create | "PromptPay ID is required when THB amount is set" | Error |
| 11 | Enable deposit, set refund deadline = 0 → click Create | "Refund deadline must be at least 1 hour" | Error |

### Inline Warnings (Yellow Hints)

These appear as yellow text hints below the field before clicking save:

| # | Test Action | Expected Yellow Hint |
|---|-------------|---------------------|
| 1 | Set event end before event start (both set) | "Event end must be after event start" |
| 2 | Enable deposit, both amounts = 0 | "At least one deposit amount (USDC or THB) is required" |
| 3 | USDC value between 0 and 0.01 | "Minimum deposit is 0.01 USDC" |
| 4 | USDC value > 1000 | "Maximum deposit is 1,000 USDC (SEC-003 cap)" |
| 5 | THB > 0 but no PromptPay | "PromptPay ID is required when THB amount is set" |
| 6 | Refund deadline = 0 with deposit enabled | "Refund deadline must be at least 1 hour" |

### What to Verify
- [ ] All 11 validation toasts appear correctly
- [ ] Yellow inline warnings appear BEFORE clicking save
- [ ] Form does NOT submit on validation failure
- [ ] No console errors (only validation messages)

---

## 13. Flow 8 — Admin Escrow Actions

### Prerequisites
- Event with **active escrow** (from Flow 3 or Flow 4 follow-up)
- The event must have:
  - `event_start` in the past (for deactivate)
  - `deposit_enabled=true` and `escrow_address` set
- **Phantom wallet** connected to Devnet with the **same wallet** that initialized the escrow (organizer wallet)
- Sufficient SOL for transaction fees

### Important: Ordering Constraints

Escrow lifecycle actions **must** be performed in this exact order:
1. **Deactivate Event** → stops new deposits
2. **Claim Forfeited** → transfers unclaimed deposits to organizer
3. **Close Event** → reclaims rent, closes PDA

> **Warning**: Attempting steps out of order will fail with on-chain errors. Each step depends on the previous state.

### Steps

1. Navigate to **Admin → Escrow** tab (sidebar)
2. The Escrow Management page loads
3. **Select the event** from the dropdown (if multiple events)
4. Click **"Connect Organizer Wallet"**
   - Phantom popup → approve connection
   - Shows connected wallet address
5. **Step 1: Deactivate Event**
   - Click **"Sign TX"** button next to "Deactivate Event"
   - Phantom popup appears → **approve**
   - Expected: ✅ checkmark + Solscan link
   - Console: `[admin-escrow] Deactivate Event TX confirmed: <sig>`
6. **Step 2: Claim Forfeited**
   - Click **"Sign TX"** button next to "Claim Forfeited"
   - Phantom popup → **approve**
   - Expected: ✅ checkmark + Solscan link
   - Console: `[admin-escrow] Claim Forfeited TX confirmed: <sig>`
7. **Step 3: Close Event**
   - Click **"Sign TX"** button next to "Close Event"
   - Phantom popup → **approve**
   - Expected: ✅ checkmark + Solscan link
   - Console: `[admin-escrow] Close Event TX confirmed: <sig>`

### What to Verify
- [ ] All 3 steps show ✅ checkmarks after completion
- [ ] Each step has a Solscan link that opens on devnet cluster
- [ ] Results persist (all 3 banners visible simultaneously)
- [ ] Console logs show confirmation signatures
- [ ] Clicking Solscan links opens the correct transaction on explorer.solana.com?cluster=devnet

### Common Failures
| Symptom | Cause | Fix |
|---------|-------|-----|
| "Transaction rejected or failed" | Wrong wallet (not the organizer) | Connect the wallet that initialized the escrow |
| "Program error: X" | Steps done out of order | Follow deactivate → claim → close order |
| "Wallet is on mainnet-beta" | Phantom on wrong network | Switch to Devnet |
| Claim forfeited returns nothing | No deposits to claim | Have at least 1 no-show deposit first |
| Cannot deactivate | Event is not active on-chain | Verify on-chain state via Solscan |

### Edge Case: Wrong Wallet

1. Connect a different wallet than the one that created the escrow
2. Try to sign any escrow action
3. Expected: TX fails on-chain with "constraint has_one" or similar error
4. This is correct behavior — only the organizer wallet can sign

---

## 14. Flow 9 — Attendee Deposit Flow

### Prerequisites
- Event with **active escrow** (initialized, not yet started or currently active)
- An attendee link — format: `/deposit/{attendee_id}?event_id={event_id}`
- **Phantom wallet** with devnet USDC for the attendee (can be the same wallet if testing solo)
- The event's `deposit_amount_usdc` must be set (e.g., 0.5 USDC)

### Getting an Attendee Link

Attendee deposit links follow the pattern:
```
https://bethere.solana-thailand.workers.dev/deposit/{attendee_id}?event_id={event_id}
```

You need a valid `attendee_id` from your Google Sheet. The attendee must be registered in the sheet for the event.

**For testing**, you can create a test attendee by:
1. Adding a row to your Google Sheet with the event's Sheet ID
2. Or using the admin to generate QR codes (which creates attendee records)

### Steps: USDC Deposit

1. Open the deposit link in your browser
2. Page loads with **deposit status** — shows event name, deposit amount, payment options
3. Click **"Pay with USDC (Solana)"** or **"Connect Wallet"** button
4. Phantom popup → approve connection
5. Page shows connected wallet address
6. Click **"Deposit 0.5 USDC"** button
7. Phantom popup with transaction → **approve**
8. Page shows "Confirming deposit..." with spinner
9. After ~3-10 seconds: **"Deposit Confirmed"** with transaction signature
10. Console: `[deposit] confirmed on-chain: <signature>`

### Steps: Solana Pay QR (Alternative)

1. On the deposit page, instead of wallet connect
2. Click **"Show QR Code"** or **"Solana Pay"**
3. A Solana Pay QR code appears
4. Scan with a mobile wallet (Phantom mobile)
5. Transaction appears in mobile wallet → approve
6. Desktop page polls for confirmation and updates

### Steps: THB Payment (PromptPay)

1. On the deposit page, click **"Pay with THB"** tab/section
2. If the event has a `promptpay_id` configured, a PromptPay QR code is displayed with the exact THB amount
3. Scan the QR code with any Thai banking app (K Plus, SCB EASY, TrueMoney, etc.)
4. Confirm the payment in your bank app
5. Back on the deposit page, upload a screenshot of the payment slip
6. Click **"Upload Slip"**
7. Page shows: **"Slip uploaded, waiting for verification"**
8. Admin must verify in **Admin → Deposits** tab

> **Note:** The PromptPay QR uses EMVCo standard encoding with the correct AID (`A000000677010112`). Tags are in strict ascending order (53 → 54 → 58) per Thai QR Payment standard. It should work with all Thai bank apps.

### What to Verify
- [ ] Deposit page loads with correct event info
- [ ] USDC amount matches the event's `deposit_amount_usdc` (fixed, cannot be changed)
- [ ] After successful deposit, page shows confirmed state with TX signature
- [ ] Depositing again shows "Already Deposited" with deposit details
- [ ] Console logs show deposit lifecycle

### Steps: Refund (After Event End)

1. After the event's `event_end` timestamp passes
2. Revisit the same deposit link
3. Page shows **"Refund Available"** button
4. Click **"Connect Wallet for Refund"**
5. Connect the same wallet used for deposit
6. Click **"Claim Refund"**
7. Phantom popup → approve refund TX
8. Page shows: **"Refund Confirmed"** with TX signature
9. Console: `[deposit] refund TX sent, signature: <sig>`

### Steps: Reclaim Rent (After Refund)

1. After refund is confirmed
2. Page shows **"Reclaim Rent"** button
3. Click it → Phantom popup → approve
4. AttendeeDeposit PDA closed, rent reclaimed
5. Console: deposit close confirmed

### Common Failures
| Symptom | Cause | Fix |
|---------|-------|-----|
| "Deposit not required" | Event has deposit disabled or no escrow | Ensure escrow is initialized |
| "Deposit not enabled" | Backend checks `escrow_address` is empty | Initialize escrow first |
| TX fails: "insufficient funds" | Not enough USDC in wallet | Get devnet USDC (Section 2) |
| TX fails: "already in use" | Already deposited from this wallet | Use different wallet or check "Already Deposited" state |
| Confirmation polling times out | Slow devnet RPC | Wait and refresh, or check Solscan manually |
| Refund button not visible | Event hasn't ended yet | Wait for `event_end` to pass |

---

## 15. Flow 10 — Scanner: On-Chain Check-In

### Prerequisites
- Event with **active escrow** initialized
- At least **one attendee has deposited** USDC (from Flow 9)
- Logged in as **Staff** or **Admin**
- **Phantom wallet** connected to Devnet with the **organizer's wallet**
- Camera access (for QR scanning) or manual entry

### Steps

1. Navigate to the **Scanner** page (via Staff link or admin)
2. Camera viewfinder loads with bottom sheet
3. **Scan a QR code** for the attendee who deposited
   - The QR code should contain the attendee's claim link or ID
4. Off-chain check-in happens first:
   - Console: `[scanner] check-in successful: <name>`
   - Green success overlay appears with attendee name
5. **Escrow check-in prompt** appears below the success message:
   - "Mark Checked In On-Chain" button appears (only if escrow enabled)
   - Console: `[scanner] event '...' escrow_enabled=true`
6. Click **"Mark Checked In On-Chain"**
7. Wallet selection panel appears (if multiple wallets)
8. Click **"Connect Phantom"** → approve in popup
9. Console: `[scanner] organizer wallet connected: Phantom (ABC...)`
10. Click **"Sign TX"**
11. Phantom popup with `mark_checked_in` transaction → **approve**
12. Console: `[scanner] on-chain check-in TX sent: <signature>`
13. Green confirmation: **"On-chain check-in confirmed!"**

### Alternative: Skip On-Chain Check-In

- Click **"Skip & Scan Next"** to skip the on-chain step
- Off-chain check-in is still recorded
- On-chain check-in can be done later

### What to Verify
- [ ] Off-chain check-in works regardless of escrow
- [ ] On-chain check-in only appears if escrow is enabled
- [ ] Wrong wallet (not organizer) → TX fails with constraint error
- [ ] Already checked-in attendee → shows "Already checked in" warning
- [ ] Console logs show both off-chain and on-chain steps

### Common Failures
| Symptom | Cause | Fix |
|---------|-------|-----|
| No escrow prompt after check-in | Event has no escrow or not detected | Verify event has escrow_address set |
| "No event selected for on-chain check-in" | Events list empty or event not found | Create/load an event first |
| TX fails: "has_one constraint" | Connected wallet is not the organizer | Connect the organizer's wallet |
| Camera not working | Browser permission denied | Allow camera access in browser settings |
| QR not detected | Blurry/angled QR code | Hold QR flat, good lighting |

---

## 16. Full Lifecycle Walkthrough

This is the recommended order for a complete end-to-end test:

### Phase A: Event Setup

```
1. Login as SuperAdmin
2. Create event WITH deposit + wallet connected (Flow 3)
   → Verify green "Escrow" badge
3. Verify escrow on Solscan:
   → Click Solscan link → see EventEscrow PDA with deposit_amount, is_active=true
```

### Phase B: Attendee Deposits

```
4. Open attendee deposit link (Flow 9) — USDC deposit
   → Verify deposit on Solscan: AttendeeDeposit PDA created
5. (Optional) Open second attendee deposit link — different wallet
   → Multiple deposits visible
```

### Phase C: Event Day — Check-In

```
6. Open Scanner (Flow 10)
7. Scan QR for attendee → off-chain check-in success
8. Click "Mark Checked In On-Chain" → sign TX
   → Verify on Solscan: AttendeeDeposit.checked_in = true
9. Scan second attendee → repeat
```

### Phase D: Post-Event — Refund

```
10. Wait for event_end to pass (or adjust event times)
11. Attendee opens deposit link → sees "Refund Available"
12. Click "Claim Refund" → sign TX
    → Verify on Solscan: vault → attendee USDC transfer
    → AttendeeDeposit.refunded = true
13. Click "Reclaim Rent" → close deposit PDA
```

### Phase E: No-Show Forfeiture

```
14. If attendee did NOT check in and did NOT refund:
    → Their deposit is "forfeited" after refund_deadline passes
15. Admin → Escrow Management
16. Step 1: Deactivate Event → sign TX
17. Step 2: Claim Forfeited → sign TX
    → Organizer receives forfeited USDC
18. Step 3: Close Event → sign TX
    → Rent reclaimed, all PDAs closed
```

### Phase F: Verification

```
19. Check Solscan for the escrow PDA → should show closed
20. Check organizer USDC balance → includes forfeited deposits
21. Check attendee USDC balance → includes refunded deposits
```

---

## 17. Troubleshooting & Common Failures

### General Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| Page blank/white | Frontend WASM failed to load | Hard refresh (Ctrl+Shift+R), check Network tab |
| 401 on all API calls | Session expired | Re-login via Sign In |
| "Internal Server Error" | Worker crash | Check Cloudflare dashboard for logs |
| API returns unexpected data | Stale KV cache | Wait 60s for KV propagation |
| `wrangler dev` fails: "Address already in use (127.0.0.1:8787)" | Stale `wrangler dev` process still bound to port 8787 | Kill stale process: `kill -9 $(lsof -ti:8787)` — or use alternate port: `npx wrangler dev --port 8788` |

### Wallet Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| "No Solana wallets detected" | Extension not installed or disabled | Install Phantom extension, refresh page |
| Wallet popup doesn't appear | Extension blocked by browser | Disable popup blocker, check extension permissions |
| TX signature fails | Insufficient SOL | Airdrop more SOL to wallet |
| "Wallet is on mainnet-beta" | Wrong network in Phantom | Settings → Change Network → Devnet |
| Cannot detect cluster | Phantom API limitation | Cluster check skipped gracefully |

### On-Chain Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| "already in use" | PDA already initialized | Expected for duplicate operations |
| "has_one constraint" | Wrong signer (not organizer) | Use the organizer's wallet |
| "is_active=false" | Event deactivated | Expected after deactivate step |
| "refund_deadline not passed" | Too early for claim_forfeited | Wait for deadline to pass |
| Program not found | Program not deployed | Verify: `solana program show C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T --url devnet` |

### Useful Devnet Links

| Resource | URL |
|----------|-----|
| **Solscan (Devnet)** | `https://solscan.io/account/{ADDRESS}?cluster=devnet` |
| **Solana Explorer (Devnet)** | `https://explorer.solana.com/address/{ADDRESS}?cluster=devnet` |
| **Phantom Devnet Faucet** | Phantom Settings → Devnet Faucet |
| **Solana Faucet** | `https://faucet.solana.com` |
| **Health Check** | `https://bethere.solana-thailand.workers.dev/api/health` |
| **Escrow Program** | `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |

### Quick Debug Commands

```bash
# Check devnet deployment health
curl -s https://bethere.solana-thailand.workers.dev/api/health | jq .

# Verify escrow program on devnet
solana program show C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T --url devnet

# Check your devnet balance
solana balance --url devnet

# Check USDC token accounts
spl-token accounts --url devnet

# Look up a transaction
solana confirm <TX_SIGNATURE> --url devnet
```

---

## Appendix A: Key Architecture Reference

```
┌─────────────────┐     ┌──────────────────────┐     ┌─────────────────┐
│   Phantom Wallet │────▶│   Cloudflare Worker   │────▶│  Solana Devnet  │
│   (Browser Ext)  │     │  bethere.workers.dev  │     │  (RPC + Program)│
└─────────────────┘     └──────────────────────┘     └─────────────────┘
                              │         │         │
                              ▼         ▼         ▼
                        ┌─────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
                        │  KV Store │ │ D1        │ │ R2        │ │ Google   │
                        │  (Events) │ │ (Metadata)│ │ (Assets)  │ │ Sheets   │
                        └─────────┘ └──────────┘ └──────────┘ └──────────┘
```

### On-Chain Program Instructions (Order Matters)

```
create_event ──▶ deposit (×N) ──▶ deactivate_event
                                      │
                               mark_checked_in (×N)
                                      │
                               refund (×N, after event_end)
                                      │
                               claim_forfeited (after refund_deadline)
                                      │
                               close_event (after settlement)

rollover_deposit (source event deactivated, target event active)
```

### PDA Seeds

| Account | Seeds |
|---------|-------|
| EventEscrow | `["escrow", organizer, event_id]` |
| AttendeeDeposit | `["deposit", event, attendee]` |
| Vault (ATA) | Derived via Associated Token Program |

---

## Appendix B: Devnet USDC Mint Addresses

> **Important**: The devnet USDC mint changed between versions. Use the correct one:

| Label | Mint Address | Notes |
|-------|-------------|-------|
| **Devnet USDC** | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` | Current devnet test USDC |
| ~~Old Devnet~~ | ~~`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`~~ | **This is mainnet USDC!** Do NOT use on devnet |

---

*Document generated from codebase analysis. Last updated: 2025-05-09. For the latest deployment info, check `.handovers/` for the most recent handover document.*
