# BeThere — Architecture Discussion

> **Date**: 2025-06-30
> **Participants**: Development team, CTO (Solana Thailand)
> **Status**: Decided — awaiting implementation

## Summary

This document captures the architectural decisions for evolving BeThere from a Google Sheets-only check-in system to a Solana-integrated event platform. The key insight: **NFT is a post-check-in reward (badge), not a pre-event ticket.**

---

## 1. AS-IS (Current System)

### Registration Flow

```
User registers on Luma (free tier)
  → Organizers manually download CSV from Luma
  → Upload CSV to Google Drive
  → Convert to Google Sheet
  → If more users register → download CSV again, re-upload
```

**Pain points:**
- Luma API costs money, so CSV export is manual
- No real-time sync between Luma and Google Sheets
- Organizers must remember to re-download when new registrations come in

### Check-In Flow

```
Staff scans attendee QR code (contains api_id)
  → Worker fetches ALL attendees from Google Sheets API (~200-800ms)
  → Scans in-memory to find matching api_id
  → Checks approval status, participation type, duplicate check-in
  → Writes checked_in_at (col I) + checked_in_by (col J) to Google Sheets (~200-300ms)
  → Total latency: 500ms - 2s
```

**Pain points:**
- Google Sheets API is the bottleneck (~80% of total latency)
- Service account JWT signing adds ~100ms overhead (RSA-SHA256 via SubtleCrypto)
- Rate limited to ~100 requests per 100 seconds
- Full sheet scan on every check-in (no indexing)

### Deposit / Refund Flow

```
Attendee pays 500 THB deposit (via Luma or at door)
  → At check-in, staff manually hands back 500 THB in cash
  → No automation
  → No on-chain record
  → No receipt / proof of refund
```

**Pain points:**
- Manual cash handling is error-prone and slow
- No audit trail
- Staff must carry cash

### NFT / Wallet

```
Nothing exists. Column G (solana_address) exists in the sheet but is empty for all attendees.
```

---

## 2. TO-BE (Proposed System)

### Key Architectural Decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | NFT as ticket or badge? | **Badge (post-check-in)** | Purpose is to REWARD attendance, not gate entry. "BeThere" = proof you showed up. |
| D2 | Wallet required at registration? | **No** | Users are new to Solana. Don't block registration. |
| D3 | Wallet required at check-in? | **No** | Don't block entry. Wallet is optional, for the reward. |
| D4 | 500 THB refund method? | **Hybrid: SOL airdrop + USDC** | SOL for gas (~$1.50), USDC for stable value (~$13). Total ≈ 500 THB. |
| D5 | NFT standard? | **Compressed NFT (cNFT) via Bubblegum** | $0.001/NFT vs $0.02+ for regular. Cost-efficient for 500+ attendees. |
| D6 | Check-in verification? | **Keep Google Sheets (Phase 1), add Solana RPC later** | Don't break what works. Incremental improvement. |
| D7 | Claim flow? | **URL-based, accessed after check-in** | Staff shows URL / attendee opens link. Works on any device. |
| D8 | Non-claimers? | **NFT: claim anytime. Refund: manual fallback.** | Don't force crypto on everyone. Keep cash option. |
| D9 | Data store? | **Google Sheets remains source of truth** | Organizers know Sheets. Don't replace, enhance. |
| D10 | RPC provider? | **Helius / QuickNode free tier** | Free tier sufficient for event-scale usage (~500 check-ins). |
| D11 | Event format? | **In-Person / Online / Hybrid (organizer selects)** | Controls deposit, escrow, check-in, and claim paths. |
| D12 | Online attendee verification? | **Quest completion (quiz/adventure)** | Replaces physical check-in for online track. |
| D13 | Deposit toggle? | **No toggle — auto-enabled for in-person** | Deposit is a protocol requirement, not an option. |
| D14 | Self-registration? | **Yes, from public event page** | Attendees can register and deposit without organizer manually adding them. |

### Why NFT Badge (Not NFT Ticket)?

The original discussion considered minting NFTs **before** the event and using them as tickets (scan NFT → enter). We rejected this for several reasons:

| Concern | NFT Ticket (rejected) | NFT Badge (chosen) |
|---------|----------------------|-------------------|
| Wallet required when? | Before registration or check-in | After check-in (optional) |
| Blocks non-crypto users? | Yes — can't check in without wallet | No — check-in works without wallet |
| Onboarding friction | High — must create wallet before event | Low — "Here's a reward, want it?" |
| NFT purpose | Gate (permission to enter) | Reward (proof you were there) |
| Pre-minting required? | Yes — must mint before event | No — mint on-demand when claimed |
| Cost risk | Must pre-mint for all attendees | Only mint for those who claim |
| UX at door | Slow — wallet + NFT verification | Fast — same as current QR scan |

**The "BeThere" name says it all** — the NFT proves you were physically present. It's a POAP (Proof of Attendance Protocol) on Solana.

### Why Hybrid Refund (SOL + USDC)?

| Refund Method | Value Stability | User Gets Gas? | Complexity |
|---------------|----------------|----------------|------------|
| SOL only | ❌ Volatile (10%+ swings) | ✅ Yes | Low |
| USDC only | ✅ Stable | ❌ No — can't transact | Low |
| **SOL + USDC (chosen)** | ✅ Mostly stable | ✅ Yes | Medium |
| Cash only (current) | ✅ Stable | N/A | Low but manual |

**Hybrid breakdown:**
- ~0.01 SOL ($1.50) — enough for ~100 transactions
- ~$13 USDC — stable value equivalent to remaining 500 THB
- Exchange rate locked at check-in time (not registration time)
- Total cost per attendee: ~$14.50 + ~$0.01 gas = ~$14.51

### Revised Check-In Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: CHECK-IN (same as current, unchanged)                  │
│                                                                 │
│  Staff scans QR code (contains api_id)                          │
│    → Worker verifies in Google Sheets                           │
│    → Marks checked_in_at (col I) + checked_in_by (col J)       │
│    → Generates UUID claim_token, stores in column L             │
│    → Staff screen shows: "✅ Checked in!"                       │
│      + claim URL: bethere.solana-thailand.workers.dev/claim/TOKEN│
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STEP 2: CLAIM (new, AFTER check-in, optional)                  │
│                                                                 │
│  Attendee opens claim URL on their phone                        │
│    → Page shows: "✅ You're checked in! Claim your reward:"    │
│    → Attendee connects wallet (Phantom/Solflare/Backpack)      │
│    → Or enters wallet address manually                          │
│    → System mints cNFT badge to their wallet                    │
│    → System sends: 0.01 SOL (gas) + ~$13 USDC (refund)         │
│    → Saves wallet to column G, marks claimed_at in column M    │
└─────────────────────────────────────────────────────────────────┘
```

### Google Sheet Column Updates

| Column | Index | Field | Status | Notes |
|--------|-------|-------|--------|-------|
| A | 0 | `api_id` | Existing | Unique ID |
| B | 1 | `name` | Existing | First name |
| C | 2 | `last_name` | Existing | Last name |
| D | 3 | `display_name` | Existing | Fallback display |
| E | 4 | `email` | Existing | Attendee email |
| F | 5 | `ticket_name` | Existing | Ticket type |
| G | 6 | `solana_address` | **Updated** | Filled at claim time |
| H | 7 | `approval_status` | Existing | Approval state |
| I | 8 | `checked_in_at` | Existing | Check-in timestamp |
| J | 9 | `checked_in_by` | Existing | Staff email |
| K | 10 | `qr_code_url` | Existing | QR link |
| **L** | **11** | **`claim_token`** | **NEW** | UUID, generated at check-in |
| **M** | **12** | **`claimed_at`** | **NEW** | Timestamp when NFT claimed |
| Y | 24 | `participation_type` | Existing | In-Person / Online |

### System Architecture (TO-BE)

```
┌─────────────┐                            ┌──────────────────┐
│  Luma (free)│── CSV export (manual) ────▶│  Google Sheet    │
│  Registration│                            │  (source of truth│
└─────────────┘                            │  for organizers) │
                                           └────────┬─────────┘
                                                    │
                    ┌───────────────────────────────┘
                    ▼
┌──────────────────────────────────────────────────────────────┐
│  BeThere Worker (Cloudflare Workers — Rust WASM)             │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ Check-In     │  │ Claim Page   │  │ Refund Engine    │   │
│  │ /api/checkin │  │ /claim/TOKEN │  │ (SOL + USDC)     │   │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────────┘   │
│         │                 │                  │               │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌──────▼───────────┐   │
│  │ Google Sheets│  │ NFT Minter   │  │ SOL + USDC       │   │
│  │ read/write   │  │ (Bubblegum)  │  │ transfer         │   │
│  └──────────────┘  └──────┬───────┘  └──────┬───────────┘   │
│                           │                  │               │
│  ┌────────────────────────▼──────────────────▼───────────┐   │
│  │           Solana JSON-RPC (worker::Fetch)              │   │
│  │        Helius / QuickNode / Solana RPC (free tier)     │   │
│  └────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────┐
│                    Solana (mainnet-beta)                      │
│                                                              │
│  ┌─────────────┐  ┌──────────────────────────────────────┐  │
│  │ cNFT        │  │ Refund Transaction                   │  │
│  │ Collection  │  │ 0.01 SOL + ~$13 USDC per attendee    │  │
│  │ (Bubblegum) │  │ Treasury wallet → Attendee wallet    │  │
│  └─────────────┘  └──────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. Implementation Phases

| Phase | Feature | Effort | Dependency | Risk |
|-------|---------|--------|------------|------|
| **1** | Claim token generation (column L + M) | 0.5 day | None | Low |
| **2a** | Claim page (frontend Leptos) | 2 days | Phase 1 | Low |
| **2b** | Wallet connect UI | 1 day | Phase 2a | Low |
| **2c** | cNFT minting on claim | 2 days | Bubblegum program | Medium |
| **3a** | SOL airdrop on claim | 1 day | Treasury wallet with SOL | Medium |
| **3b** | USDC transfer on claim | 1 day | Treasury wallet with USDC | Medium |
| **3c** | On-chain check-in tx (optional) | 1 day | Phase 2c | Low |

**Total estimated effort: 7-10 days**

---

## 4. Open Questions (CTO Decision Needed)

| # | Question | Options | Default |
|---|----------|---------|---------|
| Q1 | Exchange rate lock time? | Registration / Check-in / Fixed rate | **Check-in time** |
| Q2 | Treasury wallet management? | Single keypair / Multi-sig (Squads) | **Single keypair** (MVP) |
| Q3 | NFT artwork source? | Manual upload / AI-generated / Template | **TBD** |
| Q4 | NFT metadata schema? | Custom / Metaplex standard | **Metaplex standard** |
| Q5 | RPC provider? | Helius / QuickNode / Triton / Public | **Helius free tier** |
| Q6 | Refund for non-claimers? | Cash fallback / Bank transfer / Forfeit | **Cash fallback** |
| Q7 | Future event gating? | NFT holder check / Token gate / None | **NFT holder (Phase 4)** |

---

## 5. Performance Comparison

| Metric | Current (Sheets) | Phase 1 (Sheets + claim) | Phase 2 (cNFT mint) | Phase 3 (SOL+USDC) |
|--------|------------------|--------------------------|----------------------|---------------------|
| Check-in latency | 500ms - 2s | 500ms - 2s (unchanged) | 500ms - 2s (unchanged) | 500ms - 2s (unchanged) |
| Claim latency | N/A | N/A | ~2-5s (mint cNFT) | ~3-5s (mint + transfer) |
| Cost per attendee | $0 | $0 | ~$0.001 (cNFT) | ~$0.01 (cNFT + gas) |
| Throughput | ~30 check-ins/min | ~30 check-ins/min | ~10 claims/min | ~10 claims/min |
| Audit trail | Google-owned | Google-owned | **On-chain (cNFT)** | **On-chain (cNFT + tx)** |

---

## 6. Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Attendees don't want crypto | High | Medium | Cash refund fallback always available |
| SOL price crashes before event | Low | Medium | USDC is stable, SOL portion is small |
| RPC provider goes down | Low | High | Fallback to public RPC + Sheets works independently |
| Bubblegum program issue | Low | Medium | Can use regular NFTs as fallback (higher cost) |
| Wallet onboarding too complex | Medium | Medium | Clear step-by-step guide, multiple wallet options |

---

## 7. Outcome

The system evolves from:
- **Purely Web2** (Google Sheets + QR + cash) →
- **Web2 + Web3 hybrid** (Sheets for ops, Solana for rewards)

The key principle: **Web3 is additive, never blocking.** Check-in works without Solana. The NFT and refund are rewards for those who want them.

---

## 8. Evolution: PDA Escrow (Current Architecture)

> **Date**: 2025-05-04
> **Status**: Implemented — devnet validated

The deposit/refund system evolved from the original SOL+USDC airdrop design (Section 2) to a **PDA-based on-chain escrow** model. Key reasons for the change:

| Original Design | PDA Escrow (Current) |
|----------------|---------------------|
| Worker directly transfers USDC from treasury | USDC held in on-chain PDA vault |
| Requires treasury wallet with full funds | Each event has its own escrow PDA |
| Single point of failure (treasury key) | Attendee can self-refund (signs TX) |
| Off-chain tracking only | Full on-chain audit trail |
| Refund requires worker to be online | Refund works even if worker is down |

### On-Chain Escrow Program

- **Framework**: Quasar (not Anchor) — lighter-weight Solana program framework
- **Program ID** (devnet): `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
- **Source**: `bethere-escrow/` directory

### Instructions

| Discriminator | Instruction | Signer | Purpose |
|---------------|-------------|--------|---------|
| 0 | `create_event` | Organizer | Initialize EventEscrow PDA + vault ATA |
| 1 | `deposit` | Attendee | Transfer USDC to escrow vault |
| 2 | `mark_checked_in` | Organizer | Mark attendee as checked in (enables refund) |
| 3 | `refund` | Attendee | Self-refund USDC from vault (after event ends) |

### Dual-Track Deposits

The system supports two deposit methods per event:

| Track | Currency | Flow | Refund |
|-------|----------|------|--------|
| **USDC** (on-chain) | USDC (SPL token) | Solana Pay QR → wallet signs TX → on-chain escrow | Self-serve via `refund` instruction |
| **THB** (off-chain) | Thai Baht | PromptPay QR → attendee uploads payment slip → admin verifies | Admin marks refund as done |

### Deposit Confirmation

USDC deposits use a two-step Solana Pay pattern:
1. `POST /api/deposit/usdc` → Returns Solana Pay URL
2. `GET /api/deposit/usdc/tx?event_id=...&attendee_id=...&wallet=...` → Wallet fetches serialized TX
3. Client signs and submits TX to Solana
4. `POST /api/deposit/usdc/webhook` → Notifies worker of TX signature
5. Worker verifies TX on-chain via `getSignatureStatuses` (with `searchTransactionHistory: true`)

### Validation Rules

- Deposits rejected after `event_end_ms` (server-side check)
- Refunds require `clock > event_end` (on-chain check)
- Refunds require attendee to be `mark_checked_in` (on-chain check)
- Vault ATA must be created before `create_event` (two-step initialization)

### Wallet-Signed Operations (Frontend)

All on-chain operations use a shared wallet adapter JS module (`solana_wallet.js`) supporting Phantom, Backpack, Solflare, and Coinbase via the Wallet Standard API. The pattern:

1. **Detect wallets** → `getDetectedWallets()` checks `window.solana`, `window.backpack`, `window.solflare` + Wallet Standard registry
2. **Connect wallet** → `connectWallet(name)` prompts user, returns base58 public key
3. **Backend builds TX** → Server constructs unsigned serialized transaction
4. **Wallet signs + sends** → `signAndSendTransaction(name, b64_tx)` decodes base64 → wallet signs → broadcasts to Solana

Three flows use this pattern:

| Flow | Where | Steps |
|------|-------|-------|
| **Escrow Init** | Admin Events page | Connect → Create Vault ATA (Step 1) → Initialize Escrow (Step 2) |
| **On-Chain Check-In** | Staff Scanner | After off-chain check-in → Connect → Sign `mark_checked_in` TX |
| **Refund** | Deposit page | Attendee connects own wallet → Sign `refund` TX |

---

## 9. Event Format Model

> **Date**: 2025-07-12
> **Status**: Proposed — awaiting implementation

Every BeThere event has a **format** selected by the organizer at event creation. The format is not cosmetic — it is the top-level control that determines which subsystems are active: deposit, escrow, physical check-in, quest, NFT mint, and refund.

### Format Definitions

| Format | Physical Venue | Online Stream | Deposit (USDC/THB) | Physical Check-In | Quest | NFT Claim | Refund |
|--------|---------------|---------------|---------------------|-------------------|-------|-----------|--------|
| **In-Person** | ✅ | ❌ | ✅ Auto-enabled | ✅ Required | ✅ Optional | ✅ Post-check-in | ✅ After event |
| **Online** | ❌ | ✅ | ❌ Disabled | ❌ N/A | ✅ Required (virtual check-in) | ✅ Post-quest | ❌ N/A |
| **Hybrid** | ✅ | ✅ | ✅ In-person track only | ✅ In-person track | ✅ Both tracks | ✅ Both tracks | ✅ In-person track only |

### How Format Controls the System

The event format is stored as a field on the `EventEscrow` PDA (or the event record in the data store). Every subsystem reads this field to determine its behaviour:

```
Event Format
  ├── Deposit Engine → In-Person/Hybrid: enable deposit flow (USDC escrow + THB off-chain)
  │                   Online: skip deposit entirely
  ├── Check-In Service → In-Person/Hybrid: require QR scan by staff
  │                      Online: skip physical check-in
  ├── Quest Engine → In-Person: optional (bonus engagement)
  │                 Online: REQUIRED (this IS the virtual check-in)
  │                 Hybrid: required for online track, optional for in-person track
  ├── NFT Minter → All formats: mint cNFT badge upon completion criteria met
  └── Refund Engine → In-Person/Hybrid: enable refund after event ends
                     Online: no refund (no deposit collected)
```

### Feature Matrix

| Feature | In-Person | Online | Hybrid (In-Person Track) | Hybrid (Online Track) |
|---------|-----------|--------|--------------------------|-----------------------|
| Deposit required | ✅ | ❌ | ✅ | ❌ |
| Escrow PDA created | ✅ | ❌ | ✅ | ❌ |
| Staff QR check-in | ✅ | ❌ | ✅ | ❌ |
| Quest completion | Optional | ✅ Required | Optional | ✅ Required |
| NFT badge minted | ✅ | ✅ | ✅ | ✅ |
| SOL + USDC refund | ✅ | ❌ | ✅ | ❌ |
| Self-registration | ✅ | ✅ | ✅ | ✅ |
| Claim token issued | At check-in | At quest completion | At check-in | At quest completion |

---

## 10. Attendee Journey by Format

The attendee experience differs significantly depending on the event format. Each format has its own end-to-end flow.

### In-Person Journey

```
Discovery (event page / social link)
  → Self-register on public event page
  → Deposit USDC (Solana Pay) or THB (PromptPay + slip upload)
  → Attend event at physical venue
  → Staff scans QR code → physical check-in confirmed
  → Complete optional quest (engagement activity)
  → Open claim URL → connect wallet → mint cNFT badge
  → After event ends → self-refund from escrow PDA (USDC)
```

**Key characteristics:**
- Deposit is mandatory (protocol requirement, not optional)
- Physical check-in is the gate — must be scanned by staff
- NFT is a reward for showing up
- Refund returns the deposit to the attendee's wallet
- Quests are optional engagement, not required for NFT

### Online Journey

```
Discovery (event page / social link)
  → Self-register on public event page
  → No deposit required
  → Attend online stream / session
  → Complete quest (quiz, adventure, interactive challenge)
  → Quest completion = virtual check-in → claim token generated
  → Open claim URL → connect wallet → mint cNFT badge
  → No refund (nothing was deposited)
```

**Key characteristics:**
- No deposit, no escrow, no financial commitment
- Quest completion replaces physical check-in as the attendance proof
- NFT badge is the same cNFT standard (unified badge collection)
- No refund flow — nothing to return
- Lower friction, higher accessibility

### Hybrid Journey

Hybrid events run **both tracks in parallel**. An attendee is assigned to a track at registration (or self-selects):

```
                    ┌─ In-Person Track → deposit → physical check-in → quest (opt) → NFT → refund
Register ──────────┤
                    └─ Online Track    → no deposit → quest (required) → NFT → done
```

**Key characteristics:**
- Organizer creates one event with `format = Hybrid`
- Attendees choose their track during self-registration
- In-person track attendees follow the full deposit/check-in/refund flow
- Online track attendees follow the quest-based virtual check-in flow
- Both tracks receive the same cNFT badge (provenance differs by claim path)
- The `participation_type` field (column Y in Google Sheets) tracks which track each attendee is on

---

## 11. Online Attendee Architecture

> **Date**: 2025-07-12
> **Status**: Proposed — awaiting implementation

Online attendees do not interact with the escrow, deposit, or physical check-in subsystems. Instead, **quest completion serves as virtual check-in**, producing a claim token that unlocks NFT minting.

### Quest Completion = Virtual Check-In

The quest engine is the online attendee's equivalent of the staff QR scan:

```
In-Person Check-In:  Staff scans QR  → verified attendance → claim token
Online Check-In:     Quest completed → verified attendance → claim token
```

Both paths produce the same output: a `claim_token` (UUID) stored against the attendee record. The claim page is format-agnostic — it only checks that a valid claim token exists.

### KV Storage Pattern for Online Attendees

Online attendees are stored in a lightweight KV store (Cloudflare KV or Workers KV) rather than (or alongside) Google Sheets. This avoids inflating the organizer's sheet with online-only attendees who have no deposit or physical check-in data.

```
Key:   event:{event_id}:online:{attendee_id}
Value: {
         "attendee_id": "uuid",
         "name": "...",
         "email": "...",
         "registered_at": 1720000000,
         "track": "online",
         "quest_id": "quiz-001",
         "quest_completed_at": null,       // null until quest is done
         "claim_token": null,               // null until quest is done
         "claimed_at": null,                // null until NFT is minted
         "wallet_address": null             // null until claim page
       }
```

**Lifecycle transitions:**

1. **Registration** → KV entry created with `quest_completed_at: null`
2. **Quest completion** → `quest_completed_at` + `claim_token` populated
3. **NFT claim** → `wallet_address` + `claimed_at` populated
4. **Done** — no further state changes

### Claim Token Generation

Claim tokens for online attendees are generated by the quest engine rather than the check-in service:

```
// In-person: generated by check-in service after QR scan
POST /api/checkin { attendee_id, event_id }
  → claim_token = Uuid::now_v7()

// Online: generated by quest engine after quest completion
POST /api/quest/complete { attendee_id, event_id, quest_id, answers }
  → verify answers → claim_token = Uuid::now_v7()
```

Both paths use the same `Uuid::now_v7()` format and store the token in the same claim URL pattern:
`bethere.solana-thailand.workers.dev/claim/{TOKEN}`

### No Escrow, No Deposit, No Refund

Online attendees are excluded from the financial subsystem entirely:

| Subsystem | In-Person Attendee | Online Attendee |
|-----------|-------------------|-----------------|
| Escrow PDA | Created per event | Not created for online track |
| Deposit (USDC) | Required | Not collected |
| Deposit (THB) | Required | Not collected |
| `mark_checked_in` instruction | Called by staff | Not applicable |
| `refund` instruction | Available after event | Not applicable |
| NFT mint (cNFT) | Available | Available |
| Quest | Optional | **Required** (virtual check-in) |

This separation keeps the financial flows clean — only attendees who deposited funds can refund them. The NFT badge remains the universal reward across all formats.