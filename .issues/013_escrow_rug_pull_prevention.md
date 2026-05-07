# 013 — Escrow Rug Pull Prevention

## Summary
Security audit identified a critical rug pull vector: the organizer controls check-in, which gates refunds. If the organizer refuses to check in attendees, they can claim all deposits as forfeited. This issue tracks all security fixes from the audit.

## Status: OPEN

## Findings (from security audit)

| ID | Severity | Title | Fix Scope |
|----|----------|-------|-----------|
| SEC-001 | 🔴 Critical | Check-in gate enables complete fund theft | On-chain + Backend |
| SEC-002 | 🟠 High | Escrow fields mutable after on-chain init | Backend |
| SEC-003 | 🟡 Medium | No maximum deposit cap | Backend + On-chain |
| SEC-004 | 🟡 Medium | Archive doesn't deactivate on-chain escrow | Backend |
| SEC-005 | 🟡 Medium | Explorer links hardcoded to devnet | Frontend |
| SEC-006 | 🟢 Low | Duplicate Merkle Tree field | Frontend |

## Implementation Plan

### Phase 1: Backend Guards (Quick Wins)
- [ ] Lock `organizer_wallet`, `on_chain_event_id`, `deposit_amount_usdc`, `refund_deadline_hours` in `update_event` when `escrow_address` is non-empty (SEC-002)
- [ ] Add max deposit cap ($1,000 USDC) in backend validation (SEC-003)
- [ ] Archive guards: check escrow state before allowing archive (SEC-004)
- [ ] Add `restore_event` endpoint for unarchiving

### Phase 2: Frontend Fixes
- [ ] Make all explorer links cluster-aware (SEC-005)
- [ ] Remove duplicate Merkle Tree field (SEC-006)
- [ ] Add escrow account verification (show ✅/❌ if account exists on-chain)
- [ ] Persist each step's Solscan link in admin_escrow panel

### Phase 3: On-Chain Fix (SEC-001 — Critical)
- [ ] Modify refund instruction: allow refunds after `event_end` regardless of `checked_in`
- [ ] Keep `checked_in` for analytics/NFT eligibility, just don't gate refunds
- [ ] Update tests in `bethere-escrow/src/tests.rs`
- [ ] Redeploy program to devnet for testing
- [ ] Redeploy program to mainnet

### Phase 4: UX Improvements
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
