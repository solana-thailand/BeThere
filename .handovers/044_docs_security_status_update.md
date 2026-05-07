# Handover 044: Docs Update + Phase 2 Frontend Security Fixes

**Date**: 2026-05-07
**Branch**: `feature/010_deposit_refund_escrow`

## What Happened

1. Updated all documentation to reflect security fixes from Phases 1 and 3 (6 findings were resolved in code but still marked "Open" in docs)
2. Committed Phase 3 on-chain changes (SEC-001/009/011) together with the docs update
3. Implemented Phase 2 frontend fixes (SEC-005 cluster-aware explorer links, SEC-006 duplicate Merkle field)

## Changes Made

### README.md
- **Escrow Architecture**: Removed stale ⚠️ SEC-001 warning, replaced with fixed note
- **Important constraints**: Added `mark_checked_in` event_end guard (SEC-011), `transfer_checked()` note (SEC-009), refund "no check-in required" clarification
- **Security table**: All 3 escrow rows upgraded from ⚠️ to ✅ Secure
- **API endpoints**: Added `POST /api/events/{id}/restore` (was missing from Phase 1)
- **Roadmap**: Phase 9a changed from 🔴 Blocks Mainnet → 🟡 Phase 1+3 done, needs rebuild + redeploy
- **Security reference**: Updated to "11 findings, 6 fixed"

### SECURITY.md
- **Scope table**: Fixed `Anchor` → `Quasar` (wrong framework name)
- **Finding statuses**: Updated SEC-001/002/003/004/009/011 from "Open" to "✅ Fixed (Phase N)"

### docs/security_audit.md
- **Findings Summary**: 6 status cells updated from "Open" to "✅ Fixed (Phase N)"
- **SEC-001 detail**: Status line now says "Fixed (Phase 3) — `checked_in` constraint removed from `refund.rs`"
- **Remediation Priority table**: Added Status column, all 6 P0-P5 marked Fixed
- **Scope for Mainnet**: Removed SEC-001/002/003/009 from "MUST FIX", SEC-004/011 from "SHOULD FIX", only SEC-005 remains
- **Trust Model appendix**: Rewrote to reflect SEC-001 fix applied — no longer a trust assumption

## Remaining Open Findings

| ID | Title | Notes |
|----|-------|-------|
| SEC-010 | AttendeeDeposit PDAs never closed (rent leak) | Requires new `close_deposit` on-chain instruction + UI button |

### Phase 2 Frontend Fixes (SEC-005/006)

| File | Fix | Details |
|------|-----|--------|
| `worker/src/handlers/health.rs` | SEC-005 | Add `cluster` field to health response (from RPC URL) |
| `frontend-leptos/src/utils.rs` | SEC-005 | Add `fetch_cluster()`, `get_cluster()`, `solscan_tx_url()`, `solscan_address_url()` helpers |
| `frontend-leptos/src/pages/scanner.rs` | SEC-005 | Replace hardcoded `?cluster=devnet` with `solscan_tx_url()` |
| `frontend-leptos/src/pages/deposit.rs` | SEC-005 | Replace 2 hardcoded devnet URLs (deposit + refund confirmation) |
| `frontend-leptos/src/pages/escrow_init.rs` | SEC-005 | Replace hardcoded devnet URL in escrow init success |
| `frontend-leptos/src/pages/admin_escrow.rs` | SEC-005 | Replace hardcoded devnet URL in action result |
| `frontend-leptos/src/pages/claim.rs` | SEC-005 | Refactor to use shared `solscan_tx_url`/`solscan_address_url` helpers |
| `frontend-leptos/src/pages/events_page.rs` | SEC-006 | Remove duplicate Merkle Tree field from NFT Settings section |

## Plan / Next Steps

1. **Rebuild + redeploy escrow program** — `quasar build` → devnet → E2E validation → mainnet
2. **Phase 4: Rent reclamation** — SEC-010 `close_deposit` instruction
3. **Phase 5: UX improvements** — search/filter, progressive disclosure form

## Issues Ref

- `.issues/013_escrow_rug_pull_prevention.md` — SEC-001 original finding
- `.handovers/043_cargo_zed_cleanup_escrow_diagnostics.md` — prior session context
