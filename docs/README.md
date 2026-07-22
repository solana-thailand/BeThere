# BeThere — Documentation Index

> Deposit-backed event check-in platform on Solana.

---

## Protocol & Architecture

| Document | Description |
|---|---|
| [Protocol POC Requirements](protocol_poc_requirements.md) | Formal "shall" requirements for every instruction, account, and flow — **the spec** |
| [Solana Protocol Architecture](solana_protocol_architecture.md) | Mermaid diagrams and visual flows for the entire protocol — **the diagrams** |
| [Escrow Protocol](escrow_protocol.md) | Original protocol design document |

> **Read together:** Start with POC Requirements for the formal spec, then reference the Architecture doc for diagrams of each instruction and flow.

---

## Solana On-Chain

| Document | Description |
|---|---|
| [Devnet E2E Walkthrough](devnet_e2e_walkthrough.md) | End-to-end testing guide on devnet |
| [Devnet Testing Guide](devnet_testing_guide.md) | How to run tests against devnet |
| [On-Chain Event Indexing](onchain_event_indexing.md) | How on-chain events are indexed off-chain |
| [PDA Migration Strategy](pda_migration_strategy.md) | Strategy for PDA seed changes |
| [Security Audit](security_audit.md) | Security review and checklist |

---

## Architecture & Design

| Document | Description |
|---|---|
| [TDD/DDD Architecture](tdd_ddd_architecture.md) | Domain-driven design and testing strategy |
| [D1 Migration Architecture](d1_migration_architecture.md) | Database migration approach with Cloudflare D1 |
| [Durable Objects Architecture](durable_objects_architecture.md) | Per-event ACID locks via Cloudflare DO |

---

## Operations

| Document | Description |
|---|---|
| [Gradual Deploy Runbook](gradual_deploy_runbook.md) | Step-by-step deployment guide |
| [Events Management](events_management.md) | How events are managed in the system |
| [Campaigns Guide](campaigns_guide.md) | Multi-event series setup, attendee rewards, and UX gaps |
| [Business Flows — Event Page](business_flows_event_page.md) | UX and business flows for the event page |

---

## Research & Planning

| Document | Description |
|---|---|
| [Research — Event Management Patterns](research_event_management_patterns.md) | Patterns and approaches for event management |
| [Research — Technology Review](research_technology_review.md) | Technology stack evaluation |
| [Competitive Analysis — Kickback](competitive_analysis_kickback.md) | Analysis of competing platform |
| [UX Roadmap](ux_roadmap.md) | Planned UX improvements |

---

## Misc

| Document | Description |
|---|---|
| [Presentation Materials](presentation_materials.md) | Slides and demo materials |
| [Cloudflare Bug Report #10013](cloudflare_bug_report_10013.md) | Known Cloudflare issue tracking |
