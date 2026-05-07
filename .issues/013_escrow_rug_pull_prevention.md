# 013 — Escrow Rug Pull Prevention

## Summary
Security audit identified a critical rug pull vector: the organizer controls check-in, which gates refunds. If the organizer refuses to check in attendees, they can claim all deposits as forfeited. This issue tracks all security fixes from the audit.

## Status: RESOLVED

All 11 findings addressed: 9 fixed (Phases 1–4), 2 confirmed safe.

## Findings (from security audit)

| ID | Severity | Title | Fix Scope |
|----|----------|-------|-----------|
| SEC-001 | 🔴 Critical | Check-in gate enables complete fund theft | On-chain + Backend |
| SEC-002 | 🟠 High | Escrow fields mutable after on-chain init | Backend |
| SEC-003 | 🟡 Medium | No maximum deposit cap | Backend + On-chain |
| SEC-004 | 🟡 Medium | Archive doesn't deactivate on-chain escrow | Backend |
| SEC-005 | 🟡 Medium | Explorer links hardcoded to devnet | Frontend |
| SEC-006 | 🟢 Low | Duplicate Merkle Tree field | Frontend |
| SEC-007 | 🟢 Info | Worker cannot manipulate funds | ✅ Safe |
| SEC-008 | 🟢 Info | On-chain escrow fields immutable after creation | ✅ Safe |
| SEC-009 | 🟡 Medium | Token transfers use `transfer()` not `transfer_checked()` | On-chain |
| SEC-010 | 🟡 Medium | AttendeeDeposit PDAs never closed (rent leak) | On-chain + Frontend |
| SEC-011 | 🟡 Medium | No `event_end` guard on `mark_checked_in` | On-chain |

**Cross-reference**: [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) by Frank Castle (124⭐, 70+ Rust audits, 250+ Critical/High findings)

## Implementation Plan

### Phase 1: Backend Guards (Quick Wins) ✅ Done
- [x] Lock `organizer_wallet`, `on_chain_event_id`, `deposit_amount_usdc`, `refund_deadline_hours` in `update_event` when `escrow_address` is non-empty (SEC-002)
- [x] Add max deposit cap ($1,000 USDC) in backend validation (SEC-003)
- [x] Archive guards: check escrow state before allowing archive (SEC-004)
- [x] Add `restore_event` endpoint for unarchiving

### Phase 2: Frontend Fixes ✅ Done
- [x] Make all explorer links cluster-aware (SEC-005)
- [x] Remove duplicate Merkle Tree field (SEC-006)
- [ ] Add escrow account verification (show ✅/❌ if account exists on-chain)
- [ ] Persist each step's Solscan link in admin_escrow panel

### Phase 3: On-Chain Fixes (SEC-001, SEC-009, SEC-011) ✅ Done
- [x] SEC-001: Modify refund instruction: allow refunds after `event_end` regardless of `checked_in`
- [x] SEC-001: Keep `checked_in` for analytics/NFT eligibility, just don't gate refunds
- [x] SEC-009: Replace `transfer()` with `transfer_checked()` in 3 sites (deposit, refund, claim_forfeited)
- [x] SEC-011: Add `event_end` guard on `mark_checked_in` (no check-ins after event ends)
- [x] Update tests in `bethere-escrow/src/tests.rs` (26/26 pass)
- [ ] Redeploy program to devnet for testing
- [ ] Redeploy program to mainnet

### Phase 4: Rent Reclamation (SEC-010) ✅ Done
- [x] Add `close_deposit` instruction to escrow program
- [x] Attendee can close own deposit PDA after `refunded == true`
- [x] Anyone can close deposit PDAs after event escrow is closed
- [x] Add TX builder in worker
- [x] Add UI button for attendees to reclaim rent

### Phase 5: UX Improvements
- [ ] Event search/filter/pagination on events page
- [ ] Progressive disclosure for event form (collapse advanced sections)
- [ ] Visual priority indicators on form fields (required/recommended/optional)
- [ ] Hide `on_chain_event_id` from main form view

## Refs
- `docs/security_audit.md` — Full security audit findings
- `docs/escrow_protocol.md` — Protocol design with win-win-win model
- Issue 010 — Deposit/refund escrow architecture
- Issue 012 — Escrow panel single-TX migration
- Handover 042 — Admin escrow UI + mint config

## How to Test
```bash
# Phase 1: Backend guards
cd worker && cargo test --lib

# Phase 2: Frontend
cd frontend-leptos && cargo check --target wasm32-unknown-unknown
bash build.sh

# Phase 3: On-chain
cd bethere-escrow && cargo test-sbf
# Deploy: solana program deploy target/deploy/bethere_escrow.so --url devnet

# E2E: full lifecycle test on devnet
```
