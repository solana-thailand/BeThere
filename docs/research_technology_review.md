# Research & Technology Review

> Summary of academic/technical paper review sessions for BeThere project direction.
> Purpose: inform CTO/senior discussion on technology strategy.

## Papers Reviewed

### 1. Learning Beyond Gradients — Jiayi Weng (May 2026)

**Topic**: Heuristic Learning (HL) — coding agents maintain rule-based systems that "learn" without neural networks.

**Key Results**:
- Atari Breakout 864/864 (max score), MuJoCo Ant 6146 avg — all pure code, no NN training
- Core insight: heuristics were never useless, they were too expensive to *maintain*. Coding agents change the maintenance curve.

**BeThere Relevance: ✅ High (conceptual)**
- Describes the agent-assisted development workflow we already use
- Maps to: tests = regression protection, handovers = memory, refactoring = compression
- **Action**: None needed — we're already doing this. Useful as a framework for discussing our process.

---

### 2. RAVEN (arxiv 2604.17948)

**Topic**: Multi-agent LLM framework for auto-generating vulnerability analysis reports (Explorer → Analyst → Reporter → Judge pipeline + RAG).

**Key Results**:
- 54.21% remediation success on 105 NIST-SARD samples across 15 CWE types
- Best RAG config: Contextual Chunking + Hybrid Retrieval + LLM Reranker

**BeThere Relevance: ✅ Medium-High (methodology)**
- 4-agent pipeline + RAG over security databases transferable to Solana escrow security analysis
- Caveat: targets C memory corruption (CWE-119, CWE-787). Rust eliminates most of these by default.
- Would need Solana-specific vulnerability knowledge base (account validation, signer checks, CPI safety, rent-exempt)
- **Action**: Post-mainnet — adapt pipeline for BeThere escrow security review. Build Solana-specific vulnerability KB.

---

### 3. REM2.0 (arxiv 2601.19207v1)

**Topic**: Rust refactoring toolchain built on rust-analyzer. Extract Method refactoring with automatic lifetime/ownership repair. Optional equivalence verification (Rust → CHARON → AENEAS → Coq).

**BeThere Relevance: ✅ Medium (tool)**
- Applicable to worker/frontend Rust code
- NOT applicable to escrow program (uses unsafe CPI, Pod types outside AENEAS's supported subset)
- **Action**: New branch `develop/feature/017_rem20_refactor` when ready. Focus on worker TX builders + API handlers.

---

### 4. SpecPrune-VLA (arxiv 2509.05614) — Robotics VLA Model Pruning

**Topic**: Training-free, heuristic-based pruning for Vision-Language-Action (robotics) models. Two-level pruning (static + dynamic) with action-aware controller.

**BeThere Relevance: ❌ None (direct)**
- Robotics/ML model optimization — does not apply to event check-in dApp
- Conceptual pattern ("classify operation type → adapt behavior") is general but not novel
- **Action**: None.

---

### 5. TurboQuant (arxiv 2504.19874) — Vector Quantization for LLM KV Caches

**BeThere Relevance: ❌ None** — LLM inference optimization.

---

### 6. FlashPrefill (arxiv 2603.06199) — Sparse Attention for LLM Prefilling

**BeThere Relevance: ❌ None** — GPU kernel-level LLM inference optimization.

---

### 7. Screening Is Enough / Multiscreen (arxiv 2604.01178v3)

**Topic**: New LLM architecture replacing softmax attention with absolute relevance + explicit threshold (screening).

**BeThere Relevance: ❌ Direct none, Medium conceptual**
- "Absolute relevance with explicit rejection" pattern generalizes to retrieval/search problems
- Not actionable for BeThere's current scope

---

## Relevance Ranking

| Rank | Paper | Relevance | Type | Actionable? |
|------|-------|-----------|------|-------------|
| 1 | Learning Beyond Gradients | ✅ High | Conceptual (process) | Already doing it |
| 2 | RAVEN | ✅ Medium-High | Methodology (security) | Post-mainnet |
| 3 | REM2.0 | ✅ Medium | Tool (refactoring) | Next branch |
| 4 | Screening Is Enough | ❌ Medium | Conceptual | No |
| 5 | TurboQuant | ❌ Low | ML compression | No |
| 6 | FlashPrefill | ❌ Low | ML inference | No |
| 7 | SpecPrune-VLA | ❌ None | Robotics ML | No |

## Key Takeaway

**BeThere does not need ML/AI features.** The product is a deterministic, auditable event check-in dApp with escrow. The "modelless" technique (Heuristic Learning) applies to our **development process**, not the product — and we're already doing it.

Papers 5-7 (TurboQuant, FlashPrefill, SpecPrune-VLA) would only become relevant if BeThere adds AI features (e.g., smart event recommendations, fraud detection) — which is not the current direction.

---

## Event Management Perspective (Session 2)

> Papers reviewed from event management industry and software architecture perspectives.
> These complement the technology papers above with domain-specific insights.

### 8. From Event Management to Managing Events (ResearchGate 259117550)

**Topic**: Industry evolution from managing individual events to strategic event portfolio management.

**Status**: ⚠️ ResearchGate blocked (DDoS protection). Could not access full text.

**BeThere Relevance: ❓ Unknown (likely High)**
- Based on title: discusses the shift from operational event execution to strategic event portfolio management
- Could inform BeThere's product direction (multi-event management, analytics, organizer dashboard)
- **Action**: Re-fetch when ResearchGate access restored, or find alternate source.

---

### 9. Event Management Challenges in Microservice Architectures (arxiv 2408.00440)

**Topic**: Empirical study of 8,000+ Stack Overflow questions on event-driven microservice challenges. Identifies 16 specific challenges across 5 categories.

**Key Findings**:
- 5 categories: Safety & Liveness (CAS1-7), Schema Management (CEM1-2), Performance (CP1-3), Observability (CO1-2), Security (CS1-2)
- Top NFRs: Consistency (312), Decoupling (303), Scalability (249)
- Top patterns: Messaging (1056), Event Sourcing (537), CQRS (219)
- Key insight: developers struggle most with event ordering, delivery semantics, and cross-service state consistency

**BeThere Relevance: ✅ High (architecture validation + gap identification)**

**Solana eliminates 8 of 16 challenges** by design:

| Paper Challenge | BeThere Status | Why Solved |
|---------------|----------------|------------|
| CAS1: Publishing safely | ✅ Solved | Solana TX are atomic — no partial commits |
| CAS3: Event dependencies | ✅ Solved | CPI chain is synchronous within a TX |
| CAS4: Processing order | ✅ Solved | Solana's PoH provides global ordering |
| CAS7: Weak delivery | ✅ Solved | TX finality is deterministic |
| CEM1: Schema modeling | ✅ Solved | Rust types + Quasar structs = compile-time enforcement |
| CS1: Authentication | ✅ Solved | JWT + wallet signing = two-tier auth |
| CP2: Large payloads | ✅ Solved | Events are tiny (pubkey + amount + timestamp) |
| CP3: Fluctuating rates | ✅ Mitigated | Solana handles throughput; KV is the bottleneck |

**5 remaining gaps that need attention:**

| Gap | Challenge | Risk | Action |
|-----|-----------|------|--------|
| KV ↔ on-chain divergence | CAS2 (rollback) | ⚠️ Medium | Add reconciliation job + idempotent writes |
| No cross-system observability | CO1 (tracing) | ⚠️ Medium | Add `correlation_id` across worker ↔ chain ↔ KV |
| KV state not replayable | CAS5/CO2 | ⚠️ Medium | Event sourcing for KV writes or periodic snapshots |
| Schema migration for PDAs | CEM2 (evolution) | 🔴 High | Plan escrow program upgrade path before mainnet |
| Check-in spike capacity | CP3 (rate) | 🔴 High | Load test at 100+ concurrent check-ins |

**Action**: Pre-mainnet — address the 5 gaps. The PDA schema evolution (CEM2) is the highest risk.

---

### 10. Hikester — Event Management Application (arxiv 1801.06400)

**Topic**: Social event management app (create events, invite people, recommender system, spam filtering, parameter optimization). Built with Firebase + Node.js + React + neural networks.

**Key Features**:
- Recommender system (suggests events based on user behavior + social profile)
- Spam recognition (Naive Bayes + MLP neural network for content filtering)
- Parameter optimizer (suggests optimal time/date/location for new events based on historical data)
- Real-time updates via Firebase
- Cross-platform (Web + iOS + Android)

**BeThere Relevance: ✅ Medium (competitive analysis + feature comparison)**

| Feature | Hikester | BeThere | Notes |
|---------|----------|---------|-------|
| Event creation | ✅ | ✅ | Both support it |
| Attendee check-in | ❌ Not mentioned | ✅ Core feature | BeThere's differentiator |
| Payment/Deposit | ❌ None | ✅ USDC escrow | BeThere's unique value |
| NFT rewards | ❌ None | ✅ cNFT claims | BeThere's unique value |
| Recommender | ✅ Neural network | ❌ Not planned | CTO wants modelless approach |
| Spam filter | ✅ Neural network | ❌ Not needed | BeThere is organizer-controlled |
| Parameter optimizer | ✅ Neural network | ❌ Not planned | Could be heuristic-based |
| Architecture | Firebase (centralized) | Solana + Worker (decentralized) | Different trade-offs |

**Key takeaway**: Hikester is a Web2 social event app. BeThere is a Web3 event check-in dApp with escrow. They serve different markets. Hikester's AI features (recommender, spam, optimizer) could theoretically be done with heuristics — aligning with CTO's modelless direction.

**Action**: None for product. Useful as competitive reference.

---

### 11. ESeMan — Event Sequence Visualization (arxiv 2508.03974)

**Topic**: ESeMan system for interactive rendering of timeline visualizations for parallel event sequences (program traces, manufacturing pipelines). Uses hierarchical data structures + intelligent caching for sub-100ms fetch times.

**BeThere Relevance: ❌ None (direct), ✅ Medium (future admin dashboard)**
- Not applicable to current BeThere scope
- **Potential future use**: Admin dashboard showing attendee flow as timeline (check-in → deposit → claim → refund) with zoom/filter
- The hierarchical data structure pattern (coarse overview → detailed drill-down) maps to event analytics
- **Action**: Post-mainnet Phase B — if admin analytics dashboard is prioritized, borrow the hierarchical + cached rendering pattern.

---

### 12. (Duplicate of #9 — same paper, HTML version)

See entry #9 above.

---

## Updated Relevance Ranking

| Rank | Paper | Relevance | Type | Actionable? |
|------|-------|-----------|------|-------------|
| 1 | Learning Beyond Gradients | ✅ High | Conceptual (process) | Already doing it |
| 2 | **Event Management in Microservices** | ✅ High | Architecture validation + gaps | Pre-mainnet fixes |
| 3 | RAVEN | ✅ Medium-High | Methodology (security) | Post-mainnet |
| 4 | REM2.0 | ✅ Medium | Tool (refactoring) | Next branch |
| 5 | **Hikester** | ✅ Medium | Competitive analysis | Reference only |
| 6 | **From Event Management to Managing Events** | ❓ Unknown | Business strategy | Need full text |
| 7 | Screening Is Enough | ❌ Medium | Conceptual | No |
| 8 | **ESeMan** | ❌ Medium (future) | Visualization pattern | Post-mainnet Phase B |
| 9 | TurboQuant | ❌ Low | ML compression | No |
| 10 | FlashPrefill | ❌ Low | ML inference | No |
| 11 | SpecPrune-VLA | ❌ None | Robotics ML | No |

## Action Items

- [ ] Discuss Heuristic Learning framework with CTO — are we formalizing our dev process?
- [ ] **Pre-mainnet: address 5 architecture gaps from microservice event paper** (PDA schema evolution = highest risk)
- [ ] Post-mainnet: evaluate RAVEN-style security analysis for escrow program
- [ ] Post-mainnet: evaluate REM2.0 for worker/frontend Rust refactoring
- [ ] Decision: does BeThere ever plan to add AI features? (changes paper relevance)
- [ ] Re-fetch "From Event Management to Managing Events" paper — likely important for product strategy
- [ ] Post-mainnet Phase B: consider ESeMan pattern for admin analytics dashboard

## References

- [Learning Beyond Gradients](https://jiayiweng.github.io/posts/2026-05-01-learning-beyond-gradients/) — blog post
- [RAVEN](https://arxiv.org/abs/2604.17948) — vulnerability analysis
- [REM2.0](https://arxiv.org/abs/2601.19207v1) — Rust refactoring
- [SpecPrune-VLA](https://arxiv.org/abs/2509.05614) — VLA pruning
- [TurboQuant](https://arxiv.org/abs/2504.19874) — vector quantization
- [FlashPrefill](https://arxiv.org/abs/2603.06199) — sparse attention
- [Screening Is Enough](https://arxiv.org/abs/2604.01178v3) — attention alternative

### 13. Les Roches — Event Planning Step-by-Step Guide (Industry)

**Source**: [Les Roches Blog](https://lesroches.edu/blog/event-planning/) — hospitality management school industry guide

**12-Step Event Planning Process**:
1. Define purpose & goals
2. Know your audience
3. Set budget
4. Choose format & platform
5. Create detailed plan & timeline
6. Select venue
7. Build team
8. Marketing strategy
9. Finalize vendors & suppliers
10. On-site logistics & run sheet
11. Manage attendee experience
12. Measure success (KPIs)

**BeThere Relevance: ✅ High (product gap analysis)**

Mapping the 12 industry steps to BeThere's current capabilities:

| Step | Industry Standard | BeThere Status | Gap |
|------|------------------|----------------|-----|
| 1. Purpose & goals | Clear objectives & KPIs | ❌ Not tracked | No event goal/metric fields |
| 2. Know audience | Surveys, CRM, segmentation | ⚠️ Partial | Google Sheets provides attendee list, no segmentation |
| 3. Set budget | Detailed budget categories | ❌ Not tracked | No budget management features |
| 4. Format & platform | In-person/virtual/hybrid | ⚠️ Partial | In-person only, no virtual/hybrid support |
| 5. Plan & timeline | Workback timeline, milestones | ⚠️ Partial | Has event_start/end, no milestones/deadlines |
| 6. Select venue | Venue checklist, site visit | ❌ Not tracked | No venue fields (location, capacity, accessibility) |
| 7. Build team | Roles: coordinator, marketing, production | ✅ Done | organizer_emails + staff_emails + role matrix |
| 8. Marketing | Multi-channel, email, social, paid | ❌ Not tracked | No marketing tools or analytics |
| 9. Vendors & suppliers | Catering, AV, security contracts | ❌ Not tracked | No vendor management |
| 10. On-site logistics | Run sheet, signage, crowd control | ⚠️ Partial | QR check-in scanner exists; no run sheet |
| 11. Attendee experience | Pre-event comms, check-in, engagement | ✅ Strong | Check-in flow, quiz gate, NFT claim, deposit/refund |
| 12. Measure success | Attendance, engagement, ROI, feedback | ⚠️ Partial | Admin stats (attendee counts, deposit totals); no surveys/feedback |

**Key Trends from the article** (BeThere mapping):
- **Sustainable practices**: BeThere uses cNFTs (compressed, low energy) ✅
- **Event technology**: BeThere uses Solana + Wallet ✅
- **Hybrid/virtual**: BeThere is in-person only ❌
- **Personalization at scale**: BeThere has quiz-gated claims ⚠️
- **Inclusive/accessible design**: Not addressed ❌

**Summary**: BeThere excels at steps 7, 10 (partially), and 11 (attendee experience). It's weak on strategic planning (steps 1-3, 5-6, 8-9) and post-event (step 12). This is expected — BeThere is a **check-in & escrow tool**, not a full event management platform. The gaps represent potential Phase B/C features.

**Action**: None required pre-mainnet. Useful for product roadmap prioritization.

---

### Event Management Papers
- [From Event Management to Managing Events](https://www.researchgate.net/publication/259117550) — industry evolution (⚠️ access blocked)
- [Event Management in Microservices](https://arxiv.org/abs/2408.00440) — architecture challenges
- [Hikester](https://arxiv.org/abs/1801.06400) — competitor event management app
- [ESeMan](https://arxiv.org/abs/2508.03974) — event sequence visualization
- [Les Roches Event Planning Guide](https://lesroches.edu/blog/event-planning/) — industry 12-step process
