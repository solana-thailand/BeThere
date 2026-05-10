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
| `No Escrow` badge (yellow, CSS-only) | `deposit_enabled=true` AND `escrow_address=""` | Deposit is enabled but on-chain escrow PDA not yet created. Organizer must Edit → Init Escrow. |
| `Escrow` badge (green, CSS-only) | `deposit_enabled=true` AND `escrow_address≠""` | On-chain escrow is active and ready to accept deposits. |
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

> **Note:** The events page displays a computed refund deadline datetime below the `refund_deadline_hours` input field. The value is calculated as `event_end_ms + (refund_deadline_hours × 3_600_000)` and rendered with a human-friendly duration label (e.g. "Refund deadline: Jan 15, 2026, 11:59 PM — 48 hours after event end").

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
| ~~No concurrent edit protection~~ (✅ Fixed) | ~~Low~~ | Optimistic concurrency implemented: backend checks `expected_updated_at` against stored `updated_at`, returns conflict error on mismatch. Frontend detects `"conflict"` in error message and shows a user-friendly toast prompting refresh. |
| No duplicate slug pre-check | Low | Backend catches it, but could check on slug input blur for better UX. |
| ~~NFT fields have no format validation~~ (✅ Fixed) | ~~Low~~ | `nft_collection_mint` validates base58, URLs validate format. Added in `a0ca1b5`.
| ~~No event end → refund deadline visual timeline~~ (✅ Fixed) | ~~Low~~ | Events page now shows a computed refund deadline datetime below the `refund_deadline_hours` input. Calculation: `event_end_ms + (hours × 3_600_000)`, displayed with a human-friendly duration label. |
| ~~Schedule section defaults to collapsed~~ (✅ Fixed) | ~~Medium~~ | `sec_schedule_open` now initialized to `true`; schedule section defaults to expanded on Create. |
| ~~No NFT badge preview~~ (✅ Fixed) | ~~Low~~ | Events page shows live badge preview + "Use default badge" auto-fill button. `30a23e0`.
| ~~No walk-in attendee registration~~ (✅ Fixed) | ~~Medium~~ | Staff can register walk-ins via scanner UI → KV record → same deposit/NFT/refund flow. `ef70bca`. |
| No public event page (`/e/{slug}`) | ~~**High**~~ | ✅ **Implemented** — `GET /api/public/event/{slug}` + `/e/:slug` frontend. Sanitized response (no sensitive fields). Only Active/Completed events shown. See `docs/ux_roadmap.md` P0-1. |
| No event context on deposit page | **High** | Deposit page jumps straight to payment without showing what event the deposit is for. Needs event header with name + date. See `docs/ux_roadmap.md` P0-2. |
| Scanner has no haptic/audio feedback | Medium | No vibration or sound on scan success/failure. Critical for throughput at real events. `navigator.vibrate()` + short beep. See `docs/ux_roadmap.md` P1-1. |
| No progress indicator on claim flow | Medium | Multi-step flow (connect → deposit → quiz → claim) with no visible step indicator. See `docs/ux_roadmap.md` P1-2. |
| No share CTA on NFT mint success | Low | Missing free marketing — "Share your badge" button after NFT mint. See `docs/ux_roadmap.md` P1-3. |

---

## 13. Behavioral Economics UX Patterns

The deposit page and events form incorporate behavioral science patterns to increase attendance and deposit uptake.

### 13A. Loss Aversion — Deposit Toggle Nudge (Events Page)

**Location:** `events_page.rs` — Deposit toggle section

When an organizer toggles deposits **OFF**, a yellow hint appears:

> "Events without deposits often see 30-40% no-shows. Deposits reduce no-shows by making attendance the default — attendees get 100% back just by showing up."

**Pattern:** Loss aversion (Kahneman & Tversky) — framing the cost of NOT having deposits as a loss.

### 13B. Loss Aversion — Refund CTA Language (Deposit Page)

**Location:** `deposit.rs` — `Deposited` state, `RefundChooseWallet` state

After deposit is verified, the refund button uses loss-framed language:

- Button: `"💸 Don't lose your {X} USDC — claim it now"`
- After event: `"💰 Refund window: {duration} after event ends ({deadline})."`
- Refund button: `"💸 Claim {X} USDC — Don't lose it"`

**Pattern:** Loss aversion — phrasing the refund as "don't lose" (loss frame) rather than "get back" (gain frame). Users are ~2x more sensitive to losses than equivalent gains.

### 13C. Endowment Effect — Deposit Confirmation (Deposit Page)

**Location:** `deposit.rs` — `Deposited` state header

The deposit confirmation card uses:

- Title: `"🎫 Spot Reserved!"`
- Message: `"Your {X} USDC is secured on-chain. Show up → get it all back."`
- Rent reclamation: `"♻️ You have ~0.002 SOL waiting — reclaim it"`

**Pattern:** Endowment effect — making the attendee feel ownership of the reserved spot and deposited funds. "Your USDC is secured" (not "the deposit is held").

### 13D. Commitment Device — Deposit Flow Architecture

The entire deposit architecture acts as a commitment device:

1. Attendee deposits USDC → psychological commitment to attend
2. NFT badge is the reward for showing up (commitment + endowment)
3. Full refund on attendance = zero cost for following through
4. Loss of deposit = cost of not following through

The NFT section on the events page is labeled **"Recommended"** (green badge) because NFTs complete the commitment device loop: deposit → show up → get NFT + refund.

### 13E. Refund Success — Positive Reinforcement (Deposit Page)

**Location:** `deposit.rs` — `RefundConfirmed` state

On successful refund:

- Title: `"🎉 Refund Recovered!"`
- Celebration emoji: 💰
- Message: `"Your refund has been confirmed on Solana."`

**Pattern:** Positive reinforcement with "recovered" language — emphasizes regaining what was theirs, reinforcing the commitment device for future events.

### Pattern Summary

| Pattern | Location | Technique | Behavioral Principle |
|---------|----------|-----------|---------------------|
| Deposit nudge | Events form | Yellow hint when deposit OFF | Loss aversion |
| Refund CTA | Deposit page | "Don't lose your X USDC" | Loss aversion |
| Spot reserved | Deposit page | "Your USDC is secured" | Endowment effect |
| NFT = Recommended | Events form | Green badge on NFT section | Commitment device |
| Refund deadline | Deposit page | Countdown + urgency framing | Scarcity/urgency |
| Refund recovered | Deposit page | "🎉 Refund Recovered!" | Positive reinforcement |

---

## 14. Walk-in Attendee Flow

Walk-in attendees (people who show up without pre-registering) are handled via a hybrid KV-based approach.

### 14A. Walk-in Registration (Staff → Scanner UI)

**Endpoint:** `POST /api/walkin/register` (staff-only, requires auth)

```
Staff taps "Register Walk-in" in scanner UI
  → Form: name (required), email (required), phone (optional)
  → Backend creates KV record: walkin:{event_id}:{email_lower}
  → Generates UUID v7 claim token
  → Creates reverse mapping: claim_walkin:{token} → {event_id}:{email_lower}
  → Returns claim token + claim URL to staff UI
  → Staff shows QR code of claim URL to walk-in
```

### 14B. Walk-in Claim Flow (Attendee Side)

Walk-ins follow the same deposit/NFT/refund loop as pre-registered attendees:

```
Walk-in scans claim QR
  → lookup_claim() checks claim_walkin:{token} reverse mapping
  → Loads walkin:{event_id}:{email} KV record
  → execute_walkin_claim() mints NFT + updates KV (no Sheet write)
  → Walk-in gets NFT badge
```

Deposit and refund are wallet-based (on-chain PDA linked to wallet address) — independent of attendee records.

### 14C. Walk-in KV Key Patterns

| Purpose | Key | Value | TTL |
|---------|-----|-------|-----|
| Walk-in record | `walkin:{event_id}:{email_lower}` | `WalkinAttendee` JSON | 90 days |
| Reverse mapping | `claim_walkin:{claim_token}` | `{event_id}:{email_lower}` | 90 days |

### 14D. Walk-in vs Pre-registered Differences

| Aspect | Pre-registered | Walk-in |
|--------|---------------|---------|
| Data source | Google Sheet sync | KV (staff-entered) |
| Quiz/Adventure gates | Subject to gates | **Skipped** (auto check-in) |
| Check-in | Staff scans QR | **Automatic** at registration |
| Sheet record | Yes | No (Phase 4: optional sync) |
| Claim token source | Sheet sync generates | Registration endpoint generates |
| NFT minting | Same | Same |
| Deposit/Refund | Same (wallet-based) | Same (wallet-based) |

### 14E. Scanner UI States (Walk-in)

```
Idle → "Register Walk-in" button
  → WalkinForm (name, email, phone inputs)
  → WalkinRegistering (spinner)
  → WalkinSuccess (QR code of claim URL + "Scan Another" button)
  → Idle (loop)
```

### Files

| File | Role |
|------|------|
| `domain/src/models/attendee.rs` | `WalkinAttendee` struct |
| `worker/src/handlers/walkin.rs` | `POST /api/walkin/register` handler |
| `worker/src/claim.rs` | Walk-in claim lookup + execution |
| `frontend-leptos/src/api.rs` | `register_walkin()` + request/response types |
| `frontend-leptos/src/pages/scanner.rs` | Walk-in form, QR display, state management |

---

## 15. Self-Hosted NFT Badge System

The worker serves its own badge image and dynamic metadata — no Arweave/IPFS upload required for basic minting.

### Endpoints

| Route | Purpose |
|-------|---------|
| `GET /api/badge-hd.svg` | 1000×1000 production SVG (hexagonal shield + checkmark + Solana gradient) |
| `GET /api/metadata/{event_id}` | Dynamic Metaplex-compliant metadata JSON (loads from KV, falls back to global config) |

### Configuration

Organizers set these in the admin UI event form:

```
nft_image_url      = https://bethere.solana-thailand.workers.dev/api/badge-hd.svg
nft_metadata_uri   = https://bethere.solana-thailand.workers.dev/api/metadata/{event_id}
nft_name_template  = BeThere - {event_name}
nft_symbol         = BETHERE
```

The events page provides a **"Use default badge"** button that auto-fills these URLs. A live badge preview is shown next to the image URL input.

### Dynamic Metadata

The metadata endpoint loads per-event configuration from KV:

- **Falls back** to global worker config if event-specific fields aren't set
- **Auto-includes** event name and date as NFT traits
- **Metaplex-compliant** — `name`, `symbol`, `description`, `image`, `attributes` fields

Organizers can override with any custom URL (Arweave, IPFS, CDN) for permanent storage.
