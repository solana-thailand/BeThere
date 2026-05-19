# Business Flow Scenarios — Create/Edit Event Page

> All scenarios for what happens when a user (organizer/admin) interacts with the Create/Edit Event page,
> including event format selection, deposit configuration, escrow initialization, and error paths.

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

### Format Badge (always visible)

| Badge | Condition | Meaning |
|-------|-----------|---------|
| `📍 In-Person` | `event_format = "in_person"` | Physical event, deposit required, check-in via scanner |
| `💻 Online` | `event_format = "online"` | Virtual event, no deposit, NFT claim via quiz/adventure |
| `🔄 Hybrid` | `event_format = "hybrid"` | Both physical + virtual tracks, see escrow badge for deposit status |

### Escrow Badge (only for in-person / hybrid)

| Indicator | Condition | Meaning |
|-----------|-----------|---------|
| `No Escrow` badge (yellow) | Format includes in-person AND `escrow_address=""` | Deposit required but on-chain escrow PDA not yet created. Organizer must Edit → Init Escrow. |
| `Escrow` badge (green) | Format includes in-person AND `escrow_address≠""` | On-chain escrow is active and ready to accept deposits. |
| No escrow badge | Format = online | No deposit feature — escrow not applicable. |

---

## 3. Create Event Scenarios

### Event Format Selector (required field)

```
Form loads with Event Format dropdown:
  - 📍 In-Person
  - 💻 Online
  - 🔄 Hybrid

Selecting a format controls which sections appear/disappear:
  - In-Person → Deposit section (required), Escrow Setup panel, Scanner/Check-in enabled
  - Online → No deposit section, Quiz/Adventure config section, No escrow
  - Hybrid → Deposit section (for in-person track), Quiz/Adventure config, Both claim paths
```

### 3A. Create Event — In-Person (replaces "WITH Deposit")

> **Format = In-Person** → deposit is **REQUIRED**, not optional. The deposit section auto-appears and cannot be dismissed.

```
User clicks "+ Create Event"
  → Form loads with Event Format dropdown
  → User selects "📍 In-Person"
  → Deposit Details section appears (required)
  → Escrow Setup panel appears

User fills:
  - Name* (slug auto-generated)
  - Sheet ID* (required)
  - Event Format* = "In-Person" (already selected)
  - Schedule, NFT, People (optional sections, collapsed by default)
  - USDC Amount (required, min 0.01)
  - THB Amount (optional)
  - PromptPay ID (required if THB > 0)
  - Refund Deadline (min 1 hour)

--- Sub-path 1: Wallet NOT Connected ---

User clicks "Create Event"
  → Validate: name, slug, sheet_id + deposit fields
  → POST /api/events → backend creates event (escrow_address = "")
  → Toast: "Event 'X' created (In-Person)"
  → Events list shows: 📍 In-Person + ⚠ No Escrow badge

**State after:** Event exists with `event_format="in_person"`, `deposit_enabled=true`, `escrow_address=""`. Organizer must Edit → Init Escrow later.

--- Sub-path 2: Wallet Connected (Combined Flow) ---

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

#### 3A-i. User APPROVES wallet signature ✅

```
  → On-chain escrow created
  → Step 4: PUT /api/events/{id} → saves escrow_address, organizer_wallet, on_chain_event_id
  → Toast: "Event 'X' created + escrow initialized"
  → Events list shows: 📍 In-Person + 🏦 Escrow badge
```

#### 3A-ii. User REJECTS wallet signature ❌

```
  → signAndSendTransaction returns None
  → Event IS already created (backend committed, cannot rollback)
  → Toast (yellow): "Event 'X' created, but escrow TX was rejected. Edit event to retry."
  → Events list shows: 📍 In-Person + ⚠ No Escrow badge
```

**Recovery path:** Edit event → scroll to deposit section → EscrowInitPanel → connect wallet → "Create & Sign"

#### 3A-iii. init_escrow API fails (e.g. backend error) ❌

```
  → Event IS already created
  → Toast (yellow): "Event 'X' created, but escrow init failed: {error}. Edit event to retry."
  → Events list shows: 📍 In-Person + ⚠ No Escrow badge
```

#### 3A-iv. create_event API fails ❌

```
  → No event created
  → Toast (red): "Failed to create event: {error}"
  → User stays on Create form (can retry)
```

### 3B. Create Event — Online (NEW)

> **Format = Online** → no deposit, no escrow, no check-in. NFT claim via quiz/adventure only.

```
User clicks "+ Create Event"
  → Form loads with Event Format dropdown
  → User selects "💻 Online"
  → Deposit section does NOT appear
  → Escrow Setup panel does NOT appear
  → Quiz/Adventure config section appears (for NFT claim gating)

User fills:
  - Name* (slug auto-generated)
  - Sheet ID* (required — one sheet for all online registrations)
  - Event Format* = "Online" (already selected)
  - Schedule (start/end dates required)
  - NFT config (badge name, image, template)
  - Quiz/Adventure config (optional — gates for NFT claim)

User clicks "Create Event"
  → Validate: name, slug, sheet_id, dates (no deposit validations)
  → POST /api/events → backend creates event
  → Toast: "Event 'X' created (Online)"
  → Events list shows: 💻 Online badge
  → Return to events list
```

**Validations:**
- Name must not be empty
- Slug must not be empty
- Sheet ID must not be empty
- Start/end dates required
- No deposit validations (deposit not applicable)
- Quiz/Adventure config optional (if not set, NFT claim is ungated)

**State after:** Event exists with `event_format="online"`, `deposit_enabled=false`, no escrow. Attendees register via `/e/{slug}`, complete quiz/adventure, claim NFT.

### 3C. Create Event — Hybrid (NEW)

> **Format = Hybrid** → in-person attendees get deposit flow, online attendees get quest-based NFT claim. One Google Sheet with `participation_type` column differentiates them.

```
User clicks "+ Create Event"
  → Form loads with Event Format dropdown
  → User selects "🔄 Hybrid"
  → Deposit Details section appears (for in-person track)
  → Escrow Setup panel appears
  → Quiz/Adventure config section appears (for online track)
  → Sheet config note: "Ensure your Google Sheet has a 'participation_type' column (values: 'in_person' or 'online')"

User fills:
  - Name* (slug auto-generated)
  - Sheet ID* (required — one sheet with participation_type column)
  - Event Format* = "Hybrid" (already selected)
  - Schedule, NFT, People (standard sections)
  - Deposit fields (USDC Amount required, for in-person attendees)
  - Quiz/Adventure config (for online attendees)
  - Participation Type Column config (defaults to "participation_type")

--- Sub-paths mirror In-Person for escrow setup ---

User clicks "Create Event" (or "Create Event + Initialize Escrow" if wallet connected)
  → Same escrow sub-paths as 3A (3A-i through 3A-iv)
  → Toast: "Event 'X' created (Hybrid)"
  → Events list shows: 🔄 Hybrid + escrow badge status
```

**Validations:**
- Same as In-Person for deposit fields (deposit required for in-person track)
- Quiz/Adventure config optional
- `participation_type_column` defaults to `"participation_type"` if not specified

**State after:** Event exists with `event_format="hybrid"`, `deposit_enabled=true` (for in-person), escrow pending or active. Both claim paths (deposit-based and quest-based) active.

---

## ~~3A. Create Event WITHOUT Deposit~~ (DEPRECATED)

> ⚠️ **Deprecated** — Replaced by Event Format selector. Creating an event "without deposit" is now explicitly choosing "Online" format. See Section 3B above.

## ~~3B. Create Event WITH Deposit — Wallet NOT Connected~~ (DEPRECATED)

> ⚠️ **Deprecated** — Replaced by In-Person format (3A), Sub-path 1. Wallet not connected is now a sub-path of In-Person creation, not a separate scenario.

## ~~3C. Create Event WITH Deposit — Wallet Connected (Combined Flow)~~ (DEPRECATED)

> ⚠️ **Deprecated** — Replaced by In-Person format (3A), Sub-path 2. The combined create+escrow flow is now the default In-Person + wallet connected path.

---

## 4. Edit Event Scenarios

### 4A. Edit Event — Online (No Deposit)

```
User clicks "Edit" on 💻 Online event card
  → GET /api/events/{id} → loads full detail
  → Form populated with all fields
  → Event Format = "Online" (dropdown)
  → No deposit section visible
  → Quiz/Adventure config section visible

User edits fields, clicks "Update Event"
  → PUT /api/events/{id} → backend updates
  → Toast: "Event 'X' updated"
  → Return to events list
```

### 4B. Edit Event — In-Person, No Escrow Yet

```
User clicks "Edit" on 📍 In-Person event with ⚠ No Escrow badge
  → Form loads, Event Format = "In-Person"
  → Deposit fields filled (required)
  → EscrowInitPanel shows (no escrow_address)
  → Status selector visible (draft/active/completed)

User has two paths:

Path 1: Just update event settings (no escrow)
  → Edit fields, click "Update Event"
  → PUT /api/events/{id}
  → Still shows 📍 In-Person + ⚠ No Escrow

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

### 4C. Edit Event — In-Person, Escrow Already Initialized

```
User clicks "Edit" on 📍 In-Person event with 🏦 Escrow badge
  → Form loads, Event Format = "In-Person"
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

### 4D. Edit Event — Hybrid, Manage Both Tracks

```
User clicks "Edit" on 🔄 Hybrid event card
  → Form loads, Event Format = "Hybrid"
  → Deposit section visible (for in-person track)
  → Quiz/Adventure config visible (for online track)
  → Escrow status depends on escrow_address (same as 4B/4C)

User can edit:
  - Deposit fields (in-person track)
  - Quiz/Adventure config (online track)
  - Participation type column name
  - All standard event fields
```

### ~~4D. Edit Event — Enable Deposit After Creation~~ (DEPRECATED)

> ⚠️ **Deprecated** — Deposits are no longer toggled on/off. They are automatically determined by event format. To "enable deposit" on an existing online event, change its format to In-Person or Hybrid (see 4E).

### ~~4E. Edit Event — Disable Deposit After Escrow Created~~ (DEPRECATED)

> ⚠️ **Deprecated** — Replaced by 4E below (Change Format).

### 4E. Edit Event — Change Format

> **Format transitions have constraints** to protect existing deposits, escrow, and attendee records.

| Transition | Allowed? | Conditions |
|------------|----------|------------|
| Online → In-Person | ✅ | Only if escrow not yet needed (no in-person attendees with deposits). Must configure deposit fields + init escrow. |
| Online → Hybrid | ✅ | Same as Online → In-Person for deposit setup. Online attendees unaffected. |
| In-Person → Online | ⚠️ | Only if **no deposits exist** (no attendee deposits recorded). Escrow PDA remains on-chain but inactive. |
| In-Person → Hybrid | ✅ | Always allowed. Existing in-person attendees unaffected. Must add participation_type column to sheet. |
| Hybrid → In-Person | ⚠️ | Only if **no online attendees exist** (or all online attendees have already claimed NFTs). |
| Hybrid → Online | ⚠️ | Only if **no in-person deposits exist**. Escrow PDA remains on-chain but inactive. |

```
User edits event, changes Event Format dropdown
  → If transition is blocked:
    → Red warning: "Cannot change to {format}: {reason} (e.g. existing deposits must be refunded first)"
    → Format dropdown reverts to current value
  → If transition is allowed:
    → Sections appear/disappear based on new format
    → If new format requires deposit: deposit section appears, escrow setup required
    → If new format removes deposit: deposit section hides, existing escrow preserved on-chain
    → User must fill any newly required fields before saving
    → Click "Update Event"
    → PUT /api/events/{id} with new event_format + required fields
    → Toast: "Event 'X' updated (now {format})"
```

---

## 5. Validation Rules Summary

### On Save (Create or Update)

| Field | Rule | When | Message |
|-------|------|------|---------|
| `name` | Not empty | Always | "Event name is required" |
| `slug` | Not empty | Always | "Event slug is required" |
| `sheet_id` | Not empty | Always | "Google Sheet ID is required" |
| `event_format` | Must be one of: `in_person`, `online`, `hybrid` | Always (create) | "Event format is required" |
| `event_start` | Must be set (> 0) | Always | "Event start date is required" |
| `event_end` | Must be set (> 0) | Always | "Event end date is required" |
| `event_end` vs `event_start` | end > start | Always | "Event end must be after event start" |
| `deposit_amount_usdc` + `deposit_amount_thb` | At least one > 0 | Format = `in_person` or `hybrid` | "At least one deposit amount (USDC or THB) is required for in-person events" |
| `deposit_amount_usdc` | ≥ 0.01 if > 0 | Format = `in_person` or `hybrid` | "Minimum deposit is 0.01 USDC" |
| `deposit_amount_usdc` | ≤ 1,000 (SEC-003) | Format = `in_person` or `hybrid` | "Maximum deposit is 1,000 USDC" |
| `deposit_amount_usdc` | > 0 (required for escrow) | Format includes in-person + wallet connected | "USDC deposit amount is required to initialize on-chain escrow" |
| `promptpay_id` | Not empty when THB > 0 | `deposit_amount_thb > 0` (in-person/hybrid) | "PromptPay ID is required when THB amount is set" |
| `refund_deadline_hours` | ≥ 1 | Format = `in_person` or `hybrid` | "Refund deadline must be at least 1 hour" |

> **Note:** The events page displays a computed refund deadline datetime below the `refund_deadline_hours` input field. The value is calculated as `event_end_ms + (refund_deadline_hours × 3_600_000)` and rendered with a human-friendly duration label (e.g. "Refund deadline: Jan 15, 2026, 11:59 PM — 48 hours after event end"). Only shown when format = in-person or hybrid.

### Inline Warnings (Yellow Hints)

| Field | Condition | Hint |
|-------|-----------|------|
| `event_end` | end ≤ start (both set) | "Event end must be after event start" |
| `deposit_amount_usdc` | Both USDC = 0 and THB = 0 while format includes in-person | "At least one deposit amount (USDC or THB) is required" |
| `deposit_amount_usdc` | Value > 0 and < 0.01 | "Minimum deposit is 0.01 USDC" |
| `deposit_amount_usdc` | Value > 1,000 | "Maximum deposit is 1,000 USDC (SEC-003 cap)" |
| `promptpay_id` | THB > 0 but PromptPay ID empty | "PromptPay ID is required when THB amount is set" |
| `refund_deadline_hours` | Value = 0 while format includes in-person | "Refund deadline must be at least 1 hour" |
| `nft_name_template` | Resolved name > 32 chars | "Resolved name exceeds 32-char limit..." |

### Locked Fields (Read-Only)

| Field | When | Why |
|-------|------|-----|
| `event_format` | After attendees have registered | Changing format with existing attendees requires explicit migration |
| `escrow_address` | Always (when set) | Set by on-chain escrow init, never user-editable |
| `organizer_wallet` | Always (when set) | Set by wallet connect during escrow init |
| `on_chain_event_id` | Always (when set & > 0) | Auto-derived from PDA seeds |

---

## 6. Button Label Logic

| Mode | Condition | Button Label |
|------|-----------|-------------|
| Create | Format = in-person/hybrid + wallet connected | "Create Event + Initialize Escrow" |
| Create | Format = online, or no wallet connected | "Create Event" |
| Edit | Any | "Update Event" |

While saving: "Saving..." (disabled)

---

## 7. Toast Message Summary

| Scenario | Type | Message |
|----------|------|---------|
| Event created successfully (online) | Success | "Event 'X' created (Online)" |
| Event created successfully (in-person, no escrow) | Success | "Event 'X' created (In-Person)" |
| Event created + escrow initialized | Success | "Event 'X' created + escrow initialized" |
| Event created, escrow TX rejected | Warning | "Event 'X' created, but escrow TX was rejected. Edit event to retry." |
| Event created, escrow init failed | Warning | "Event 'X' created, but escrow init failed: {err}. Edit event to retry." |
| Event create failed | Error | "Failed to create event: {err}" |
| Event updated successfully | Success | "Event 'X' updated" |
| Event updated + format changed | Success | "Event 'X' updated (now {format})" |
| Event update failed | Error | "Failed to update event: {err}" |
| Event archived | Success | "Event 'X' archived" |
| Archive failed | Error | "Failed to archive: {err}" |
| Validation: name empty | Error | "Event name is required" |
| Validation: slug empty | Error | "Event slug is required" |
| Validation: sheet_id empty | Error | "Google Sheet ID is required" |
| Validation: event_format empty | Error | "Event format is required" |
| Validation: start date empty | Error | "Event start date is required" |
| Validation: end date empty | Error | "Event end date is required" |
| Validation: end before start | Error | "Event end must be after event start" |
| Validation: USDC too small | Error | "Minimum deposit is 0.01 USDC" |
| Validation: USDC too large | Error | "Maximum deposit is 1,000 USDC" |
| Validation: no deposit amount set | Error | "At least one deposit amount (USDC or THB) is required for in-person events" |
| Validation: no USDC for escrow | Error | "USDC deposit amount is required to initialize on-chain escrow" |
| Validation: THB without PromptPay | Error | "PromptPay ID is required when THB amount is set" |
| Validation: deadline = 0 | Error | "Refund deadline must be at least 1 hour" |
| Validation: format change blocked | Error | "Cannot change to {format}: {reason}" |
| Wallet connection rejected | Error | "Wallet connection rejected" |
| No wallets detected | Info | "No Solana wallets detected. Install Phantom or another wallet extension." |

---

## 8. Attendee Deposit Flow (For Reference)

Attendee arrives via **auto-redirect** after registration ("Reserve Spot" on `/e/{slug}`), or via **resume** from localStorage if they previously started the flow. If attendee already deposited, they are redirected to the ticket/QR page instead.

When an attendee accesses `/deposit/{attendee_id}?event_id=xxx`:

```
GET /api/deposit/status/{attendee_id}?event_id={event_id}

If event_format = "online" → "Deposit not required for this event"

If format includes in-person AND deposit_enabled=false → "Deposit not configured"

If already deposited → Show deposit status (amount, TX, timestamp)

If not deposited → Show payment options:
  USDC: Connect wallet → GET /api/deposit/usdc → sign TX → on-chain deposit
  USDC QR: Solana Pay QR code → scan with mobile wallet
  THB: Upload slip URL → admin verifies manually
  → After THB slip upload: auto-redirect to `/ticket/{attendee_id}?event_id={id}` (shows QR code + pending approval status)
```

**Resume capability**: localStorage stores `{attendee_id, event_id, event_slug}` after registration. Returning to `/e/{slug}` auto-redirects to deposit or ticket page based on current state.

**Dev-mode gating**: USDC payment card is hidden unless the backend returns `dev_mode: true` (via health endpoint and public event endpoint). Non-crypto attendees see THB option only in production.

**Key constraint:** Attendee deposits exactly `deposit_amount_usdc` (fixed by program). No partial deposits possible. If attendee has insufficient USDC → TX fails atomically → nothing happens.

---

## 9. State Transition Diagram

```
Event States:
  Draft → Active → Completed → Archived

Event Format:
  Online ↔ In-Person ↔ Hybrid
    ↑         ↑          ↑
    └─ constrained by existing deposits/attendees ─┘

Deposit States (format = in-person or hybrid only):
  Required → Escrow Pending → Escrow Active
                                ↓
                Escrow preserved on-chain (if format changes to online)

Escrow Init States (EscrowInitPanel):
  Idle → WalletConnected → Initializing → Done
   ↑                                   ↓
   └──────────── Error ────────────────┘ (retry available)
```

---

## 10. Edge Cases & Gotchas

| Edge Case | Behavior |
|-----------|----------|
| Create online event, later change to in-person | Must configure deposit fields + init escrow before saving. |
| Create in-person event with deposit but wallet extension not installed | Shows "No Solana wallets detected". Button says "Create Event" (not combined). Event created without escrow. |
| Wallet connected in Create mode, then disconnect before saving | Button reverts to "Create Event". Escrow init step skipped. |
| Escrow already exists when trying to init | Backend returns "already has escrow" error → Panel reloads event detail and shows success state. |
| Organizer changes format to online, but in-person deposits exist | Blocked with error: "Cannot change to Online: existing deposits must be refunded first" |
| Edit event and change deposit_amount after deposits already made | New amount applies to future deposits only. Existing deposits unchanged. |
| Network error during combined create+escrow flow | If create succeeded: event exists without escrow. If create failed: no event created. No partial state. |
| User has multiple wallet extensions | All detected wallets shown as separate "Connect X" buttons. User picks one. |
| Browser tab closed during wallet signature prompt | Event already created (if past Step 1). Escrow not initialized. Edit later to retry. |
| Event archived with active escrow | Backend rejects: SEC-004 blocks archive if escrow_address is set. Must close escrow on-chain first. |
| Two admins editing same event simultaneously | Last-write-wins. KV storage has no locking. Could lose changes. |
| Duplicate slug on create | Backend rejects: "event with id '{slug}' already exists". Frontend has no pre-check. |
| Wallet on wrong network (e.g. mainnet vs devnet) | TX fails on-chain with confusing error. No frontend network detection. |
| Hybrid event with missing participation_type column | Backend defaults all attendees to "in_person" if column missing. Online attendees treated as in-person. |

---

## 11. Backend-Only Validations (Not Caught by Frontend)

These are enforced by the backend but the frontend does not have inline warnings for them:

| Rule | Backend Check | When |
|------|--------------|------|
| SEC-002: Lock escrow fields after init | Rejects changes to `organizer_wallet`, `on_chain_event_id`, `deposit_amount_usdc`, `refund_deadline_hours` when `escrow_address` is set | `update_event` |
| SEC-003: Max deposit cap $1,000 | `deposit_amount_usdc > 1,000,000,000` → rejected | `create_event` + `update_event` |
| SEC-004: Block archive with active escrow | Rejects archive if `escrow_address` is set | `archive_event` |
| SEC-005: Format transition constraints | Rejects format change if deposits exist (→ online) or online attendees exist (→ in-person) | `update_event` |
| Duplicate slug/ID check | `events.iter().any(\|e\| e.id == id)` | `create_event` |
| Deposit amount not configured | `deposit_amount_usdc == 0` → rejected | `init_escrow_tx_handler` |
| Invalid organizer_wallet | Must be valid base58 Solana address | `init_escrow_tx_handler` |
| Invalid event_format | Must be `in_person`, `online`, or `hybrid` | `create_event` + `update_event` |
| Deposit fields on online event | Rejects deposit fields when `event_format = "online"` | `create_event` + `update_event` |

---

## 12. Known Gaps & Future Improvements

| Gap | Severity | Notes |
|-----|----------|-------|
| No wallet network detection | Medium | User could sign on wrong cluster. Add `connection.getGenesisHash()` check. |
| ~~No concurrent edit protection~~ (✅ Fixed) | ~~Low~~ | Optimistic concurrency implemented: backend checks `expected_updated_at` against stored `updated_at`, returns conflict error on mismatch. Frontend detects `"conflict"` in error message and shows a user-friendly toast prompting refresh. |
| No duplicate slug pre-check | Low | Backend catches it, but could check on slug input blur for better UX. |
| ~~NFT fields have no format validation~~ (✅ Fixed) | ~~Low~~ | `nft_collection_mint` validates base58, URLs validate format. Added in `a0ca1b5`. |
| ~~No event end → refund deadline visual timeline~~ (✅ Fixed) | ~~Low~~ | Events page now shows a computed refund deadline datetime below the `refund_deadline_hours` input. Calculation: `event_end_ms + (hours × 3_600_000)`, displayed with a human-friendly duration label. |
| ~~Schedule section defaults to collapsed~~ (✅ Fixed) | ~~Medium~~ | `sec_schedule_open` now initialized to `true`; schedule section defaults to expanded on Create. |
| ~~No NFT badge preview~~ (✅ Fixed) | ~~Low~~ | Events page shows live badge preview + "Use default badge" auto-fill button. `30a23e0`. |
| ~~No walk-in attendee registration~~ (✅ Fixed) | ~~Medium~~ | Staff can register walk-ins via scanner UI → KV record → same deposit/NFT/refund flow. `ef70bca`. |
| ~~No public event page (`/e/{slug}`)~~ (✅ Fixed) | ~~**High**~~ | ✅ **Implemented** — `GET /api/public/event/{slug}` + `/e/:slug` frontend. Sanitized response (no sensitive fields). Only Active/Completed events shown. See `docs/ux_roadmap.md` P0-1. |
| ~~No self-registration from public event page~~ (✅ Fixed) | ~~**High**~~ | ✅ **Implemented** — `POST /api/public/register` + "Reserve Spot" form on `/e/{slug}`. Attendee fills name + email → backend appends to Google Sheet → claim token issued → auto-redirect to deposit. |
| ~~No online attendee claim path~~ (✅ Fixed) | ~~**High**~~ | ✅ **Implemented** — Quiz completion or adventure finish triggers virtual check-in for online attendees. |
| ~~No event context on deposit page~~ (✅ Fixed) | ~~**High**~~ | ✅ **Implemented** — Deposit page shows event name, date, location header. See `docs/ux_roadmap.md` P0-2. |
| Scanner has no haptic/audio feedback | Medium | No vibration or sound on scan success/failure. Critical for throughput at real events. `navigator.vibrate()` + short beep. See `docs/ux_roadmap.md` P1-1. |
| No progress indicator on claim flow | Medium | Multi-step flow (connect → deposit → quiz → claim) with no visible step indicator. See `docs/ux_roadmap.md` P1-2. |
| No share CTA on NFT mint success | Low | Missing free marketing — "Share your badge" button after NFT mint. See `docs/ux_roadmap.md` P1-3. |
| ~~No auto-redirect after registration~~ (✅ Fixed) | ~~**High**~~ | After "Reserve Spot", attendee is auto-redirected to deposit page (or ticket page if already deposited). No manual "Complete Deposit →" button. |
| ~~No resume capability for partial flows~~ (✅ Fixed) | ~~**High**~~ | localStorage stores `{attendee_id, event_id, event_slug}` after registration. Returning to `/e/{slug}` redirects attendee to their correct step (deposit or ticket). |
| ~~Confusing landing after slip upload~~ (✅ Fixed) | ~~**Medium**~~ | After uploading THB slip, auto-redirect to `/ticket/{attendee_id}?event_id={id}` showing QR code + pending approval status. Replaces "Go Home" button. |
| ~~Solana wallet confusing for non-crypto attendees~~ (✅ Fixed) | ~~**Medium**~~ | USDC payment card hidden in production. Only shown when backend returns `dev_mode: true`. Health endpoint and public event endpoint include `dev_mode`. |
| **~~No attendee identity verification~~** (✅ Fixed) | ~~**🔴 Critical**~~ | ~~Anyone who knows an email can register as that person and access their ticket/QR.~~ Fixed: Google Sign-In required for registration and ticket access. Email locked to JWT. See `.issues/016_attendee_google_auth.md`. |
| ~~CSP blocks registration redirect~~ (✅ Fixed) | ~~**High**~~ | ~~`js_sys::eval()` blocked by CSP `script-src`. Fixed by replacing all eval calls with `wasm_bindgen` JS module imports (`navigation.js`).~~ |

---

## 13. Behavioral Economics UX Patterns

The deposit page and events form incorporate behavioral science patterns to increase attendance and deposit uptake.

### 13A. Loss Aversion — Format Selection Nudge (Events Page)

**Location:** `events_page.rs` — Event Format selector section

When an organizer selects "Online" format, a yellow hint appears:

> "Online events without deposits often see 30-40% no-shows. Consider Hybrid format — in-person attendees commit with deposits and get 100% back just by showing up."

**Pattern:** Loss aversion (Kahneman & Tversky) — framing the cost of NOT having deposits as a loss.

> ⚠️ **Migration note:** Previously this was a deposit toggle nudge. Now replaced by format selection context.

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

The entire deposit architecture acts as a commitment device (in-person and hybrid events only):

1. Attendee deposits USDC → psychological commitment to attend
2. NFT badge is the reward for showing up (commitment + endowment)
3. Full refund on attendance = zero cost for following through
4. Loss of deposit = cost of not following through

The NFT section on the events page is labeled **"Recommended"** (green badge) because NFTs complete the commitment device loop: deposit → show up → get NFT + refund.

> **Online events** use a different motivation: quest-based engagement (quiz/adventure completion) gates the NFT. This creates commitment through effort rather than financial stake.

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
| Format selection nudge | Events form | Yellow hint when Online selected | Loss aversion |
| Refund CTA | Deposit page | "Don't lose your X USDC" | Loss aversion |
| Spot reserved | Deposit page | "Your USDC is secured" | Endowment effect |
| NFT = Recommended | Events form | Green badge on NFT section | Commitment device |
| Refund deadline | Deposit page | Countdown + urgency framing | Scarcity/urgency |
| Refund recovered | Deposit page | "🎉 Refund Recovered!" | Positive reinforcement |
| Quest-based engagement | Online claim flow | Quiz/adventure gates NFT | Effort justification |

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

---

## 16. Event Format Model

### Overview

Every event has a required `event_format` field that determines which features are available. This replaces the previous deposit toggle — deposit availability is now a **consequence** of format choice, not an independent setting.

### The Three Formats

#### 📍 In-Person

A physical event where attendees show up at a venue. Deposit is **required** to incentivize attendance and reduce no-shows.

- **Deposit:** Required (not optional, auto-enabled)
- **Escrow:** Required (on-chain PDA for holding deposits)
- **Check-in:** Physical QR scan by staff
- **NFT Claim:** After check-in (staff scans attendee QR)
- **Refund:** Full refund after check-in
- **Walk-in:** Supported (staff registers via scanner)

#### 💻 Online

A virtual event with no physical venue. No deposit, no escrow, no physical check-in. NFTs are earned through engagement (quiz/adventure completion).

- **Deposit:** Not applicable
- **Escrow:** Not applicable
- **Check-in:** Virtual (quest completion = check-in)
- **NFT Claim:** After quiz/adventure completion
- **Refund:** Not applicable (no deposit)
- **Walk-in:** Not applicable (self-registration instead)

#### 🔄 Hybrid

Combines both in-person and online tracks in one event. One Google Sheet with a `participation_type` column differentiates attendee types.

- **Deposit:** Required for in-person attendees only
- **Escrow:** Required (for in-person deposits)
- **Check-in:** Physical scan for in-person; quest completion for online
- **NFT Claim:** Both paths active — check-in OR quest completion
- **Refund:** In-person attendees only
- **Walk-in:** Supported for in-person track

### Format → Feature Matrix

| Feature | 📍 In-Person | 💻 Online | 🔄 Hybrid |
|---------|-------------|-----------|-----------|
| Deposit required | ✅ Yes | ❌ No | ✅ In-person only |
| Escrow setup | ✅ Required | ❌ N/A | ✅ Required |
| Physical check-in | ✅ QR scan | ❌ No | ✅ In-person track |
| Virtual check-in (quest) | ❌ No | ✅ Quiz/Adventure | ✅ Online track |
| NFT claim | ✅ After check-in | ✅ After quest | ✅ Both paths |
| Refund available | ✅ After check-in | ❌ No | ✅ In-person only |
| Walk-in registration | ✅ Yes | ❌ No | ✅ In-person track |
| Quiz/Adventure config | ❌ Optional | ✅ Recommended | ✅ Recommended |
| Google Sheet `participation_type` | ❌ Not needed | ❌ Not needed | ✅ Required |
| Self-registration (`/e/{slug}`) | ❌ Reserved via deposit | ✅ Yes | ✅ Online track |

### Self-Registration Flow (for Online/Hybrid)

Attendees discover the event via public page `/e/{slug}` and can register directly:

```
Attendee visits /e/{event_slug}
  → GET /api/public/event/{slug} → event details
  → Event format = online or hybrid → shows "Reserve Spot" / "Register" button
  → Attendee fills: name*, email* (optional: wallet address for direct NFT)
  → POST /api/public/register
    → Creates KV record: online_reg:{event_id}:{email_lower}
    → Generates UUID v7 claim token
    → Creates reverse mapping: claim_online:{token} → {event_id}:{email_lower}
    → Returns claim token + claim URL
  → Attendee receives claim URL (shown on page + email if configured)
  → At event time: attendee completes quiz/adventure → NFT minted
```

**KV Key Patterns for Self-Registration:**

| Purpose | Key | Value | TTL |
|---------|-----|-------|-----|
| Online registration | `online_reg:{event_id}:{email_lower}` | `OnlineRegistration` JSON | 90 days |
| Claim token reverse | `claim_online:{claim_token}` | `{event_id}:{email_lower}` | 90 days |
| Quest progress | `quest:{event_id}:{email_lower}` | Quest state JSON | 90 days |

---

## 17. Attendee Journey by Format

### 17A. In-Person Attendee Journey

```
Discovery
  → Organizer shares event link or event page /e/{slug}
  → Attendee sees event details + deposit requirement

Registration
  → Attendee clicks "Reserve Spot" → backend creates attendee record
  → Auto-redirect to deposit page (no manual button click)
  → Resume: if attendee returns to /e/{slug}, localStorage redirects to deposit or ticket page

Deposit (required)
  → THB: upload PromptPay slip → auto-redirect to /ticket/{attendee_id}?event_id={id}
    → Shows QR code + pending approval status
  → USDC (dev_mode only): Connect wallet → USDC deposit → on-chain TX
  → Deposit confirmed: "🎫 Spot Reserved!"

Event Day — Check-in
  → Attendee arrives at venue
  → Staff scans attendee's QR code (claim token)
  → Backend verifies deposit → marks checked-in
  → NFT badge minted automatically (or attendee claims)

Refund
  → After check-in, refund becomes available
  → "💸 Don't lose your {X} USDC — claim it now"
  → Connect wallet → refund TX → USDC returned
  → "🎉 Refund Recovered!"

Result: Attendee showed up → got NFT badge + full refund. Net cost: $0.
```

### 17B. Online Attendee Journey

```
Discovery
  → Attendee finds event via /e/{slug} or shared link
  → No deposit required — sees "Register" / "Reserve Spot" button

Self-Registration
  → Fills name + email → POST /api/public/register
  → Receives claim URL: /claim/{token}
  → Bookmark or save link

Event Time — Quest
  → Attendee opens claim URL at event time
  → Quiz/Adventure loads (if configured)
  → Completes quest → "virtual check-in" recorded
  → Backend creates quest completion record in KV

NFT Claim
  → Quest completion triggers NFT mint
  → Attendee connects wallet → NFT minted to their address
  → "🎉 Badge earned!"
  → Share CTA (future improvement)

Result: Attendee engaged → completed quest → earned NFT badge. No money exchanged.
```

### 17C. Hybrid Attendee Journey

Both tracks run in parallel within the same event:

```
In-Person Track:
  → Same as 17A (deposit → check-in → NFT → refund)
  → participation_type = "in_person" in Google Sheet

Online Track:
  → Same as 17B (register → quest → NFT)
  → participation_type = "online" in Google Sheet

One event, one Google Sheet, two experiences.
Staff scanner shows which track the attendee is on.
```

---

## 18. Online Attendee NFT Claim Flow

### Overview

Online attendees do not deposit funds, do not have escrow, and are not physically checked in. Instead, they earn NFT badges through **quest-based engagement** — completing a quiz or adventure serves as a "virtual check-in."

### Claim Token Sources

Online attendees receive claim tokens through one of two paths:

1. **Self-Registration:** Attendee registers via `/e/{slug}` → `POST /api/public/register` → claim token generated → stored in KV
2. **Auto-Generated:** Organizer pre-populates Google Sheet with online attendee emails → Sheet sync generates claim tokens → tokens shared via email/message

### Quest Completion = Virtual Check-in

```
Attendee opens /claim/{token}
  → GET /api/claim/status/{token} → returns attendee info + event details + quest config
  → If quest not started: loads quest UI (quiz questions / adventure steps)
  → Attendee completes quest
    → POST /api/claim/quest/complete → validates answers/steps
    → Creates KV record: quest_complete:{event_id}:{email_lower} → timestamp
    → Marks "virtual check-in" in attendee record

NFT Mint
  → After quest completion, NFT claim unlocked
  → Attendee connects wallet
  → POST /api/claim/mint → mints NFT to attendee wallet
  → KV updated: claim_minted:{event_id}:{email_lower} → {mint_address, timestamp}
  → Attendee sees: "🎉 Badge earned!"
```

### Quiz/Adventure Gates

| Gate Type | Config | Behavior |
|-----------|--------|----------|
| Quiz | Array of questions + answers | Attendee must answer X% correctly to pass |
| Adventure | Sequence of actions/steps | Attendee must complete all steps in order |
| None | No quest configured | NFT claim is ungated — attendee can mint immediately |

### What's NOT Involved for Online Attendees

| Concept | Status |
|---------|--------|
| Deposit | ❌ Not collected |
| Escrow PDA | ❌ Not created |
| Physical check-in | ❌ Not needed (quest = virtual check-in) |
| Refund | ❌ Not applicable (no deposit to refund) |
| Wallet requirement | Only for NFT mint (not for registration or quest) |

### Online Attendee KV Key Patterns

| Purpose | Key | Value | TTL |
|---------|-----|-------|-----|
| Registration record | `online_reg:{event_id}:{email_lower}` | `OnlineRegistration` JSON | 90 days |
| Claim token reverse | `claim_online:{claim_token}` | `{event_id}:{email_lower}` | 90 days |
| Quest progress | `quest:{event_id}:{email_lower}` | `{status, current_step, answers}` | 90 days |
| Quest completion | `quest_complete:{event_id}:{email_lower}` | `{completed_at}` timestamp | 90 days |
| NFT minted | `claim_minted:{event_id}:{email_lower}` | `{mint_address, timestamp}` | Permanent |

### Error Paths

| Scenario | Behavior |
|----------|----------|
| Invalid/expired claim token | "Invalid claim link" — token not found in KV or TTL expired |
| Quest not yet available | "Event hasn't started yet" — event_start not reached |
| Quest already completed | Skip quest, go directly to NFT claim |
| NFT already minted | Show "Badge already claimed" with link to Solscan |
| Wallet not connected at mint step | Prompt wallet connection, then retry |
| Mint TX fails | "Minting failed, please retry" — claim_minted not set, can retry |

---

## 19. Registration Capacity & Track Gating (Issue 024)

### Overview

Events can have configurable capacity limits per track, with intelligent gating that controls when each track becomes available for registration. This prevents NFT exhaustion and ensures fair spot allocation.

### Capacity Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `in_person_capacity` | `Option<u32>` | Required | Max in-person spots (None = unlimited) |
| `online_capacity` | `Option<u32>` | None (unlimited) | Max online spots — prevents NFT exhaustion |
| `online_open_mode` | `OnlineOpenMode` | `Always` | How/when online track opens |
| `online_registration_open` | `bool` | `false` | Manual toggle (for Manual mode) |
| `deposit_deadline_hours` | `Option<u32>` | `None` | Hours to complete deposit before auto-switch |

### Capacity Counting

In-person capacity counts **all registered in-person attendees** regardless of deposit status:

| State | Description | Counts toward capacity? |
|-------|-------------|------------------------|
| Registered | Just registered, no deposit yet | ✅ Yes — holds the spot |
| Pending | Deposit slip uploaded, awaiting verification | ✅ Yes — holds the spot |
| Deposited | Deposit verified (USDC on-chain or THB admin-verified) | ✅ Yes — confirmed spot |

### Deposit Deadline + Auto-Switch to Online

```
Register as In-Person
  ├─ Deposit within deadline → spot confirmed ✅
  └─ Deadline passes → auto-switch to Online track ♻️
      → participation_type changed to "Online" in sheet + KV
      → in-person spot released
      → attendee gets online claim path (after event end)
```

Rationale: Instead of cancelling the registration entirely (loses the attendee), auto-switch to online. They keep their registration but free up the physical spot.

### Online Registration Gating

#### `Always` Mode
Both tracks open from registration start. User picks their preferred track. In-person disappears when full.

#### `AutoOnFull` Mode
Online track is hidden until in-person capacity is reached. Once in-person is full, online automatically becomes available. This creates a natural funnel: physical spots first, then virtual.

#### `Manual` Mode
Organizer controls when online opens via a toggle in staff UI. Useful for events where online should open at a specific moment (e.g., livestream goes live).

### Registration UX

#### Tracks Available (Both Open)
```
┌──────────────────────────────────┐
│  Choose your track:              │
│                                  │
│  ● In-Person  — 12 spots left   │
│  ○ Online  — Unlimited          │
│                                  │
│  [Reserve My Spot]               │
└──────────────────────────────────┘
```

#### In-Person Full (Auto-Switch to Online)
```
┌──────────────────────────────────┐
│  ✅ Online — Unlimited           │
│                                  │
│  In-person spots are all taken.  │
│  You've been registered for the  │
│  online track.                   │
│                                  │
│  [Register for Online]           │
└──────────────────────────────────┘
```

### Claim Timing by Track

| Track | Claim Available | Rationale |
|-------|----------------|----------|
| In-Person | After check-in | Proved physical presence |
| Online | After event end (`now > event_end_ms`) | Prevents gaming before event |

### Walk-in Capacity Handling

- Walk-ins always count against in-person capacity
- If capacity is reached, staff sees a warning: "⚠️ In-person capacity reached (150/150). Register anyway?"
- Staff can override — they physically see the person
- Online-only events: walk-in registration is blocked

### Backend Capacity Check (Pseudocode)

```
fn check_registration_capacity(config, attendees, participation_type):
    if participation_type == "In-Person":
        in_person_count = attendees.filter(|a| a.is_in_person()).count()
        if let Some(cap) = config.in_person_capacity:
            if in_person_count >= cap:
                return Err("In-person spots are full")
    
    if participation_type == "Online":
        online_count = attendees.filter(|a| !a.is_in_person()).count()
        if let Some(cap) = config.online_capacity:
            if online_count >= cap:
                return Err("Online spots are full")
        
        match config.online_open_mode:
            Manual => if !config.online_registration_open:
                return Err("Online registration not yet available")
            AutoOnFull => 
                in_person_count = attendees.filter(|a| a.is_in_person()).count()
                if let Some(cap) = config.in_person_capacity:
                    if in_person_count < cap:
                        return Err("Online opens when in-person is full")
            Always => () // always allowed
    
    Ok(())
```

### Google Sheet Row Deletion Fix

**Problem**: `delete_sheet_row` uses `spreadsheets.values.clear` which empties cells but leaves empty rows. When appending new rows, Google Sheets may fail on sheets with gaps.

**Fix**: Use `spreadsheets.batchUpdate` with `DeleteDimensionRequest` to actually remove the row, then invalidate all caches so subsequent reads get fresh row indices.

```
Before: Row 1 [data], Row 2 [empty], Row 3 [data] → append fails
After:  Row 1 [data], Row 2 [data] → append works
```
