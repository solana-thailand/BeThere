# BeThere Escrow Protocol (Devnet Deployed)

> Technical design document for the deposit-backed attendance system on Solana.
> Status: **Draft** — this is the implementation spec.

---

## 1. Problem Statement

Event no-shows cost organizers real money. Venue booking, food catering, printed materials, and wasted capacity all scale with headcount. The industry standard no-show rate for free RSVPs is **40–60%** — meaning organizers routinely plan for double the actual attendance.

Existing solutions have tradeoffs:

| Approach | Problem |
|---|---|
| Free RSVP | No skin in the game → 40–60% no-show rate |
| Non-refundable deposit | Feels unfair to honest attendees who show up |
| Manual refund | Operational burden, trust required |
| Centralized escrow | Custodial risk, platform could rug pull |

**What we need:** a commitment mechanism that is fair to honest attendees, compensates organizers for genuine no-shows, and requires zero trust in the platform.

---

## 2. Solution: Deposit-Backed Attendance

The core idea is simple:

1. **Attendee deposits USDC** on Solana into an on-chain escrow when RSVPing.
2. **Attendee shows up** → organizer marks them checked in → **full refund**.
3. **Attendee no-shows** → deposit is forfeited → **organizer is compensated** for the wasted cost.

Key properties:

- **On-chain escrow** — trustless, transparent, non-custodial. No platform holds your keys.
- **Deposit amount is immutable** — set at creation, cannot be changed after the first deposit.
- **NFT badge** is minted as proof of attendance (separate from the deposit/refund flow).
- **Time-based refund eligibility** — after the event ends, attendees can always self-refund regardless of check-in status. This is the critical anti-rug-pull mechanism.

### Event Format Determines Escrow Behavior

Events now have a **format** that determines whether the escrow protocol applies:

| Event Format | Deposit Required | Escrow | Check-in | Refund |
|---|---|---|---|---|
| **In-Person** | ✅ Yes (mandatory) | ✅ Yes | QR scan by staff | ✅ Yes (refund+close in 1 TX) |
| **Hybrid** | ✅ Yes (in-person track only) | ✅ Yes | QR scan for in-person attendees | ✅ Yes (refund+close in 1 TX) |
| **Online** | ❌ No | ❌ No | Quest completion = virtual check-in | N/A |

- **In-person and hybrid events** use the full deposit-backed escrow protocol. Deposit is **required**, not optional.
- **Online events** do not use this protocol at all. Attendance verification is handled via quest/quiz completion (see [Online Attendee Flow](#online-attendee-flow)).
- **Hybrid events** have two tracks: in-person attendees deposit and check in physically; online attendees follow the quest-based flow with no deposit.

---

## 3. Win-Win-Win Model

### Attendee Wins

- **Shows up → full refund.** Zero cost to attend if you honor your commitment.
- **NFT badge** as portable, verifiable proof of attendance.
- **Protected from rug pulls** via time-based refund eligibility. After the event ends, you can always get your money back.
- **Transparent.** Verify escrow state on Solscan at any time. No "trust us" required.
- **Online attendees** participate without financial commitment — no deposit, no risk.

### Organizer Wins

- **Genuine no-shows forfeit deposits**, covering wasted venue and food costs.
- **Higher attendance rate.** A deposit — even fully refundable — creates skin in the game.
- **Reputation builds through transparency.** Honest organizers benefit from verifiable escrow.
- **Cannot steal from attendees who showed up.** The protocol's time-based refund mechanism makes this impossible.
- **Hybrid events** let organizers run physical + virtual tracks from one event page.

### Platform Wins

- **Trust is the product.** Every rug pull kills adoption. The protocol is designed to make rug pulls impossible.
- **Sustainable revenue** via optional protocol fee on forfeited deposits only (e.g., 2–5%).
- **Not a tax on honest attendees.** Only no-shows generate platform revenue.
- **Network effects.** More trust → more events → more attendees → more trust.
- **Broader reach** with hybrid/online events — not limited to physical venues.

---

## 4. Trust Model

### Trustless (Protocol Guarantees)

These are enforced by on-chain program logic. No trust in any party is required:

- Deposit amount is **immutable** after escrow creation.
- Event end time and refund deadline are **immutable**.
- Organizer **cannot change terms** after the first deposit.
- Platform **never holds private keys**.
- All operations are **verified on-chain** by the Solana runtime.

### Requires Trust (Off-Chain)

These cannot be verified on-chain. Participants accept these risks:

- **Event is real.** The organizer will actually host the event as described.
- **Organizer will show up.** We cannot verify attendance on-chain.
- **Check-in is honest.** The organizer scans people who actually attended.

### Anti-Rug-Pull: Time-Based Refund Eligibility

The key insight: we minimize trust to only "is this event real?" by ensuring attendees can **always** refund after the event ends, regardless of check-in status.

```
Current (VULNERABLE):
  Refund requires checked_in = true (organizer controls this flag)
  → Organizer can refuse all check-ins → claim all deposits as forfeited
  → This is a rug pull vector

Proposed (SAFE):
  After event_end: refund allowed regardless of check_in status
  → Organizer can't rug pull — attendees can always self-refund
  → Organizer still gets forfeited deposits from people who didn't refund by deadline
  → Risk to organizer: if they don't show up, everyone refunds
```

This aligns incentives correctly:

- **Organizer wants people to show up** (so they don't all refund).
- **Attendee wants to show up** (to get checked in and ensure refund before deadline).
- **Organizer who ghosts their own event** gets nothing — everyone refunds.

---

## 5. Protocol Flow

### Phase 1: Event Setup (Organizer)

```
Organizer → Create Event (off-chain KV store)
         → Sets title, description, location, max attendees
         → Chooses event FORMAT: In-Person | Online | Hybrid
         → If In-Person or Hybrid:
             → deposit_amount is REQUIRED (escrow mandatory)
             → Initialize Escrow (on-chain, 1 transaction)
             → Sets: deposit_amount, event_end, refund_deadline, organizer_wallet
             → All fields immutable after creation
             → Escrow PDA: seeds = ["escrow", organizer_pubkey, event_id]
             → Vault (ATA) created for USDC deposits
         → If Online:
             → No escrow, no deposit_amount
             → Quest/quiz gates configured for attendance verification
```

### Phase 2: Deposit (Attendee)

```
Attendee → Visit event page → Connect wallet

For In-Person events:
  Attendee → Sign deposit transaction (USDC → vault)
           → AttendeeDeposit PDA created: seeds = ["deposit", attendee_pubkey, event_id]
           → On-chain validates: escrow is_active, correct deposit amount
           → Attendee's RSVP confirmed on-chain

For Hybrid events:
  In-person track (attendee selects in-person participation):
    Attendee → Sign deposit transaction (USDC → vault)
             → Same flow as in-person events above
  
  Online track (attendee selects online participation):
    No deposit required
    → RSVP confirmed off-chain
    → Quest/quiz assigned for virtual check-in

For Online events:
  No deposit flow. RSVP is off-chain.
  → See Online Attendee Flow section below
```

### Phase 3: Event Day — In-Person Check-in

```
For In-Person and Hybrid events (in-person track):
  Organizer → Scan attendee QR code → Sign mark_checked_in transaction
           → Sets attendee_deposit.checked_in = true
           → Only organizer can sign (has_one constraint on escrow.authority)
           → One transaction per attendee (no batch ops to prevent errors)

For Online track / Online events:
  No physical check-in.
  → Attendance verified via quest completion (see Online Attendee Flow)
```

### Phase 3.5: Online Track — Quest-Based Verification

For **online attendees in hybrid events** and **all attendees in online-only events**, there is no physical check-in. Instead:

```
Online Attendee → Complete quest/quiz/adventure challenge
               → Quest completion = virtual check-in
               → Marks attendance off-chain (no on-chain escrow involved)
               → Unlocks NFT claim gate
```

See [Online Attendee Flow](#online-attendee-flow) for the full flow.

### Phase 4: Refund Window (After Event)

```
After event_end timestamp:
  Checked-in attendees → can refund anytime
  Non-checked-in attendees → can refund anytime (anti-rug-pull)

  Refund flow (combined TX — refund + close_deposit in 1 transaction):
  Attendee → Sign combined transaction:
           1. refund instruction (vault → attendee USDC ATA)
           2. close_deposit instruction (reclaim rent-exempt SOL)
         → On-chain validates: event_end passed, not already refunded
         → USDC transferred back to attendee
         → attendee_deposit.refunded = true
         → AttendeeDeposit PDA closed, rent reclaimed
         → Single TX = simpler UX (attendee signs once, gets USDC + rent back)

  Note: refund and close_deposit can still be called as separate transactions
  for backwards compatibility, but the combined path is preferred for UX.
```

### Phase 5: Forfeiture (After Refund Deadline)

```
After refund_deadline timestamp:
  Organizer → Sign claim_forfeited transaction
           → Receives: total_deposited - total_refunded - total_already_forfeited
           → On-chain validates: refund_deadline passed, correct accounting

  What's forfeited: deposits from people who deposited but neither:
    - got checked in and refunded, nor
    - self-refunded during the refund window
    
  These are genuine no-shows who didn't bother to claim their money back.
```

### Phase 6: Close (Cleanup)

```
After all funds settled (total_deposited == total_refunded + total_forfeited):

  Step 1: Deactivate
    Organizer → Sign deactivate_event transaction
             → Sets is_active = false on-chain
             → Escrow enters Deactivated state

  Step 2: Close
    Organizer → Sign close_event transaction
             → Reclaims rent (SOL used for PDA storage)
             → Escrow PDA closed
             → All remaining AttendeeDeposit PDAs for this event closed
```

**UI Flow:** The admin panel shows Deactivate and Close buttons when an escrow exists. After close, `escrow_address` and `on_chain_event_id` are cleared from the event form.

---

## Online Attendee Flow

Online attendees (in both online-only and hybrid events) follow a different path with no financial escrow:

```
┌─────────────────────────────────────────────────────────────┐
│                  Online Attendee Flow                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. RSVP (off-chain)                                        │
│     → No deposit, no wallet required at RSVP time           │
│     → Attendee selects "Online" participation type          │
│                                                              │
│  2. Quest / Quiz / Adventure (during event window)          │
│     → Attendee completes a challenge gate                   │
│     → Could be: quiz questions, adventure tasks, etc.       │
│     → Quest completion = virtual check-in                   │
│                                                              │
│  3. Attendance Verified                                     │
│     → Marked as attended (off-chain)                        │
│     → Unlocks NFT claim gate                                │
│                                                              │
│  4. NFT Claim                                               │
│     → Attendee connects wallet                              │
│     → Claims attendance NFT (if quest completed)            │
│     → Same NFT as in-person attendees (same collection)     │
│                                                              │
│  ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─  │
│                                                              │
│  What does NOT happen:                                       │
│  ✗ No deposit                                               │
│  ✗ No escrow PDA                                            │
│  ✗ No refund                                                │
│  ✗ No on-chain check-in                                     │
│  ✗ No forfeiture                                            │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Why no deposit for online?**

- Online events have near-zero marginal cost per attendee (no venue, no food).
- The commitment mechanism for online is **quest completion**, not financial skin-in-the-game.
- Quest gates provide engagement value (attendees learn something) rather than financial friction.

**NFT claim via quest gates:**

- The NFT serves as proof of participation, same as in-person events.
- Quest completion ensures attendees actually engaged with the content.
- For hybrid events, both tracks (in-person and online) mint from the same NFT collection, but via different verification paths.

---

## 6. Security Model

### Threat: Malicious Organizer

| Attack Vector | Current Status | Proposed Mitigation |
|---|---|---|
| Refuse check-ins, claim all forfeited | 🔴 POSSIBLE | Auto-refund eligibility after `event_end` — attendees don't need organizer's permission |
| Change deposit amount after deposits | 🟢 Blocked | On-chain immutable after creation |
| Change refund deadline after deposits | 🟢 Blocked | On-chain immutable after creation |
| Change organizer wallet after deposits | 🟡 Possible in KV | Lock escrow-critical fields after `escrow_address` is set |
| Create fake event, collect deposits | 🟡 No verification | Reputation system + event verification (future) |
| Set extremely high deposit amount | 🟡 No cap | Max deposit cap: $1,000 USDC |

### Threat: Malicious Attendee

| Attack Vector | Current Status | Mitigation |
|---|---|---|
| Deposit then dispute chargeback | 🟢 Not possible | USDC is final — no chargeback mechanism |
| Double deposit | 🟢 Blocked | Same PDA cannot be initialized twice (Solana runtime guarantee) |
| Refund twice | 🟢 Blocked | `refunded` flag checked on-chain, PDA closed after refund |
| Fake check-in | 🟢 Blocked | Only organizer authority can sign `mark_checked_in` |
| Deposit after event starts | 🟢 Blocked | `is_active` flag checked, deactivated before event |

### Threat: Malicious Platform

| Attack Vector | Current Status | Mitigation |
|---|---|---|
| Platform steals funds | 🟢 Not possible | Platform never holds private keys — all funds in program-owned PDAs |
| Platform builds malicious transaction | 🟡 Possible | Client-side transaction verification (verify instruction data before signing) |
| Platform censors refunds | 🟡 Possible | Open protocol — attendees can build and submit transactions directly |

---

## 7. Economics

### For Organizer

- **Cost:** ~0.005 SOL per escrow initialization + rent for PDA accounts (~0.002 SOL per AttendeeDeposit).
- **Revenue:** Forfeited deposits from no-shows.
- **Typical scenario:** 100 attendees, 20% no-show rate, $15 deposit → $300 revenue from no-shows.
- **ROI:** Covers venue and food costs for the wasted capacity from no-shows.

### For Attendee

- **Cost:** $0 if they show up (full refund guaranteed by protocol).
- **Risk:** Deposit amount if they no-show (fair penalty for breaking commitment).
- **Benefit:** Guaranteed spot, NFT badge as proof of attendance, trustworthy event experience.
- **Online attendees:** $0 always. No deposit, no risk. Engagement via quests instead.

### For Platform

- **Revenue:** Optional protocol fee on forfeited deposits only (2–5%, configurable).
- **NOT a tax on honest attendees.** Only no-shows pay the fee. People who show up and refund pay nothing.
- **Sustainable:** Scales with event volume.
- **Aligned incentives:** Platform earns more when events succeed (more events = more volume), not when people get scammed.

---

## 8. On-Chain Architecture

### Accounts (PDAs)

```
EventEscrow
├── seeds: ["escrow", organizer_pubkey, event_id]
├── fields:
│   ├── deposit_amount: u64          (USDC amount in lamports, 6 decimals)
│   ├── event_end: i64               (Unix timestamp)
│   ├── refund_deadline: i64         (Unix timestamp, must be > event_end)
│   ├── organizer: Pubkey            (Authority for check-ins and claims)
│   ├── total_deposited: u64         (Running sum of all deposits)
│   ├── total_refunded: u64          (Running sum of all refunds)
│   ├── total_forfeited: u64         (Running sum of all forfeited claims)
│   ├── is_active: bool              (New deposits allowed while true)
│   ├── bump: u8                     (PDA bump seed)
│   └── usdc_mint: Pubkey            (USDC mint address)
├── immutable after creation (except total_* counters and is_active)
└── rent: ~300 bytes

Note: EventEscrow PDA is only created for In-Person and Hybrid events.
      Online-only events have no on-chain escrow accounts.

AttendeeDeposit
├── seeds: ["deposit", attendee_pubkey, event_id]
├── fields:
│   ├── attendee: Pubkey             (Attendee's wallet)
│   ├── event_id: [u8; 32]           (Links to EventEscrow)
│   ├── amount: u64                  (Deposit amount in USDC lamports)
│   ├── checked_in: bool             (Set by organizer during event)
│   ├── refunded: bool               (Set when refund processed)
│   └── bump: u8                     (PDA bump seed)
├── one per attendee per event (in-person depositors only)
└── rent: ~150 bytes

Vault (Token Account)
├── PDA-owned Associated Token Account for USDC
├── Authority: EventEscrow PDA
└── Only escrow PDA can transfer via CPI (Solana Token program enforces)
```

### Program Instructions

| Instruction | Signer | When | Effect |
|---|---|---|---|
| `create_event` | Organizer | Setup | Initialize escrow PDA + vault ATA |
| `deposit` | Attendee | Before event | USDC → vault, create AttendeeDeposit PDA |
| `mark_checked_in` | Organizer | Event day | Set `AttendeeDeposit.checked_in = true` |
| `refund` | Attendee | After `event_end` | Vault → attendee USDC ATA, set `refunded = true` |
| `refund_and_close` | Attendee | After `event_end` | Combined: refund + close_deposit in 1 TX (preferred) |
| `deactivate_event` | Organizer | Before event starts | Set `is_active = false`, stop new deposits |
| `claim_forfeited` | Organizer | After `refund_deadline` | Transfer unclaimed deposits to organizer |
| `close_event` | Organizer | After settlement | Reclaim rent, close all PDAs |
| `close_deposit` | Attendee | After refund or event deactivation | Close `AttendeeDeposit` PDA, reclaim rent-exempt SOL |

### Instruction Ordering Constraints

```
create_event → deposit (N times) → deactivate_event
                                   ↓
                              mark_checked_in (N times)
                                   ↓
                              refund | refund_and_close (N times, after event_end)
                                   ↓
                              claim_forfeited (after refund_deadline)
                                   ↓
                              close_event (after full settlement)
```

---

## 9. Proposed Changes (from Security Audit)

### P0: Time-Based Refund Eligibility (On-Chain)

**Priority:** Critical. This is the anti-rug-pull mechanism.

**Change:** Remove `checked_in` hard requirement for refunds after `event_end`.

**Logic:**
```rust
// Before (VULNERABLE):
if !deposit.checked_in {
    return Err(ErrorCode::NotCheckedIn);
}

// After (SAFE):
// checked_in is no longer required after event_end
// The only requirements are: event_end passed, not already refunded
```

**Impact:** Organizer cannot rug pull by refusing check-ins. Attendees can always self-refund after the event. Organizer still receives forfeited deposits from people who didn't refund by the deadline.

### P1: Lock Escrow-Critical Fields (Backend)

**Priority:** High. Prevents organizer from changing terms after deposits start.

**Change:** After `escrow_address` is set in the off-chain KV store, reject changes to:

- `organizer_wallet`
- `on_chain_event_id`
- `deposit_amount_usdc`
- `refund_deadline_hours`

**Impact:** Prevents a mismatch between off-chain event data and on-chain escrow parameters.

### P2: Maximum Deposit Cap (Backend)

**Priority:** Medium. Prevents abuse via unreasonably high deposits.

**Change:** Cap deposit at $1,000 USDC (1_000_000_000 lamports with 6 decimal places).

**Enforcement:** Reject in both `create_event` and `update_event` API endpoints.

**Impact:** Limits downside risk for attendees. $1,000 is more than enough for any reasonable event deposit.

### P3: Archive Guards (Backend)

**Priority:** Medium. Prevents data inconsistency between on-chain and off-chain state.

**Change:** Cannot archive an event if:
- Escrow exists and `is_active = true` on-chain.
- Any `AttendeeDeposit` PDAs exist that haven't been refunded or forfeited.

**Enforcement:** API endpoint checks on-chain state before allowing archive.

**Impact:** Prevents organizers from hiding events while escrow is still active. Ensures all deposits are settled before cleanup.

### P4: Platform Fee (Future)

**Priority:** Low. Revenue feature, not a security fix.

**Change:** Optional protocol fee on forfeited deposits.

**Design:**
- Configurable percentage (default: 0%).
- Collected into a separate platform vault PDA.
- Claimed by platform authority periodically.
- Split occurs during `claim_forfeited` instruction.

**Example:**
```
Forfeited deposit: $15.00
Platform fee (3%): $0.45
Organizer receives: $14.55
```

**Impact:** Sustainable platform revenue without taxing honest attendees. Only no-shows who forfeit deposits generate fees.

---

## 10. Cluster Configuration

| Cluster | USDC Mint | Explorer Base URL |
|---------|-----------|-------------------|
| `devnet` | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` | `https://explorer.solana.com?cluster=devnet` |
| `mainnet-beta` | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m` | `https://explorer.solana.com` |

All explorer links (transaction signatures, account addresses) must be cluster-aware. Read from `SOLANA_CLUSTER` environment variable to determine which cluster and mint to use.

### Configuration in Code

```rust
pub fn get_usdc_mint(cluster: &str) -> Pubkey {
    match cluster {
        "devnet" => "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
            .parse()
            .unwrap(),
        "mainnet-beta" => "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m"
            .parse()
            .unwrap(),
        _ => panic!("Unsupported cluster: {cluster}"),
    }
}
```

---

## Behavioral Economics Foundation

The escrow deposit system is grounded in established behavioral economics principles:

### Loss Aversion (Kahneman & Tversky, 1979)

The pain of losing is ~2× as powerful as the pleasure of gaining. Deposits leverage this:
- **Deposit = skin in the game** — attendees feel the "loss" of their committed USDC if they no-show
- **Full refund on attendance** — the deposit isn't a fee, it's a commitment device
- **Forfeiture = real loss** — no-shows permanently lose their deposit to the organizer

### Endowment Effect

Once attendees deposit, they "own" their reserved spot and value it more. The confirmation page reinforces ownership with language like "Your spot is reserved" rather than "Payment received."

### Refund Deadline (Scarcity + Urgency)

The `refund_deadline_hours` field creates a time-bounded refund window after the event. This serves dual purposes:
- **Anti-rug-pull**: Organizers can't withhold refunds indefinitely
- **Loss urgency**: After the deadline, unclaimed deposits are forfeited — framing inaction as a loss motivates timely refund claims

### Commitment Device

The deposit acts as a pre-commitment mechanism. Research shows that people who pre-commit to an action are significantly more likely to follow through. The deposit + NFT claim flow creates a two-stage commitment:
1. **Financial commitment** (deposit) — before the event (in-person/hybrid only)
2. **Identity commitment** (NFT claim) — after the event (all formats)

### Organizer Nudge (Loss Framing)

When creating events, organizers who haven't enabled deposits see: "Events without deposits often see 30-40% no-shows." This frames the decision as avoiding a loss rather than adding a feature.

---

## 11. Open Questions

1. **Refund deadline duration.** What's the default window between `event_end` and `refund_deadline`? Suggestion: 48 hours. Long enough for attendees to claim, short enough that organizers get paid quickly.

2. **Check-in method.** QR code scanning requires the organizer to have a working device and internet at the venue. What's the fallback if the organizer's device fails? Manual batch check-in post-event?

3. **Partial refunds.** Should the protocol support partial refunds (e.g., attendee shows up late, gets 50% refund)? Suggestion: no. Keep it binary — show up and refund, or don't and forfeit. Simplicity is a feature.

4. **Multi-organizer events.** Should multiple wallets be able to mark check-ins? Useful for large events with multiple entry points. Suggestion: future feature, not V1.

5. ~~**Deposit currency.** USDC only for V1, or should we support SOL and other SPL tokens? Suggestion: USDC only. Price stability matters for deposits.~~
   **✅ Resolved:** Implemented as USDC (on-chain) + THB/PromptPay (off-chain) dual-track in V1.

6. **Event cancellation.** What happens if the organizer cancels the event before `event_end`? All deposits should be immediately refundable. Needs a `cancel_event` instruction.

7. **Online attendees deposit in hybrid events.** Should online attendees in hybrid events also be able to optionally deposit? This would create a financial commitment for online participation (e.g., to reduce virtual no-shows for workshops with limited virtual seats). **Status:** Not in V1. Consider as Future Option B if organizers request it.

---

## 12. Implementation Checklist

### On-Chain Program

- [x] On-chain program: `create_event` instruction
- [x] On-chain program: `deposit` instruction
- [x] On-chain program: `mark_checked_in` instruction
- [x] On-chain program: `refund` instruction (P0: remove `checked_in` requirement after `event_end`)
- [x] On-chain program: `deactivate_event` instruction
- [x] On-chain program: `claim_forfeited` instruction
- [x] On-chain program: `close_event` instruction
- [x] On-chain program: `close_deposit` instruction (rent reclamation)
- [x] On-chain program: 26 SVM unit tests (quasar-svm)
- [x] On-chain program: 13 Kani formal verification harnesses
- [ ] On-chain program: `refund_and_close` combined instruction (refund + close_deposit in 1 TX)

### Backend

- [x] Backend: lock escrow-critical fields after `escrow_address` set (P1)
- [x] Backend: enforce max deposit cap $1,000 USDC (P2)
- [x] Backend: archive guards — check on-chain state before allowing archive (P3)
- [x] Backend: cluster-aware explorer link generation
- [ ] Backend: event format field on event model (`InPerson` | `Online` | `Hybrid`)
- [ ] Backend: auto-enable deposit requirement for in-person/hybrid event formats
- [ ] Backend: self-registration API for public event page (attendee signs up without organizer invite)
- [ ] Backend: online attendee claim path (quest-based attendance verification)
- [ ] Backend: participation type field for hybrid events (`in_person` | `online`)

### Frontend

- [x] Frontend: deposit flow (connect wallet → sign TX → confirm)
- [x] Frontend: refund flow (after event → sign TX → confirm)
- [x] Frontend: check-in flow (organizer QR scan → sign TX)
- [x] Frontend: escrow state display (Solscan links)
- [ ] Frontend: format selector on event creation form (In-Person / Online / Hybrid)
- [ ] Frontend: hide deposit settings for online-only events
- [ ] Frontend: participation type selector for hybrid events (in-person with deposit / online no deposit)
- [ ] Frontend: online attendee quest/quiz UI for virtual check-in
- [ ] Frontend: combined refund+close TX (single "Claim Refund" button, 1 signature)

### Testing & Security

- [x] Integration tests (devnet with real USDC faucet)
- [x] Security review of on-chain program (11 findings, all fixed)
- [ ] Load testing: 100+ concurrent deposits
- [ ] External security audit submission (Audit Arena)
- [ ] End-to-end test: hybrid event (in-person deposit + online quest paths)
