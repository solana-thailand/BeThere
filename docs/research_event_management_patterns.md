# Research: Event Management in Microservice Architectures & BeThere

**Source**: "An Empirical Study on Challenges of Event Management in Microservice Architectures" (arXiv 2408.00440)

## Paper Summary

The paper surveyed 1,675 microservice practitioners and analyzed 21 systems to catalog 16 challenges across 5 categories in event-driven architectures, mapping them to 8 functional requirements and identifying the most common patterns.

---

## The 16 Challenges → BeThere Mapping

### ✅ Already Solved by Solana's Architecture (8/16)

| Challenge | ID | How BeThere Solves It |
|-----------|----|----------------------|
| **Publishing safely** | CAS1 | Solana TX are atomic — deposit/refund either fully commits or reverts. No partial state. |
| **Processing dependencies** | CAS3 | Escrow flow enforces strict ordering via PDA state: `create_event → deposit → check_in → refund/claim`. Program rejects out-of-order instructions. |
| **Event ordering** | CAS4 | Solana's sequenced ledger + PDA-based state provides deterministic ordering per event escrow. |
| **Weak delivery semantics** | CAS7 | Solana provides confirmation finality (~400ms). No "at-least-once" ambiguity — TX either confirms or fails. |
| **Modeling event schemas** | CEM1 | Rust types in `domain/` crate + Quasar account structs provide compile-time schema enforcement across worker and on-chain program. |
| **Large payloads** | CP2 | BeThere events are minimal (deposit amount + pubkey + timestamp). No large event payloads. |
| **Authentication** | CS1 | JWT HMAC-SHA256 for worker + wallet signature for on-chain. Two-tier auth already in production. |
| **Continuous queries** | CAS6 | Not applicable — BeThere is request-driven (check-in, refund), not a streaming/CQRS system. |

### ⚠️ Partially Solved — Needs Attention (5/16)

| Challenge | ID | Current State | Gap |
|-----------|----|---------------|-----|
| **Roll-backing states** | CAS2 | Solana TX are atomic (no partial). But **KV state** in Cloudflare Worker has no rollback — if on-chain TX succeeds but KV write fails, state diverges. | Need idempotent write-through or compensating action. |
| **Synchronizing via replay** | CAS5 | On-chain state is fully replayable (Solana ledger). **KV state is ephemeral** — no event sourcing, no replay. | If KV loses state, can reconstruct from Google Sheets but NOT from on-chain (escrow ≠ full event state). |
| **Processing overhead** | CP1 | KV cache eliminates Sheets scan (~200-800ms saved). But **Solana RPC calls** add ~400ms per TX. Helius cNFT minting is separate latency. | Batch operations for high-traffic events. Pre-build TX bundles. |
| **Event flow observability** | CO1 | Solana Explorer shows on-chain TX. Worker has `health` endpoint. But **no cross-system trace** (KV ↔ on-chain ↔ Sheets ↔ wallet). | Add correlation IDs across worker + on-chain instructions. |
| **Auditing via replay** | CO2 | On-chain TX are auditable forever. But **off-chain state changes** (check-in logs, quiz scores, adventure saves) live only in KV/Sheets. | Consider logging all mutations as immutable events. |

### 🔴 Still Relevant — Active Risk (3/16)

| Challenge | ID | Risk | Mitigation Plan |
|-----------|----|------|-----------------|
| **Evolving schemas** | CEM2 | On-chain program upgrade requires careful migration of existing PDAs. `EventEscrow` and `AttendeeDeposit` accounts are immutable once created. | Version account discriminators. Deploy new program ID for breaking changes. Use `initialize` guard strictly. |
| **Fluctuating event rates** | CP3 | Check-in spikes at event start (hundreds in minutes). Solana has capacity, but **worker KV** has per-request limits and **RPC** may rate-limit. | Pre-warm caches. Use priority fees. Queue check-ins if RPC congested. |
| **Data privacy** | CS2 | On-chain deposits expose attendee pubkeys + amounts (public ledger). USDC amounts are visible. | Accept as trade-off (public blockchain). Consider confidential transfers (Token-2022) for future. |

---

## Functional Requirements Mapping

| FR | Requirement | BeThere Status |
|----|------------|----------------|
| FR1 | State update propagation | ✅ PDA state + KV write-through |
| FR2 | Multi-service workflows | ✅ Worker ↔ Solana ↔ Sheets ↔ Frontend |
| FR3 | Data integrity | ⚠️ Atomic on-chain, eventual KV |
| FR4 | Event replay | ⚠️ On-chain yes, KV no |
| FR5 | Query processing | ✅ KV cache + Sheets read-through |
| FR6 | Data replication | ✅ KV cache mirrors Sheets |
| FR7 | Cache management | ✅ 30s TTL + write-through invalidation |
| FR8 | Task scheduling | ✅ `event_end` / `refund_deadline` guards on-chain |

---

## Top Patterns (Paper) → BeThere Equivalents

| Pattern (Usage Count) | BeThere Equivalent |
|----------------------|-------------------|
| **Messaging** (1,056) | Solana TX = messages with guaranteed delivery + ordering |
| **Event Sourcing** (537) | Partial — on-chain ledger is event source, but KV/Sheets is CRUD |
| **Database per Service** (424) | Each "service" has own store: Solana (PDAs), Worker (KV), Sheets |
| **API Gateway** (249) | Cloudflare Worker = API gateway (auth, routing, rate limiting) |
| **CQRS** (implicit) | Worker reads from KV/Sheets, writes to both KV + Solana |

---

## Actionable Takeaways for BeThere

### Already Done Well
1. **Atomic transactions** — Solana eliminates an entire class of distributed consistency problems (CAS1, CAS3, CAS4, CAS7)
2. **Schema enforcement** — Rust types + Quasar structs = compile-time guarantees (CEM1)
3. **Cache strategy** — KV write-through with TTL solves the read amplification problem (FR5, FR7)
4. **Security** — Two-tier auth (JWT + wallet) covers the paper's top security concerns (CS1)

### Top 3 Priorities Before Mainnet
1. **KV ↔ On-chain consistency** — Add idempotent reconciliation. If on-chain TX confirms but KV write fails, a background job should detect and repair state divergence. (CAS2, CO2)
2. **Correlation tracing** — Add a `correlation_id` to worker logs + on-chain instruction metadata. Enables end-to-end debugging of the check-in flow. (CO1)
3. **Event spike handling** — Load-test the check-in flow at 100+ concurrent users. Identify RPC rate limits and KV write throughput bounds. Queue if needed. (CP3)

### Future Consideration
- **Token-2022 Confidential Transfers** — Addresses CS2 (data privacy) by hiding deposit amounts. Low priority since pubkeys remain public.
- **Full event sourcing in KV** — Instead of CRUD state, store mutation events. Enables replay (CAS5) and audit (CO2). Medium priority.
