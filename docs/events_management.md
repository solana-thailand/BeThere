# Events Management — Reference Document

> Comprehensive reference for the Events Management domain of the BeThere platform.
> Covers data models, API endpoints, lifecycle, roles, frontend, validation, and related files.

---

## 1. Overview

BeThere is a multi-event platform. **Events are the core entity** — everything (attendees, deposits, NFTs, check-ins) belongs to an event. Events are managed by organizers via the admin UI and stored in **Cloudflare KV**.

Each event encapsulates:

- Identity and schedule information
- Google Sheets integration for attendee data
- Optional quiz/adventure gates
- Optional deposit/escrow configuration (on-chain)
- NFT badge minting configuration
- Access control lists (organizers, staff)

---

## 2. Data Model

### EventStatus Enum

Four states govern the event lifecycle:

| Status | Description |
|--------|-------------|
| `Draft` | Being configured, not visible to attendees |
| `Active` | Live — attendees can check in and claim |
| `Completed` | Ended — attendance frozen, claims still possible |
| `Archived` | Soft-deleted/hidden |

### KV Storage Keys

| Key | Value Type | Purpose |
|-----|-----------|---------|
| `events` | `EventIndex` (list of `EventMeta`) | Top-level event index |
| `event:{id}` | `EventConfig` (full config) | Per-event configuration |
| `event:{id}:quiz:questions` | `QuizConfig` | Per-event quiz |
| `event:{id}:quiz:progress:{token}` | `QuizProgress` | Per-event quiz progress |

### EventMeta

Lightweight struct used in event listings:

| Field | Description |
|-------|-------------|
| `id` | Unique event identifier |
| `name` | Display name |
| `slug` | URL-safe identifier |
| `status` | Current `EventStatus` |
| `event_start_ms` | Start timestamp (epoch ms) |
| `event_end_ms` | End timestamp (epoch ms) |
| `sheet_id` | Google Sheets spreadsheet ID |
| `created_at` | Creation timestamp |
| `organizer_emails` | List of organizer email addresses |
| `deposit_enabled` | Whether deposit/escrow is active |
| `escrow_address` | On-chain escrow account address |

### EventConfig

Full configuration with 40+ fields organized in sections:

**Identity:**

| Field | Description |
|-------|-------------|
| `id` | Unique event identifier |
| `name` | Display name |
| `slug` | URL-safe identifier |
| `tagline` | Short description |
| `link` | External event link |
| `status` | Current `EventStatus` |

**Schedule:**

| Field | Description |
|-------|-------------|
| `event_start_ms` | Start timestamp (epoch ms) |
| `event_end_ms` | End timestamp (epoch ms) |

**Google Sheets:**

| Field | Description |
|-------|-------------|
| `sheet_id` | Spreadsheet ID |
| `sheet_name` | Attendee sheet tab name |
| `staff_sheet_name` | Staff sheet tab name |

**Quiz:**

| Field | Description |
|-------|-------------|
| `quiz_enabled` | Whether quiz gate is active |

**NFT / Claim:**

| Field | Description |
|-------|-------------|
| `nft_collection_mint` | Collection mint address (base58) |
| `nft_metadata_uri` | Metadata JSON URI |
| `nft_image_url` | Badge image URL |
| `nft_name_template` | NFT name pattern |
| `nft_symbol` | NFT token symbol |
| `nft_description_template` | NFT description pattern |
| `merkle_tree` | Merkle tree address for compressed NFTs |
| `claim_base_url` | Base URL for claim pages |

**Access Control:**

| Field | Description |
|-------|-------------|
| `organizer_emails` | Organizer email addresses |
| `staff_emails` | Staff email addresses |

**Deposit:**

| Field | Description |
|-------|-------------|
| `deposit_enabled` | Toggle for deposit feature |
| `deposit_amount_usdc` | USDC amount in lamports (6 decimals) |
| `deposit_amount_thb` | Thai Baht equivalent (display only) |
| `promptpay_id` | PromptPay ID for fiat deposits |
| `escrow_address` | On-chain escrow PDA |
| `organizer_wallet` | Organizer's Solana wallet |
| `on_chain_event_id` | On-chain event identifier |
| `refund_deadline_hours` | Hours after event for refund eligibility |

**R2 Storage:**

| Field | Description |
|-------|-------------|
| `ASSETS_BUCKET` | R2 bucket binding for slip images, refund proofs, badge SVGs |

**Rollover & Credit:**

| Field | Description |
|-------|-------------|
| `deposit_hold` | Deposit held as rolling credit for next event (attendee choice) |
| `rollover_target_event` | Target event for atomic deposit rollover |

**Timestamps:**

| Field | Description |
|-------|-------------|
| `created_at` | Creation timestamp |
| `updated_at` | Last update timestamp |

---

## 3. API Endpoints

### Core CRUD Endpoints

All protected (require admin auth):

| Method | Route | Purpose | Auth |
|--------|-------|---------|------|
| `GET` | `/api/events` | List events visible to current user | Admin/Organizer |
| `POST` | `/api/events` | Create a new event | Admin/Organizer |
| `POST` | `/api/events/seed` | Seed first event from env vars | SuperAdmin only |
| `POST` | `/api/events/migrate` | Migrate quiz data from QUIZ to EVENTS namespace | SuperAdmin only |
| `GET` | `/api/events/{id}` | Get event details | Admin/Organizer |
| `PUT` | `/api/events/{id}` | Update event config | Admin/Organizer |
| `DELETE` | `/api/events/{id}` | Archive (soft-delete) event | Admin only |
| `DELETE` | `/api/events/{id}/delete` | Hard delete — permanently remove archived event, frees slug | SuperAdmin only |
| `DELETE` | `/api/events/{id}/delete?force=true` | Force delete — Draft/Archived, bypasses SEC-004 escrow guard | SuperAdmin only (devnet cleanup) |

### Event-Scoped Endpoints

| Method | Route | Purpose |
|--------|-------|---------|
| `POST` | `/api/events/{id}/init-escrow-tx` | Build on-chain escrow init transaction |
| `POST` | `/api/events/{id}/escrow-info` | Get on-chain escrow state |
| `POST` | `/api/escrow/deactivate-event` | Deactivate escrow (sets `is_active=false` on-chain) |
| `POST` | `/api/escrow/close-event` | Close escrow (reclaim rent SOL) |
| `GET` | `/api/metadata/{event_id}` | Dynamic Metaplex metadata JSON |
| `POST` | `/api/walkin/register` | Register walk-in attendee (staff-only) |
| `POST` | `/api/escrow/confirm-init` | Confirm escrow init after wallet signature |
| `POST` | `/api/deposit/hold` | Hold deposit as rolling credit (attendee-authed) |
| `GET` | `/api/deposit/credit-balance` | Check deposit credit balance (attendee-authed) |
| `POST` | `/api/escrow/rollover-deposit` | Build atomic rollover deposit TX (attendee-authed) |
| `GET` | `/api/escrow/health` | Escrow health check |
| `GET` | `/api/refund/refunded` | List already-refunded attendees |
| `POST` | `/api/refund/manual/{attendee_id}` | Mark manual refund |
| `PATCH` | `/api/admin/quiz/questions/{id}/toggle` | Toggle quiz question active/inactive |
| `POST` | `/api/admin/quiz/questions` | Add individual quiz question |
| `PUT` | `/api/admin/quiz/questions/{id}` | Update individual quiz question |
| `DELETE` | `/api/admin/quiz/questions/{id}` | Delete individual quiz question |
| `GET` | `/api/contacts` | List deduplicated master contacts |
| `GET` | `/api/contacts/events` | List events tab |
| `GET` | `/api/contacts/stats` | Contacts statistics |
| `POST` | `/api/contacts/sync` | Sync contacts to sheet |
| `GET` | `/api/orgs` | List organizations |
| `POST` | `/api/orgs` | Create organization |
| `GET` | `/api/orgs/{id}` | Get org details |
| `PUT` | `/api/orgs/{id}` | Update org |
| `DELETE` | `/api/orgs/{id}` | Delete org |
| `GET` | `/api/storage/slips/{event_id}/{attendee_id}` | Serve slip image from R2 |
| `GET` | `/api/storage/refunds/{event_id}/{attendee_id}` | Serve refund proof from R2 |
| `GET` | `/api/storage/badges/{event_id}` | Serve badge SVG from R2 |

---

## 4. Event Lifecycle

```
Draft → Active → Completed → Archived
  │                                ↑
  └── Can go directly to Active ───┘
```

### On-Chain Escrow Lifecycle

```
create_event → deposit → mark_checked_in → refund/claim_forfeited → deactivate_event → close_event (rent reclaimed)
```

The escrow state machine follows these on-chain transitions:

```
Idle → WalletConnected → Initializing → Done → Deactivating → Deactivated → Closing → Closed
```

### Force Delete Flow

```
Archived → Hard Delete (force=false) — requires escrow closed first, SEC-004 enforced
Draft/Archived → Hard Delete (force=true) — irreversible, frees slug, bypasses escrow guard (SuperAdmin only, devnet cleanup)
```

### State Transitions

| From | To | Trigger |
|------|----|---------|
| `Draft` | `Active` | Organizer activates event |
| `Active` | `Completed` | After `event_end_ms` passes (or manual) |
| `Active`/`Draft` | `Archived` | Soft delete (admin only) |
| `Completed` | `Archived` | After all refunds/claims done |

---

## 5. Roles & Access Control

### Permission Matrix

| Role | Create | Edit Own | Edit All | Archive | Init Escrow | Scanner |
|------|--------|----------|----------|---------|-------------|---------|
| SuperAdmin | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Organizer | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ |
| Staff | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

### Role Determination

Roles are resolved in `worker/src/auth.rs`:

| Condition | Role |
|-----------|------|
| Email in `super_admin_emails` (worker secrets) | SuperAdmin |
| Email in `organizer_emails` (event config) | Organizer (per-event) |
| Email in `staff_emails` (event config) or `STAFF_EMAILS` secret | Staff |

---

## 6. Frontend — Events Page

**File:** `frontend-leptos/src/pages/events_page.rs`

### Views

| View | Description |
|------|-------------|
| **List View** | Shows all events with status badges, search, "Create Event" button |
| **Create/Edit View** | Form with collapsible sections |

### Form Sections

| # | Section | Required | Key Fields |
|---|---------|----------|------------|
| 1 | **Identity** | Yes | Name, Slug (auto-generated, editable), Tagline, Link |
| 2 | **Schedule** | Yes | Event Start, Event End datetime pickers |
| 3 | **Google Sheets** | Yes | Sheet ID, Sheet Name, Staff Sheet Name |
| 4 | **NFT Badges** | Recommended | NFT fields with "Use default badge" auto-fill + live preview |
| 5 | **Deposit & Escrow** | Optional | Deposit toggle (with loss-aversion nudge when OFF), USDC amount, refund deadline, escrow init panel |
| 6 | **Advanced** | Optional | Access control emails, on-chain event ID (locked after escrow init) |

### Visual Indicators on Event List

| Indicator | Condition | Style |
|-----------|-----------|-------|
| Active badge | `status = Active` | Green |
| Draft badge | `status = Draft` | Yellow |
| Completed badge | `status = Completed` | Blue |
| Archived badge | `status = Archived` | Gray |
| ⚠ No Escrow badge | `deposit_enabled = true` AND `escrow_address = ""` | Yellow warning |
| ✅ Escrow badge | `deposit_enabled = true` AND `escrow_address ≠ ""` | Green success |

---

## 7. Optimistic Concurrency

Events use **optimistic concurrency** for concurrent edit protection:

| Step | Behavior |
|------|----------|
| Backend | Checks `expected_updated_at` against stored `updated_at` |
| Conflict | Returns conflict error on mismatch |
| Frontend | Detects `"conflict"` in error message and shows refresh toast |

This prevents silent overwrites when two organizers edit the same event simultaneously.

---

## 8. Validation Rules

### On Save (Create or Update)

| Field | Rule |
|-------|------|
| Name | Required |
| Slug | Required, URL-safe, unique across all events |
| Sheet ID | Required |
| Event Start | Must be before Event End |
| Deposit amount | `0 < amount <= 1,000 USDC` (1,000,000,000 lamports) |
| `nft_collection_mint` | Must be valid base58 |
| NFT URLs | Must be valid URL format |

### Locked Fields (After Escrow Init)

Once `escrow_address` is set, the following fields become **immutable**:

| Field | Reason |
|-------|--------|
| `organizer_wallet` | Escrow funds belong to this wallet |
| `on_chain_event_id` | On-chain identifier is sealed |
| `deposit_amount_usdc` | Deposit amount is committed on-chain |
| `refund_deadline_hours` | Refund policy is committed on-chain |

> **SEC-002:** Backend rejects changes to locked fields when `escrow_address` is set.

### Security Validations (Backend-Only)

| ID | Rule |
|----|------|
| SEC-003 | Max deposit cap $1,000 |
| SEC-004 | Block archive with active escrow |

---

## 9. NFT Badge Configuration

Self-hosted approach — no Arweave/IPFS required:

| Endpoint | Returns | Details |
|----------|---------|---------|
| `GET /api/badge-hd.svg` | SVG image | 1000×1000 SVG, hexagonal shield design |
| `GET /api/metadata/{event_id}` | JSON | Dynamic Metaplex metadata, loads from KV, falls back to global config |

### Configuration Flow

1. Click **"Use default badge"** button in the NFT Badges form section
2. Auto-fills `nft_image_url` and `nft_metadata_uri` with self-hosted endpoints
3. Badge preview renders live in the form
4. Custom URLs (Arweave/IPFS/CDN) are also supported — override the defaults

---

## 10. Walk-in Attendee Integration

Walk-in attendees are registered per-event via the scanner UI:

| Aspect | Detail |
|--------|--------|
| Endpoint | `POST /api/walkin/register` |
| Auth | Staff-only |
| KV Key (attendance) | `walkin:{event_id}:{email}` |
| KV Key (claim) | `claim_walkin:{token}` |

### Walk-in Behavior

- Walk-ins **skip** quiz/adventure gates
- **Auto check-in** at registration time
- Follow the **same deposit/NFT/refund flow** as pre-registered attendees
- Scoped to the event — no cross-event leakage

---

## 11. Related Files

| File | Role |
|------|------|
| `domain/src/models/event.rs` | Data model (`EventMeta`, `EventConfig`, `EventIndex`, `EventStatus`) |
| `worker/src/handlers/events.rs` | API handlers (CRUD) |
| `worker/src/handlers/metadata.rs` | Dynamic NFT metadata endpoint |
| `worker/src/handlers/walkin.rs` | Walk-in registration endpoint |
| `worker/src/claim.rs` | Claim flow (pre-registered + walk-in) |
| `frontend-leptos/src/pages/events_page.rs` | Events management UI (list + create/edit form) |
| `frontend-leptos/src/api.rs` | Frontend API types + functions |
| `bethere-escrow/src/state.rs` | On-chain `EventEscrow` state |
| `bethere-escrow/src/lib.rs` | On-chain program (8 instructions) |
| `worker/src/storage.rs` | R2 storage helpers (slip/refund/badge serving) |
| `worker/src/db.rs` | D1 claim lock operations |
| `worker/src/cleanup.rs` | Cron KV cleanup (retention policy) |
| `worker/src/org_store.rs` | Organization KV store CRUD |
| `worker/src/escrow_indexer/` | On-chain escrow event indexer (webhook + poller + store) |
| `worker/src/middleware/` | Cache, rate-limit, security headers, correlation IDs |
| `worker/src/handlers/contacts.rs` | Master contacts API handlers |
| `worker/src/handlers/orgs.rs` | Organization CRUD API handlers |
| `worker/src/handlers/escrow_index.rs` | On-chain event indexing handlers |
| `worker/src/handlers/user_log.rs` | User sign-in logging to Google Sheets |
| `worker/src/claim/` | Claim logic (lock, mint, orchestrator) |
| `domain/src/models/org.rs` | Organization data model |
| `domain/src/models/deposit.rs` | Deposit data models |

---

## 12. Related Docs

| Document | Content |
|----------|---------|
| `docs/business_flows_event_page.md` | Detailed UI scenarios and edge cases for create/edit |
| `docs/escrow_protocol.md` | On-chain escrow protocol design |
| `docs/security_audit.md` | Security findings and fixes |
| `.issues/008_nft_config_and_production_readiness.md` | NFT configuration guide |
| `.issues/014_walkin_attendee_flow.md` | Walk-in attendee implementation |
| `docs/ux_roadmap.md` | Prioritized UX improvements (public event page, scanner feedback, etc.) |
| `.issues/032_rolling_deposit_credit.md` | Rolling deposit credit + atomic rollover design |
| `.handovers/077_rollover_deposit_e2e.md` | Rollover deposit implementation across all 3 layers |
| `.handovers/080_r2_binding_rate_limiting.md` | R2 storage binding + rate limiting |
