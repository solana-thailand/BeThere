# Competitive Analysis: Kickback vs BeThere

> A detailed analysis of the now-defunct Kickback event deposit platform and how BeThere improves upon its model.

**Date:** June 2025
**Author:** BeThere Team
**Status:** Strategic Reference

---

## 1. Executive Summary

Kickback (2016–2022) was an Ethereum-based event commitment platform where attendees staked crypto deposits to reserve event spots. If they showed up, they received a proportional payout from no-shows' forfeited deposits. If they didn't show up, they lost their deposit. The concept proved that "skin in the game" crypto-economics dramatically improves event attendance rates (76% attendance across 23 events in their final season).

However, Kickback shut down in February 2022 due to founding team burnout (co-founders became core ENS DAO contributors), single-maintainer risk, Ethereum gas cost friction, and a lack of sustainable revenue model.

**BeThere** builds on the same core insight — deposit-backed attendance — but addresses every structural weakness that killed Kickback: cheaper/faster blockchain (Solana vs Ethereum), stablecoin deposits (USDC vs volatile ETH/MATIC), anti-rug-pull guarantees (time-based refund), sustainable monetization (protocol fee on forfeited deposits only), and a lean architecture (Cloudflare Workers + Solana programs vs full Node.js backend + The Graph).

---

## 2. Kickback Product Overview

### 2.1 What Was Kickback?

Kickback (originally "Blockparty," rebranded in 2018) was an Ethereum-based event management service that required attendees to stake a crypto deposit when RSVPing. The platform's tagline was: *"Event no shows? No problem. Meet Kickback — an Ethereum-based event management service that delivers higher event participation rates by asking registrants to put some skin in the game."*

### 2.2 Company Structure

| Detail | Value |
|--------|-------|
| **Legal Entity** | "No block no party ltd" (UK limited company) |
| **Founded** | 2016 (as Blockparty) |
| **Rebranded** | 2018 (to Kickback) |
| **Incorporated** | 2018 |
| **Co-founders** | Makoto Inoue, Jeff Lau, 1 other (stepped down 2019) |
| **Incubator** | Status Incubate (small investment) |
| **Shut down** | February 2022 (announced), service wound down gradually |
| **License** | MIT (frontend, subgraph, smart contracts) |
| **Backend** | Closed-source (Node.js + PostgreSQL), planned to open-source |

### 2.3 Blockchain / Network Support

Kickback operated across multiple EVM networks over its lifetime:

| Network | Use Case | Contract Address |
|---------|----------|-----------------|
| **Ethereum Mainnet** | Primary (pre-2020) | `0x3361aa92E426E052141Daf9e41A09d36e994Ba23` |
| **Ropsten/Rinkeby/Kovan** | Testnets | Multiple revisions |
| **xDai (now Gnosis Chain)** | Low-cost alternative (2020+) | `0x05E9AE465727AAa78De8F761E44D78b43a5d9697` |
| **Polygon** | Final network (2021) | `0xc1d24FB1a9c6b5051c28b0e963473D3cE3EB3491` |

The migration from Ethereum mainnet → xDai → Polygon was driven entirely by gas costs. Ethereum mainnet became too expensive for the small deposit amounts the platform needed.

### 2.4 Smart Contract Architecture

Kickback deployed **one smart contract per event** via a factory pattern:

- **Deployer.sol** — Factory contract that creates per-event contracts
- **EthConference.sol** — For ETH-denominated events
- **ERC20Conference.sol** — For ERC20 token-denominated events
- **AbstractConference.sol** — Base contract with all core logic
- **GroupAdmin.sol** — Admin role management
- **kickback-subgraph** — The Graph indexing for event data

### 2.5 Core Flow

1. **Event Creation** — Organizer creates event via `Deployer.deploy()`, setting: name, deposit amount, participant limit, cooling period (default 1 week), token address (ETH or ERC20)
2. **RSVP/Registration** — Participants call `register()` with the correct deposit amount. For ERC20, they must first call `token.approve(deposit)`.
3. **Check-in** — Admins mark attendees as checked in. Done off-chain (manually).
4. **Finalization** — After the event, an admin calls `finalize()` with attendance data in bitmap format (to save gas). This calculates the payout per attendee.
5. **Withdrawal** — Attendees manually call `withdraw()` to claim their payout. The payout is proportional: if 10 people RSVP at 0.1 ETH and 5 show up, each attendee gets 0.2 ETH.
6. **Cooling Period** — 1 week after finalization for withdrawals. After that, remaining funds go to the organizer.
7. **Clear** — Admin calls `clear()` to collect unclaimed deposits after the cooling period.

### 2.6 Key Features

| Feature | Description |
|---------|-------------|
| **Deposit staking** | ETH or ERC20 token deposit to RSVP |
| **Proportional payout** | No-show deposits redistributed to attendees who showed up |
| **Multi-admin** | Organizer + additional admin accounts for check-in |
| **Token-gated events** | NFT-based access control for online events (late addition) |
| **POAP integration** | Proof of Attendance Protocol badges for event participants |
| **Governance badges** | Snapshot integration for token-weighted voting |
| **Ethereum live feed** | Real-time on-chain event activity display |
| **Multi-network** | Ethereum, xDai, Polygon support |
| **Cooling period** | 1-week window for withdrawals before organizer claims remainder |
| **Event cancellation** | Full refund if event is cancelled |
| **Contribution/donation** | Optional donation of forfeited deposits to causes |

### 2.7 Season Zero Stats (Final Season, Jul–Oct 2021)

From the ChillaxDAO "season zero":

| Metric | Value |
|--------|-------|
| Total participants (staking) | 1,155 |
| Total events | 23 |
| Total attendance | 838 (465 unique) |
| Attendance rate | 76% |
| Average events per person | 1.8 |
| Staking range | 0.01 MATIC – 15.599 MATIC |
| Total staked | 8,159 MATIC (~$14,767) |

### 2.8 Revenue Model

Kickback's revenue model was unclear/unsustainable:

- **No explicit platform fee** documented in the core protocol
- A late commit message mentions "Add platform fee" (Sep 2021, `multiple POAP badge` PR) — suggesting they were experimenting with fees near the end
- The **organizer collected unclaimed deposits** after the cooling period, but this was a feature, not platform revenue
- **No subscription model** for organizers
- The company received a "small amount of investment from Status incubate" but no evidence of further funding rounds
- **Zero revenue from honest attendees** — the protocol redistributed all funds

### 2.9 Technology Stack

| Layer | Technology |
|-------|-----------|
| **Smart Contracts** | Solidity (Truffle, OpenZeppelin) |
| **Frontend** | React (JavaScript), Apollo GraphQL, bnc-onboard.js (wallet) |
| **Backend** | Node.js, PostgreSQL (closed-source) |
| **Indexing** | The Graph (kickback-subgraph) |
| **Wallet Integration** | Blocknative Onboard.js (MetaMask, WalletConnect, Torus) |
| **Deployment** | Vercel (frontend) |
| **Testing** | Cypress (E2E), Truffle test suite |
| **Analytics** | Custom analytics server |
| **Bots** | Discord bot, Telegram bot |

---

## 3. Feature Comparison: Kickback vs BeThere

| Feature | Kickback | BeThere |
|---------|----------|---------|
| **Blockchain** | Ethereum / xDai / Polygon | Solana |
| **Deposit Token** | ETH or ERC20 (variable) | USDC (stablecoin, 6 decimals) |
| **Price Volatility** | HIGH — ETH/MATIC volatile; attendees could lose value between RSVP and refund | NONE — USDC is stable; $15 deposit stays $15 |
| **Deposit Model** | Proportional redistribution (no-shows' deposits split among attendees who showed up) | Full refund for checked-in attendees; forfeited deposits go to organizer |
| **Payout Model** | Attendees get MORE than they deposited if others no-show | Attendees get exactly what they deposited back |
| **Revenue Model** | Unclear / unsustainable (late platform fee experiment) | Protocol fee on forfeited deposits only (2–5%, configurable) |
| **Gas Costs** | HIGH on Ethereum mainnet; moderate on L2s | LOW — Solana sub-cent transaction fees |
| **Transaction Speed** | 12s+ (Ethereum), 2–5s (Polygon) | ~400ms finality (Solana) |
| **Withdrawal** | Manual pull — user must call `withdraw()` within cooling period | Self-service refund after event end; no deadline pressure |
| **Cooling Period** | 1 week (hardcoded default) | Refund deadline (configurable, set by organizer) |
| **Anti-Rug-Pull** | WEAK — organizer could refuse all check-ins and claim all deposits after cooling period | STRONG — time-based refund eligibility; after event_end, anyone can refund regardless of check-in status |
| **Check-in** | Off-chain (admin marks attendance) | On-chain (organizer signs `mark_checked_in` TX) + QR scan |
| **NFT Badge** | Late addition (POAP, optional) | Core feature (cNFT via Bubblegum, always minted on claim) |
| **Token-Gated Events** | Yes (late feature) | Planned (roadmap) |
| **Multi-Event** | Yes (per-event contracts) | Yes (KV-based event registry with per-event config) |
| **Multi-Network** | Ethereum, xDai, Polygon | Solana (devnet + mainnet) |
| **Event Cancellation** | Yes — full refund | Yes — full refund via `deactivate_event` |
| **Frontend** | React (JS) | Leptos (Rust WASM) |
| **Backend** | Node.js + PostgreSQL | Cloudflare Workers (edge, serverless) |
| **Data Storage** | The Graph (subgraph) + PostgreSQL | Google Sheets + Solana on-chain + Cloudflare KV |
| **Wallet Support** | MetaMask, WalletConnect, Torus | Phantom, Solflare, Backpack (Solana adapter standard) |
| **QR Code** | Not documented for check-in | Core feature (BarcodeDetector + jsQR fallback) |
| **Quiz-Gated Claim** | No | Yes (quiz before NFT claim) |
| **Adventure Game** | No | Yes (educational Rust Adventures game) |
| **Staff Roles** | Admin-only (organizer + admins) | 3-tier: super_admin → organizer → staff |
| **Dual-Track Payments** | No (crypto only) | Yes (USDC on-chain + THB PromptPay QR for local events) |
| **Open Source** | Partial (frontend + contracts MIT; backend closed) | Full codebase |
| **Smart Contract Audit** | Self-audit documented (`doc/SelfAuditV084.md`) | Security audit (11 findings, 8 fixed) |
| **Deposit Immutability** | Mutable until first RSVP | Immutable from creation |
| **Rent Reclamation** | N/A (EVM model) | Yes — close PDA accounts to reclaim SOL rent |

---

## 4. Why Kickback Shut Down (Lessons for BeThere)

### 4.1 Root Causes

#### 1. Founder Burnout & Opportunity Cost
Makoto Inoue and Jeff Lau were both core ENS DAO contributors. As ENS grew into a major protocol, their Kickback responsibilities became impossible to maintain. By the shutdown announcement, Makoto had been the **sole maintainer for 12 months** with no new features in the last 3 months.

**Lesson:** Build a team, not a founder-dependent project. Have clear succession and contributor onboarding paths.

#### 2. Single Point of Failure
One person maintained the entire codebase (frontend, backend, smart contracts, subgraph). No bus factor mitigation.

**Lesson:** Open-source everything from day one. Lower contribution barriers. Document aggressively.

#### 3. Ethereum Gas Cost Friction
The entire network migration (Mainnet → xDai → Polygon) was forced by gas costs. On Ethereum mainnet, a simple RSVP could cost $5–50 in gas fees — often exceeding the deposit amount itself. This made the platform unusable for small events.

**Lesson:** Choose a chain where transaction costs are negligible relative to deposit amounts. Solana's sub-cent fees solve this.

#### 4. No Sustainable Revenue Model
Kickback never established a clear revenue stream. The platform was essentially a free public good that required ongoing maintenance of:
- Node.js backend server
- PostgreSQL database
- The Graph subgraph
- Frontend hosting
- Smart contract monitoring

With no income, the project was pure cost for the founders.

**Lesson:** Design monetization from day one. BeThere's protocol fee on forfeited deposits is sustainable without taxing honest attendees.

#### 5. Cooling Period UX Problem
Users had to manually withdraw within a 1-week cooling period. Many forgot, couldn't, or didn't know they needed to. This was acknowledged in KIP-1: *"In practice we find that many users either a) are unaware they need to withdraw, b) forget to withdraw, c) are unable to withdraw within the cooling period, or d) would prefer auto-withdrawal."*

**Lesson:** Make refund the default, not an action. Time-based refund eligibility (anytime after event_end) removes deadline anxiety.

#### 6. Rug Pull Risk
The organizer could refuse all check-ins, preventing anyone from withdrawing, and then claim all deposits after the cooling period. While no one reported this happening, the trust assumption was a barrier to adoption.

**Lesson:** Make the protocol trustless. BeThere's time-based refund eligibility means organizers **cannot** rug pull — attendees can always self-refund after the event.

#### 7. Proportional Payout Complexity
The model where no-shows' deposits get redistributed to attendees who showed up created complexity:
- Attendees received **more than they deposited**, creating tax/reporting confusion
- The math was opaque to non-crypto users
- It required the `finalize()` step with bitmap data

**Lesson:** Keep it simple. BeThere's model (full refund if you show up, forfeiture if you don't) is cleaner and more intuitive.

#### 8. Crypto-Native Barrier
Kickback required users to:
1. Install a browser wallet (MetaMask)
2. Acquire ETH/MATIC/xDAI
3. Understand gas fees
4. Manage seed phrases
5. Manually interact with smart contracts

This limited adoption to crypto-native users only.

**Lesson:** Reduce onboarding friction. Solana wallet adapters (Phantom, Backpack) are simpler. Future: custodial/sponsored wallets for non-crypto users.

### 4.2 What Kickback Got Right

Despite the shutdown, Kickback validated several critical hypotheses:

1. **Skin-in-the-game works.** 76% attendance rate vs 40–60% industry average for free RSVPs.
2. **The concept is educational.** Makoto noted: *"Our simple 'skin in the game' staking system help explain the power of the crypto-economics to people new to crypto."*
3. **Community building works.** ChillaxDAO formed around the product, with event organizers worldwide contributing feedback.
4. **Multi-network support matters.** Users followed the product across chains when gas made it necessary.
5. **Small events are viable.** Deposit amounts ranged from 0.01 MATIC (~$0.01) to 15.6 MATIC (~$28), proving the model works across scales.

---

## 5. Key Differences / BeThere Advantages

### 5.1 Architectural Advantages

| Area | Kickback | BeThere Advantage |
|------|----------|-------------------|
| **Chain** | EVM (variable) | Solana: sub-cent fees, 400ms finality, no MEV |
| **Backend** | Node.js + PostgreSQL (server) | Cloudflare Workers (serverless, edge, zero ops) |
| **Data** | The Graph + PostgreSQL | Google Sheets (familiar) + Solana on-chain (trustless) |
| **Frontend** | React JS | Leptos Rust WASM (type-safe, fast, shared domain crate) |
| **Per-Event Cost** | ~$5–50 gas (Ethereum), ~$0.01–0.10 (L2) | < $0.001 per event initialization |

### 5.2 Product Advantages

| Area | Kickback | BeThere Advantage |
|------|----------|-------------------|
| **Deposit Token** | ETH/MATIC (volatile) | USDC (stablecoin — $15 stays $15) |
| **Refund Model** | Proportional (complex) | Full refund (simple) |
| **Anti-Rug-Pull** | Trust organizer | Time-based refund (protocol-enforced) |
| **Withdrawal** | Manual within cooling period | Anytime after event end (no deadline) |
| **Check-in** | Off-chain (trust admin) | On-chain (verifiable) |
| **NFT Badge** | Optional POAP | Core feature (cNFT, always minted) |
| **Dual Payments** | Crypto only | USDC + local fiat (PromptPay/THB) |
| **Gamification** | None | Quiz-gated claim + Rust Adventures |

### 5.3 Business Model Advantages

| Area | Kickback | BeThere Advantage |
|------|----------|-------------------|
| **Revenue** | None (unsustainable) | Protocol fee on forfeited deposits (2–5%) |
| **Who Pays** | Nobody (free product) | Only no-shows (honest attendees pay nothing) |
| **Incentive Alignment** | Misaligned (no revenue = no sustainability) | Aligned: platform earns when events succeed (volume), not when users get scammed |
| **Scalability** | Limited by server costs | Serverless (Cloudflare Workers = near-zero marginal cost) |

### 5.4 Security Advantages

| Area | Kickback | BeThere Advantage |
|------|----------|-------------------|
| **Rug Pull Protection** | None (trust organizer for check-in) | Time-based refund eligibility (protocol-enforced) |
| **Deposit Mutability** | Organizer can change deposit until first RSVP | Immutable from creation |
| **Smart Contract Audit** | Self-audit only | Professional security audit (11 findings addressed) |
| **Fund Custody** | Per-event contract holds funds | PDA escrow (Solana program-enforced) |
| **Arithmetic Safety** | Solidity (overflow risks in older versions) | Rust (checked arithmetic by default) |

### 5.5 Where BeThere Can Still Learn from Kickback

Kickback had several features BeThere hasn't implemented yet:

1. **Token-gated events** — Allow events restricted to NFT/token holders. Kickback supported this late in its life.
2. **Governance badges** — Snapshot integration for token-weighted voting by attendees.
3. **Proportional payout option** — Some organizers preferred the "attendees profit from no-shows" model as an attendance incentive.
4. **Community DAO (ChillaxDAO)** — Kickback built a community of event organizers who gave product feedback and cross-promoted events.
5. **ERC20/token flexibility** — Kickback supported any ERC20 token as deposit, not just a single stablecoin.
6. **KIP process** — Kickback Improvement Proposals provided structured feature planning.

---

## 6. References

### Primary Sources

1. **Kickback Shutdown Announcement** — Makoto Inoue, "Towards the end of Kickback as we know," Medium (wearekickback), Feb 11, 2022
   - 1,155 participants, 23 events, 76% attendance rate, 8,159 MATIC staked (~$14,767)
   - Shutdown reasons: sole maintainer burnout, ENS DAO responsibilities
   - Seeking organization or DAO to take over operations

2. **Kickback Contracts Repository** — [github.com/wearekickback/contracts](https://github.com/wearekickback/contracts)
   - Solidity smart contracts (MIT license)
   - Factory pattern: Deployer.sol → EthConference.sol / ERC20Conference.sol
   - Per-event contract deployment
   - Detailed business logic documentation in README

3. **Kickback Frontend Repository** — [github.com/wearekickback/app](https://github.com/wearekickback/app)
   - React + Apollo GraphQL frontend
   - Multi-network support (Ethereum, xDai, Polygon)
   - Blocknative Onboard.js for wallet integration
   - Cypress E2E tests

4. **KIP-1: Easier Withdrawals** — [github.com/wearekickback/KIPs/blob/master/kips/kip-1.md](https://github.com/wearekickback/KIPs/blob/master/kips/kip-1.md)
   - Proposed `UserPot` contract (singleton for all user funds)
   - Acknowledged UX problems: users forget to withdraw, cooling period confusion
   - Status: Superseded by KIP-2

5. **Kickback GitHub Organization** — [github.com/wearekickback](https://github.com/wearekickback)
   - 13 repositories (app, contracts, subgraph, shared, analytics-server, discord-bot, etc.)
   - 5 followers, last updates Jan 2023
   - JavaScript/TypeScript/Vue stack

### Medium Articles (Titles from Blog Index)

These articles were listed on the Kickback Medium publication but are now 404 (publication removed):

1. "Use Kickback NFT for token gated online events" — Sep 11, 2021
2. "Kickback and Chillax (DAO): The future of event coordination" — Sep 5, 2021
3. "Hello Polygon!" — Jun 30, 2021
4. "Kickback new features: Governance badges, Ethereum life feed, and Token gated event page" — Jun 21, 2021
5. "7 Defi Dapps in 30 days" — Jan 25, 2021
6. "Learn something new with Kickback" — Dec 29, 2020
7. "Kickback New Year resolution challenge" — Dec 15, 2020
8. "Call for Kickback events: xDAI double Kickback campaign" — Oct 2, 2020
9. "The attack of Uniswap clones" — Sep 11, 2020

### BeThere Internal References

- `docs/escrow_protocol.md` — Full BeThere escrow protocol specification
- `docs/security_audit.md` — Security audit findings (11 findings, 8 fixed)
- `README.md` — Architecture overview and feature list
- `DISCUSSION.md` — Architecture direction and decisions

### Key Takeaway

Kickback proved the concept. BeThere perfects the execution.

The deposit-backed attendance model works (76% attendance rate vs 40–60% industry average). But Kickback's implementation choices — Ethereum's gas costs, volatile deposit tokens, proportional payout complexity, organizer-trusted check-in, manual withdrawal deadlines, and zero revenue model — were all structural weaknesses that contributed to its demise.

BeThere addresses every one of these while preserving the core insight: **skin in the game creates commitment, and commitment creates better events.**
