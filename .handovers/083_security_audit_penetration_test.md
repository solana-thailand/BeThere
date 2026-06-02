# Handover 083: Security Audit & Penetration Test

## What Happened
Conducted a white-box penetration test of the BeThere Event Check-in platform. Systematically read all authentication middleware, route definitions, handler implementations, crypto/JWT code, claim logic, deposit flows, webhook handlers, storage serving, RBAC enforcement, and security headers across ~25 files.

## Findings
14 vulnerabilities identified (2 Critical, 2 High, 5 Medium, 5 Low).

### Core Pattern
6 of 14 vulnerabilities share the same root cause: **deposit-related routes that should require identity are placed on the public router**. The THB upload and hold/credit routes correctly use `attendee_authed`, but the USDC deposit routes (initiate, TX callback, confirm, webhook) were all public.

### Most Dangerous Attack Vector
**VULN-002** (unauthenticated deposit initiation) combined with the deposit counter allows spamming fake deposits, pushing legitimate deposits beyond `max_refundable_deposits` into the non-refundable tier — causing direct financial harm.

## What Was Fixed (P0 + P1)

### P0 Fixes Applied
1. **VULN-001**: Added Bearer token auth to `deposit_webhook_handler` — now requires `WEBHOOK_SECRET` and `Authorization` header
2. **VULN-002**: Moved `POST /deposit/usdc` from public → `attendee_authed`
3. **VULN-004**: Moved `GET /deposit/usdc/confirm` from public → `attendee_authed`
4. **VULN-008**: Moved `/storage/slips/*` and `/storage/refunds/*` from public → `attendee_authed`

### P1 Fixes Applied
5. **VULN-005**: Both webhook handlers (`deposit_webhook`, `onchain_webhook`) now **reject** all requests when `WEBHOOK_SECRET` is empty (was previously skipping validation)
6. **VULN-006**: Startup guard refuses `DEV_MODE=1` if `redirect_uri` contains production domain patterns

### Remaining Work (P1+P2)
- VULN-007: Replace FNV-1a with cryptographic hash (on-chain program change)
- VULN-009: Add deposit ownership check in hold/rollover flows
- VULN-010: Application-level rate limiting on expensive ops
- VULN-012: Verify `claims.email` matches deposit owner in hold flow
- VULN-011: JWT blacklist via KV on logout
- VULN-014: Scope Google Sheet role fallback to per-event only
- VULN-013: CSP hardening

## Files Modified
- `worker/src/handlers/mod.rs` — Route reorganization
- `worker/src/handlers/deposit/usdc/handlers.rs` — Bearer auth on webhook
- `worker/src/handlers/escrow_index.rs` — Hardened webhook auth
- `worker/src/state.rs` — DEV_MODE guard + webhook secret logging

## How to Test
1. Ensure `WEBHOOK_SECRET` is set in `.dev.vars` (deposit webhook now requires it)
2. `cd worker && cargo test` — 21 tests pass
3. Deposit flow E2E: initiate deposit → should require JWT cookie
4. Webhook: `curl -X POST /api/deposit/usdc/webhook` without Bearer → 401
5. Storage: `curl /api/storage/slips/{event}/{id}` without JWT → 401

## Reflection
The on-chain program and claim flow were already well-hardened. The weaknesses were almost entirely in the off-chain Worker API layer, specifically in deposit/webhook subsystems added in handovers 076–082 without applying the same auth rigor as earlier routes. A pre-deployment checklist item should prevent this pattern.

## Issues Ref
- `.issues/045_security_audit_remediation.md`
