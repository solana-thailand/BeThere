# BeThere — IslandDAO V4 Pitch

> Submission framing for IslandDAO V4 Hackathon (Koh Samui).
> Demo Day: Jun 23 · Submission deadline: Jun 22 midnight UTC.

## Build Tracks Targeted

| Track | Priority | Why |
|-------|----------|-----|
| **Payments + Commerce** | PRIMARY | USDC escrow + Solana Pay TX-Request is literally what is built and live |
| **Liquid Staking** | SECONDARY (roadmap) | Escrow float-yield — defensible roadmap angle, not yet built |
| **DeFi + Stablecoins** | SECONDARY | USDC deposit mechanics overlap with Payments framing |

---

## ⚠️ Overclaim Corrections — Paste These Into The Deck

The current deck contains two claims that do not match the code. Below are exact, verified replacements. **These are deadline-critical — fix before submission.**

### 2.1.1 — Refund Trigger

**OVERCLAIM (current, FALSE):**
> "Attendees pass a quick quiz to receive their refund."

**Why it is false:** The quiz is NOT required for the refund. The refund is earned by **checking in**.

**VERIFIED REPLACEMENT:**
> "Attendees who check in get their full USDC deposit back — guaranteed, with no deadline. The quiz unlocks the cNFT attendance badge, not the refund."

**Code reference:** `bethere-escrow/src/instructions/refund.rs` — checked-in attendees can refund anytime after `event_end` (no deadline); no-shows have only a limited window before forfeit. The quiz lives in the worker claim flow, gated separately from the on-chain refund instruction.

---

### 2.1.2 — Forfeit Distribution

**OVERCLAIM (current, FALSE):**
> "No-show deposits are split between the Organizer and BeThere platform."

**Why it is false:** Forfeited deposits go **100% to the Event Organizer**. There is no platform split today.

**VERIFIED REPLACEMENT:**
> "No-show deposits are forfeited to the Event Organizer — 100%, on-chain, instantly claimable after the refund deadline."

**Optional roadmap add (for the Liquid Staking track):**
> "Roadmap: the platform may capture yield on idle escrow capital during the lock period, or apply a configurable protocol fee on forfeits (Phase 6). Neither is live today."

**Code reference:** `bethere-escrow/src/instructions/claim_forfeited.rs` L95-119 — `vault → organizer_ta` transfer, no fee deduction. Platform fee is Phase 6 *planned* (see roadmap in `docs/presentation_materials.md`), not built.

---

## PRIMARY TRACK — Payments + Commerce

BeThere is a **payments-native event platform**: real USDC moves on-chain for every RSVP, check-in, refund, and forfeit.

### The flow in one sentence
Attendees commit USDC via Solana Pay to reserve a spot; those who show up get 100% back on-chain; those who do not forfeit to the organizer.

### What is live today (production-grade, no mocks)
- **USDC deposit escrow** — Solana Pay Transaction Request, client-side wallet signing, no server-side keypair ever touches a transaction
- **On-chain refund** — self-service after check-in, atomic with rent reclamation (SEC-010: introspection-enforced `close_deposit` pairing)
- **On-chain forfeit claim** — organizer-initiated after deadline, 100% to organizer
- **Live aggregate dashboard** — real-time big-screen view of deposits, check-ins, NFT mints (built specifically for this demo)

### Why this fits the track
- **Solana Pay TX-Request** is the deposit primitive — mobile-first, wallet-native, QR-scannable
- **USDC** throughout (6 decimals, stable) — no volatility noise between RSVP and refund
- **Real economic stakes** — deposits, refunds, and forfeits are actual on-chain value transfers, not mocks
- **Sub-cent fees end-to-end** — every flow costs a fraction of a cent

---

## SECONDARY ANGLE — Liquid Staking (Roadmap, Not Built)

Be honest: this is a roadmap angle, not a shipped feature. Present it as such.

### The observation
During the lock period (deposit → refund/forfeit), USDC sits idle in the escrow vault. That is **idle capital on-chain** — a real yield opportunity.

### The roadmap idea
Park the escrow float in a yield-bearing position during lock:
- **Variant A (lower risk):** yield-bearing stable / money-market position
- **Variant B (track-native):** swap float → bSOL during lock, swap back at refund

### Honest caveat
LSTs (bSOL, Bliq) are SOL-denominated; the vault holds **USDC**, so direct LST is not possible without a swap leg. State this as roadmap, not shipped.

---

## Demo Centerpiece — Live Aggregate Dashboard

The **live aggregate dashboard** (`/dashboard/live`) is built specifically for the in-room demo. Project it on the big screen and the room watches deposits land, check-ins light up, and NFTs mint in real time.

- Polls D1 every 2.5s, cache-bypassed
- Five aggregate tiles: registered · checked-in · deposits verified · USDC locked · NFTs minted
- Four-stage funnel with per-stage conversion %
- Live activity feed from the audit log
- Fault-tolerant: a single query blip degrades to zero, never blanks the screen

This is what makes a 40-person live flow **visible to the room** instead of invisible.

---

## Honest Scale & Data Points

> ⚠️ Verify before presenting. These are internal devnet/staging figures from prior handover audits, not independently re-confirmed this session.

- **Attendees processed:** ~69 (devnet staging — confirm in D1 before stating on stage)
- **Events run:** ~5 (devnet — confirm)
- **Latency:** edge-deployed (Cloudflare Workers), sub-500ms check-in
- **On-chain cost:** ~$0.00087 per deposit TX · ~$0.001 per cNFT badge
- **Test coverage:** 250 tests (54 on-chain + 73 domain + 123 worker) + 147 frontend specs + 16 Kani harnesses + 13 E2E
- **Stack:** 100% Rust (worker + escrow program + Leptos WASM frontend)
- **Escrow program (devnet):** `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`

---

## Backup Evidence — Real Devnet Transactions

| Action | TX Signature |
|--------|-------------|
| Init escrow | `2YMQwjLRbTX3TD3uso3wxx6rP8aipJbHpb3A3B3LXtVcTMhnS9n1wv1CWjo8dTcEqNvxaFPW9SQwnLGYKfZ5hJ8G` |
| Deposit | `4cQnNGRa5CHfcuWGzmE2LU7cUj58KreenSswnuypnZyVYTApx2vU7KPqeDYRjNyn8x1WHe6GunnwehS8CUuuoEJe` |
| Refund | `5PA5wPRnHuhqSrvPs8T7t4nS3yjZtoV3sLzJtwYes3zSyW5sKDZ8CXQj1a1ep7gEyvPoV4UsPvgXwd6KEgGtWVHT` |
| Claim forfeited | `2jnDziWsqziCsZ7dQqzh9X3YYVduEtooB3VruJJTyPpkJ3vZJHvZBish3ZaCKYsgtMNXXj96jj6ApEvEvBWrnTYG` |
| Close event | `5WzDH6gRDAkFCq5aBRn3DGdn4ixfWCGg992SAS7ZStjWm5E4Kqq9DuapTig8b1ECnHkE52t2YDgjt8frevPF9BP6` |
| cNFT mint | `4omCGAuSYEj5yCoif3soUGMGy7sZdXvZvXUfL3dUF5goYhhxrzj6LDrndKohEML6zX96fZSkvBTtJ8riwbBFP2Sh` |

View on Solscan (devnet): `https://solscan.io/tx/<SIGNATURE>?cluster=devnet`

---

## One-Liners (Paste-Ready)

- **Pitch hook:** "Skin in the game, on-chain. Show up, get your deposit back. Don't, and it funds the organizer."
- **Refund mechanic:** "Check in, and your USDC is guaranteed back — no deadline. The quiz is for the badge, not the refund."
- **Forfeit mechanic:** "No-shows forfeit to the organizer — 100%, on-chain, after the deadline."
- **Cost:** "Sub-cent fees end-to-end. Real USDC moving for a fraction of what Ethereum would charge."
- **Live demo:** "Watch the room's deposits, check-ins, and NFTs light up in real time on the big screen."