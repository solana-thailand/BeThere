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