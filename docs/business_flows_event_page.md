# Business Flow Scenarios — Create/Edit Event Page

> All scenarios for what happens when a user (organizer/admin) interacts with the Create/Edit Event page,
> including deposit configuration, escrow initialization, and error paths.

---

## 1. Roles & Permissions

| Role | Can Create | Can Edit | Can Archive | Can Init Escrow |
|------|-----------|---------|-------------|-----------------|
| SuperAdmin | ✅ | ✅ All events | ✅ | ✅ |
| Organizer | ✅ | ✅ Assigned events | ✅ Assigned | ✅ |
| Staff | ❌ | ❌ | ❌ | ❌ |

- **Staff users** see the events list but the "+ Create Event" button is hidden.
- **Organizer** emails must be listed in the event's `organizer_emails` field.

---

## 2. Events List Page — Visual Indicators

Each event card shows:

| Indicator | Condition | Meaning |
|-----------|-----------|---------|
| `⚠ No Escrow` (yellow badge) | `deposit_enabled=true` AND `escrow_address=""` | Deposit is enabled but on-chain escrow PDA not yet created. Organizer must Edit → Init Escrow. |
| `🏦 Escrow` (green badge) | `deposit_enabled=true` AND `escrow_address≠""` | On-chain escrow is active and ready to accept deposits. |
| No badge | `deposit_enabled=false` | No deposit feature — standard event. |

---

## 3. Create Event Scenarios

### 3A. Create Event WITHOUT Deposit

```
User clicks "+ Create Event"
  → Form loads with default values
  → Deposit toggle = OFF

User fills:
  - Name* (slug auto-generated)
  - Sheet ID* (required)
  - Schedule, NFT, People (optional sections, collapsed by default)

User clicks "Create Event"
  → Validate: name, slug, sheet_id not empty
  → POST /api/events → backend creates event
  → Toast: "Event 'X' created"
  → Return to events list
```

**Validations:**
- Name must not be empty
- Slug must not be empty
- Sheet ID must not be empty
- No deposit validations run (deposit_enabled=false)

### 3B. Create Event WITH Deposit — Wallet NOT Connected

```
User clicks "+ Create Event"
User enables Deposit toggle
  → Deposit Details section appears
  → "Escrow Setup" panel appears in Create mode
  → Shows wallet connect buttons (or "No wallets detected")

User fills deposit fields:
  - USDC Amount (optional, min 0.01 if > 0)
  - THB Amount (optional)
  - PromptPay ID (optional)
  - Refund Deadline (min 1 hour)

User does NOT connect wallet
User clicks "Create Event"
  → Validate: name, slug, sheet_id + deposit fields
  → POST /api/events → backend creates event (escrow_address = "")
  → Toast: "Event 'X' created"
  → Events list shows: ⚠ No Escrow badge
```

**State after:** Event exists in backend with `deposit_enabled=true`, `escrow_address=""`. Organizer must Edit → Init Escrow later.

### 3C. Create Event WITH Deposit — Wallet Connected (Combined Flow)

```
User clicks "+ Create Event"
User enables Deposit toggle
User fills deposit fields
User clicks "Connect Phantom" in Escrow Setup panel
  → Wallet popup → user approves
  → Shows "Phantom connected: ABC...XYZ" + Disconnect button
  → Save button changes to "Create Event + Initialize Escrow"

User clicks "Create Event + Initialize Escrow"
  → Validate: name, slug, sheet_id + deposit fields
  → Step 1: POST /api/events → backend creates event → get event_id
  → Step 2: POST /api/escrow/init → backend builds unsigned TX → get transaction
  → Step 3: wallet.signAndSendTransaction(TX) → wallet popup
```

**Sub-scenarios at Step 3:**

#### 3C-i. User APPROVES wallet signature ✅

```
  → On-chain escrow created
  → Step 4: PUT /api/events/{id} → saves escrow_address, organizer_wallet, on_chain_event_id
  → Toast: "Event 'X' created + escrow initialized"
  → Events list shows: 🏦 Escrow badge
```

#### 3C-ii. User REJECTS wallet signature ❌

```
  → signAndSendTransaction returns None
  → Event IS already created (backend committed, cannot rollback)
  → Toast (yellow): "Event 'X' created, but escrow TX was rejected. Edit event to retry."
  → Events list shows: ⚠ No Escrow badge
```

**Recovery path:** Edit event → scroll to deposit section → EscrowInitPanel → connect wallet → "Create & Sign"

#### 3C-iii. init_escrow API fails (e.g. backend error) ❌

```
  → Event IS already created
  → Toast (yellow): "Event 'X' created, but escrow init failed: {error}. Edit event to retry."
  → Events list shows: ⚠ No Escrow badge
```

#### 3C-iv. create_event API fails ❌

```
  → No event created
  → Toast (red): "Failed to create event: {error}"
  → User stays on Create form (can retry)
```

---

## 4. Edit Event Scenarios

### 4A. Edit Event — No Deposit

```
User clicks "Edit" on event card
  → GET /api/events/{id} → loads full detail
  → Form populated with all fields
  → Deposit toggle = OFF

User edits fields, clicks "Update Event"
  → PUT /api/events/{id} → backend updates
  → Toast: "Event 'X' updated"
  → Return to events list
```

### 4B. Edit Event — Deposit Enabled, No Escrow Yet

```
User clicks "Edit" on event with ⚠ No Escrow badge
  → Form loads, deposit toggle = ON
  → Deposit fields filled
  → EscrowInitPanel shows (no escrow_address)
  → Status selector visible (draft/active/completed)

User has two paths:

Path 1: Just update event settings (no escrow)
  → Edit fields, click "Update Event"
  → PUT /api/events/{id}
  → Still shows ⚠ No Escrow

Path 2: Initialize escrow
  → Scroll to Escrow Setup panel
  → Click "Connect Phantom" → wallet popup → approve
  → Click "Create & Sign"
    → Auto-saves event first (PUT /api/events/{id} with deposit fields)
    → POST /api/escrow/init → get unsigned TX
    → wallet.signAndSendTransaction → wallet popup

    If approved:
      → Form updates: escrow_address, on_chain_event_id filled
      → Shows green "Escrow initialized" success panel
      → Still need to click "Update Event" to save final state

    If rejected:
      → Shows red error: "Escrow transaction rejected or failed"
      → Can click "Retry" to try again
```

### 4C. Edit Event — Deposit Enabled, Escrow Already Initialized

```
User clicks "Edit" on event with 🏦 Escrow badge
  → Form loads, deposit toggle = ON
  → Escrow fields show as LOCKED (readonly badges):
    - Escrow Address: [base58...] 🔒 Locked
    - Organizer Wallet: [base58...] 🔒 Locked
    - On-Chain Event ID: 123 🔒 Locked
  → Green banner: "Escrow initialized: [address]"

User can still edit:
  - Deposit amounts (USDC/THB) ← editable
  - PromptPay ID ← editable
  - Refund deadline ← editable
  - All other event fields

User clicks "Update Event"
  → PUT /api/events/{id}
  → Toast: "Event 'X' updated"
```

### 4D. Edit Event — Enable Deposit After Creation

```
User edits an event that was created without deposit
  → Deposit toggle = OFF
  → No deposit section visible

User toggles Deposit ON
  → Deposit Details section appears
  → Escrow Setup panel appears (no escrow yet)
  → Fields: USDC Amount, THB Amount, PromptPay ID, Refund Deadline

User fills deposit fields, clicks "Update Event"
  → Validates deposit fields (min USDC 0.01 if > 0, min deadline 1hr)
  → PUT /api/events/{id} → saves with deposit_enabled=true
  → Events list now shows: ⚠ No Escrow badge
  → Organizer must Edit again → Init Escrow to complete setup
```

### 4E. Edit Event — Disable Deposit After Escrow Created

```
User edits event with 🏦 Escrow badge
User toggles Deposit OFF
  → Deposit section hides
  → Escrow fields not visible

User clicks "Update Event"
  → PUT /api/events/{id} → saves with deposit_enabled=false
  → Events list shows: no deposit badge (escrow still exists on-chain but inactive)

Note: On-chain escrow PDA still exists but backend won't route deposits to it.
      If deposit is re-enabled later, the existing escrow_address is preserved.
```

---

## 5. Validation Rules Summary

### On Save (Create or Update)

| Field | Rule | When | Message |
|-------|------|------|--------|
| `name` | Not empty | Always | "Event name is required" |
| `slug` | Not empty | Always | "Event slug is required" |
| `sheet_id` | Not empty | Always | "Google Sheet ID is required" |
| `event_start` | Must be set (> 0) | Always | "Event start date is required" |
| `event_end` | Must be set (> 0) | Always | "Event end date is required" |
| `event_end` vs `event_start` | end > start | Always | "Event end must be after event start" |
| `deposit_amount_usdc` + `deposit_amount_thb` | At least one > 0 | `deposit_enabled=true` | "At least one deposit amount (USDC or THB) is required when deposit is enabled" |
| `deposit_amount_usdc` | ≥ 0.01 if > 0 | `deposit_enabled=true` | "Minimum deposit is 0.01 USDC" |
| `deposit_amount_usdc` | ≤ 1,000 (SEC-003) | `deposit_enabled=true` | "Maximum deposit is 1,000 USDC" |
| `deposit_amount_usdc` | > 0 (required for escrow) | `deposit_enabled=true` + wallet connected | "USDC deposit amount is required to initialize on-chain escrow" |
| `promptpay_id` | Not empty when THB > 0 | `deposit_amount_thb > 0` | "PromptPay ID is required when THB amount is set" |
| `refund_deadline_hours` | ≥ 1 | `deposit_enabled=true` | "Refund deadline must be at least 1 hour" |

### Inline Warnings (Yellow Hints)

| Field | Condition | Hint |
|-------|-----------|------|
| `event_end` | end ≤ start (both set) | "Event end must be after event start" |
| `deposit_amount_usdc` | Both USDC = 0 and THB = 0 while deposit enabled | "At least one deposit amount (USDC or THB) is required" |
| `deposit_amount_usdc` | Value > 0 and < 0.01 | "Minimum deposit is 0.01 USDC" |
| `deposit_amount_usdc` | Value > 1,000 | "Maximum deposit is 1,000 USDC (SEC-003 cap)" |
| `promptpay_id` | THB > 0 but PromptPay ID empty | "PromptPay ID is required when THB amount is set" |
| `refund_deadline_hours` | Value = 0 while deposit enabled | "Refund deadline must be at least 1 hour" |
| `nft_name_template` | Resolved name > 32 chars | "Resolved name exceeds 32-char limit..." |

### Locked Fields (Read-Only)

| Field | When | Why |
|-------|------|-----|
| `escrow_address` | Always (when set) | Set by on-chain escrow init, never user-editable |
| `organizer_wallet` | Always (when set) | Set by wallet connect during escrow init |
| `on_chain_event_id` | Always (when set & > 0) | Auto-derived from PDA seeds |

---

## 6. Button Label Logic

| Mode | Condition | Button Label |
|------|-----------|-------------|
| Create | Wallet connected + deposit enabled | "Create Event + Initialize Escrow" |
| Create | No wallet or no deposit | "Create Event" |
| Edit | Any | "Update Event" |

While saving: "Saving..." (disabled)

---

## 7. Toast Message Summary

| Scenario | Type | Message |
|----------|------|---------|
| Event created successfully | Success | "Event 'X' created" |
| Event created + escrow initialized | Success | "Event 'X' created + escrow initialized" |
| Event created, escrow TX rejected | Warning | "Event 'X' created, but escrow TX was rejected. Edit event to retry." |
| Event created, escrow init failed | Warning | "Event 'X' created, but escrow init failed: {err}. Edit event to retry." |
| Event create failed | Error | "Failed to create event: {err}" |
| Event updated successfully | Success | "Event 'X' updated" |
| Event update failed | Error | "Failed to update event: {err}" |
| Event archived | Success | "Event 'X' archived" |
| Archive failed | Error | "Failed to archive: {err}" |
| Validation: name empty | Error | "Event name is required" |
| Validation: slug empty | Error | "Event slug is required" |
| Validation: sheet_id empty | Error | "Google Sheet ID is required" |
| Validation: start date empty | Error | "Event start date is required" |
| Validation: end date empty | Error | "Event end date is required" |
| Validation: end before start | Error | "Event end must be after event start" |
| Validation: USDC too small | Error | "Minimum deposit is 0.01 USDC" |
| Validation: USDC too large | Error | "Maximum deposit is 1,000 USDC" |
| Validation: no deposit amount set | Error | "At least one deposit amount (USDC or THB) is required when deposit is enabled" |
| Validation: no USDC for escrow | Error | "USDC deposit amount is required to initialize on-chain escrow" |
| Validation: THB without PromptPay | Error | "PromptPay ID is required when THB amount is set" |
| Validation: deadline = 0 | Error | "Refund deadline must be at least 1 hour" |
| Wallet connection rejected | Error | "Wallet connection rejected" |
| No wallets detected | Info | "No Solana wallets detected. Install Phantom or another wallet extension." |

---

## 8. Attendee Deposit Flow (For Reference)

When an attendee accesses `/deposit/{attendee_id}?event_id=xxx`:

```
GET /api/deposit/status/{attendee_id}?event_id={event_id}

If deposit_enabled=false → "Deposit not required for this event"

If already deposited → Show deposit status (amount, TX, timestamp)

If not deposited → Show payment options:
  USDC: Connect wallet → GET /api/deposit/usdc → sign TX → on-chain deposit
  USDC QR: Solana Pay QR code → scan with mobile wallet
  THB: Upload slip URL → admin verifies manually
```

**Key constraint:** Attendee deposits exactly `deposit_amount_usdc` (fixed by program). No partial deposits possible. If attendee has insufficient USDC → TX fails atomically → nothing happens.

---

## 9. State Transition Diagram

```
Event States:
  Draft → Active → Completed → Archived

Deposit States:
  Disabled → Enabled (no escrow) → Enabled (escrow active)
              ↑                       ↓
              └── Can disable ────────┘ (escrow preserved on-chain)

Escrow Init States (EscrowInitPanel):
  Idle → WalletConnected → Initializing → Done
   ↑                                   ↓
   └──────────── Error ────────────────┘ (retry available)
```

---

## 10. Edge Cases & Gotchas

| Edge Case | Behavior |
|-----------|----------|
| Create event with deposit but wallet extension not installed | Shows "No Solana wallets detected". Button says "Create Event" (not combined). Event created without escrow. |
| Wallet connected in Create mode, then disconnect before saving | Button reverts to "Create Event". Escrow init step skipped. |
| Escrow already exists when trying to init | Backend returns "already has escrow" error → Panel reloads event detail and shows success state. |
| Organizer enables deposit, saves, but never inits escrow | Events list shows ⚠ No Escrow. Attendee deposit page shows "Deposit not enabled" (backend checks escrow_address). |
| Edit event and change deposit_amount after deposits already made | New amount applies to future deposits only. Existing deposits unchanged. |
| Network error during combined create+escrow flow | If create succeeded: event exists without escrow. If create failed: no event created. No partial state. |
| User has multiple wallet extensions | All detected wallets shown as separate "Connect X" buttons. User picks one. |
| Browser tab closed during wallet signature prompt | Event already created (if past Step 1). Escrow not initialized. Edit later to retry. |
| Event archived with active escrow | Backend rejects: SEC-004 blocks archive if escrow_address is set. Must close escrow on-chain first. |
| Two admins editing same event simultaneously | Last-write-wins. KV storage has no locking. Could lose changes. |
| Duplicate slug on create | Backend rejects: "event with id '{slug}' already exists". Frontend has no pre-check. |
| Wallet on wrong network (e.g. mainnet vs devnet) | TX fails on-chain with confusing error. No frontend network detection. |

---

## 11. Backend-Only Validations (Not Caught by Frontend)

These are enforced by the backend but the frontend does not have inline warnings for them:

| Rule | Backend Check | When |
|------|--------------|------|
| SEC-002: Lock escrow fields after init | Rejects changes to `organizer_wallet`, `on_chain_event_id`, `deposit_amount_usdc`, `refund_deadline_hours` when `escrow_address` is set | `update_event` |
| SEC-003: Max deposit cap $1,000 | `deposit_amount_usdc > 1,000,000,000` → rejected | `create_event` + `update_event` |
| SEC-004: Block archive with active escrow | Rejects archive if `escrow_address` is set | `archive_event` |
| Duplicate slug/ID check | `events.iter().any(\|e\| e.id == id)` | `create_event` |
| Deposit amount not configured | `deposit_amount_usdc == 0` → rejected | `init_escrow_tx_handler` |
| Invalid organizer_wallet | Must be valid base58 Solana address | `init_escrow_tx_handler` |

---

## 12. Known Gaps & Future Improvements

| Gap | Severity | Notes |
|-----|----------|-------|
| No wallet network detection | Medium | User could sign on wrong cluster. Add `connection.getGenesisHash()` check. |
| No concurrent edit protection | Low | KV is last-write-wins. Could add `updated_at` optimistic concurrency check. |
| No duplicate slug pre-check | Low | Backend catches it, but could check on slug input blur for better UX. |
| NFT fields have no format validation | Low | `nft_collection_mint` could validate base58, URLs could validate format. |
| No event end → refund deadline visual timeline | Low | Could show "Refund deadline: Jan 15, 2026" based on end + hours. |
| Schedule section defaults to collapsed | Medium | Since schedule is now required, should default to expanded on Create. |
