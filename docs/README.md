# BeThere — Documentation Index

> Deposit-backed event check-in platform on Solana.
> cNFT badges mint on **mainnet** (Crossmint); USDC escrow is still on **devnet** (see the go/no-go below).

---

## Start here

| Document | Description |
|---|---|
| [Architecture](architecture.md) | System overview: worker + Leptos WASM + D1/KV/DO + Sheets + Solana — **the map** |
| [Protocol POC Requirements](protocol_poc_requirements.md) | Formal "shall" requirements for every instruction, account, and flow — **the spec** |
| [Solana Protocol Architecture](solana_protocol_architecture.md) | Mermaid diagrams and visual flows for the entire protocol — **the diagrams** |

---

## Solana On-Chain & Escrow

| Document | Description |
|---|---|
| [Escrow Protocol](escrow_protocol.md) | Original protocol design document |
| [Escrow Contract Surface](escrow_contract_surface.md) | Every instruction/account of the on-chain program |
| [Deposit & Refund Flows](deposit-refund-flows.md) | USDC + THB deposit/refund lifecycles, credit rollover, post-claim UX |
| [Crossmint Minting](crossmint-minting.md) | Hosted cNFT minting (fire-and-poll, idempotency, devnet↔mainnet cluster routing) |
| [On-Chain Event Indexing](onchain_event_indexing.md) | How on-chain escrow events are indexed off-chain (Helius webhook + poller) |
| [PDA Migration Strategy](pda_migration_strategy.md) | Strategy for PDA seed changes |
| [Devnet E2E Walkthrough](devnet_e2e_walkthrough.md) | End-to-end testing guide on devnet |
| [Devnet Testing Guide](devnet_testing_guide.md) | How to run tests against devnet |

---

## Security & Audit

| Document | Description |
|---|---|
| [Handover 2026-08-15 — Credit + Security](HANDOVER-2026-08-15-credit-and-security.md) | Rolling-credit ledger rebuild (Model B) + all admin IDOR/PII fixes — **read this first** for the credit/deposit + admin-authz state |
| [Security Findings 2026-08-13](SECURITY-FINDINGS-2026-08-13.md) | This cycle's branch-review findings (IDOR, credit double-spend, wallet binding) + fixes |
| [Escrow Audit & Go/No-Go 2026-08-13](escrow-audit-2026-08-13.md) | Pre-mainnet audit of the escrow program + **USDC mainnet checklist** (F1–F8) |
| [Security Audit](security_audit.md) | Original SEC-001–015 review and checklist |
| [Audit Submission](audit_submission.md) | External-audit submission package |

---

## Architecture & Design

| Document | Description |
|---|---|
| [TDD/DDD Architecture](tdd_ddd_architecture.md) | Domain-driven design and testing strategy |
| [D1 Migration Architecture](d1_migration_architecture.md) | Database migration approach with Cloudflare D1 |
| [Durable Objects Architecture](durable_objects_architecture.md) | Per-event ACID locks via Cloudflare DO |
| [Portable Profile Design](portable-profile-design.md) | Sketch: portable attendee identity/reputation for other Solana apps |

---

## Operations & Deployment

| Document | Description |
|---|---|
| [Gradual Deploy Runbook](gradual_deploy_runbook.md) | Step-by-step deployment guide |
| [Staging Deploy Runbook](staging_deploy_runbook.md) | Staging deploy + verification |
| [Mainnet Readiness Runbook](mainnet_readiness_runbook.md) | What must be green before mainnet USDC |
| [Mainnet Deployment Checklist](mainnet_deployment_checklist.md) | Ordered mainnet cutover steps |
| [Mainnet Canary Mitigation Runbook](mainnet_canary_mitigation_runbook.md) | Canary + rollback for the no-native-%-canary edge deploy |
| [Events Management](events_management.md) | How events are managed in the system |
| [Campaigns Guide](campaigns_guide.md) | Multi-event series setup, attendee rewards, and UX gaps |
| [Sheet Setup](sheet_setup.md) | Google Sheets structure + service-account setup |
| [Business Flows — Event Page](business_flows_event_page.md) | UX and business flows for the event page |
| [Business Flow Verdict](business_flow_verdict.md) | Business-flow review conclusions |

---

## Research & Planning

| Document | Description |
|---|---|
| [Research — Event Management Patterns](research_event_management_patterns.md) | Patterns and approaches for event management |
| [Research — Technology Review](research_technology_review.md) | Technology stack evaluation |
| [Competitive Analysis — Kickback](competitive_analysis_kickback.md) | Analysis of competing platform |
| [UX Roadmap](ux_roadmap.md) | Planned UX improvements |
| [Sources](sources.md) | Evidence ledger for every metric claimed in the README |

---

## Misc

| Document | Description |
|---|---|
| [Presentation Materials](presentation_materials.md) | Slides and demo materials |
| [IslandDAO v4 Pitch](islanddao_v4_pitch.md) | Grant/pitch material |
| [Cloudflare Bug Report #10013](cloudflare_bug_report_10013.md) | Known Cloudflare issue tracking |
