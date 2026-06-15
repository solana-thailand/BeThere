# Security Policy

## Reporting a Vulnerability

**Do not report security vulnerabilities through public GitHub Issues.**

Instead, report them via:

- **Email**: Send to the project maintainer (see repository settings for contact)
- **GitHub**: Use the [private vulnerability reporting](https://github.com/security) feature if enabled

Please include:

1. **Description** of the vulnerability
2. **Affected component** (on-chain program, worker backend, frontend)
3. **Steps to reproduce** or proof of concept
4. **Potential impact** (fund loss, data exposure, access control bypass)
5. **Suggested fix** (if available)

We aim to acknowledge reports within 48 hours and provide a substantive response within 7 days.

## Scope

### In Scope

| Component | Type | Severity |
|-----------|------|----------|
| `bethere-escrow/` | On-chain Solana program (Quasar) | Critical, High, Medium |
| `worker/src/solana_escrow.rs` | TX builders (deposit, refund, claim) | Critical, High, Medium |
| `worker/src/handlers/deposit.rs` | Deposit confirmation, webhook handling | Critical, High |
| `worker/src/auth.rs` | JWT authentication, session management | Critical, High |
| `worker/src/middleware.rs` | Auth guards, security headers | High, Medium |
| Worker secrets handling | `env.secret()`, debug redaction | High |

### Out of Scope

- Social engineering attacks
- Denial of service (Cloudflare Workers handles DDoS mitigation)
- Third-party service vulnerabilities (Google OAuth, Helius RPC)
- Devnet-only issues (test tokens, faucet abuse)
- Issues requiring `DEV_MODE=1` to exploit

## Known Security Findings

The BeThere escrow has undergone an internal security audit with 15 findings (12 fixed, 3 confirmed safe). See [`docs/security_audit.md`](docs/security_audit.md) for full details.

### Force Delete (Devnet Cleanup)

The `DELETE /api/events/{id}/delete` endpoint with `?force=true` allows SuperAdmin to hard-delete events in Draft or Archived status, bypassing the SEC-004 escrow guard. This is intended **only for devnet cleanup** and is gated at the handler level to SuperAdmin role only. When force mode is used, an explicit warning is logged server-side. In normal mode (no `force` param), only Archived events with closed escrows can be hard-deleted.

### Critical / High (Must Fix Before Mainnet)

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| SEC-001 | 🔴 Critical | Check-in gate enables complete fund theft | ✅ Fixed (Phase 3) |
| SEC-002 | 🟠 High | Escrow fields mutable after on-chain init | ✅ Fixed (Phase 1) |
| SEC-012 | 🟠 High | Refund deadline race with claim_forfeited | ✅ Fixed (Phase 5) |

### Medium

| ID | Title | Status |
|----|-------|--------|
| SEC-003 | No maximum deposit cap | ✅ Fixed (Phase 1) |
| SEC-004 | Archive doesn't deactivate on-chain escrow | ✅ Fixed (Phase 1) |
| SEC-005 | Explorer links hardcoded to devnet | ✅ Fixed (Phase 2) |
| SEC-009 | `transfer()` not `transfer_checked()` (Token-2022) | ✅ Fixed (Phase 3) |
| SEC-010 | AttendeeDeposit PDAs never closed (rent leak) | ✅ Fixed (Phase 4) |
| SEC-011 | No `event_end` guard on `mark_checked_in` | ✅ Fixed (Phase 3) |
| SEC-013 | Vault griefing via external USDC airdrop | ✅ Fixed (Phase 5) |
| SEC-014 | No wallet network detection (wrong cluster TX) | ✅ Fixed (Phase 6) |

### Info (Confirmed Safe)

| ID | Title | Status |
|----|-------|--------|
| SEC-007 | Worker cannot manipulate funds (non-custodial) | ✅ Confirmed Safe |
| SEC-008 | On-chain escrow fields immutable after creation | ✅ Confirmed Safe |
| SEC-015 | Stranded lamports on token accounts (never recovered) | ✅ Confirmed Safe |
## Security Audit References

- **Internal audit**: [`docs/security_audit.md`](docs/security_audit.md) — full findings, Solana Foundation checklist cross-reference, Kani formal verification
- **Protocol design**: [`docs/escrow_protocol.md`](docs/escrow_protocol.md)
- **Solana Foundation**: [Security Checklist](https://github.com/solana-foundation/solana-dev-skill/blob/main/skill/references/security.md) — 18 vulnerability categories + program/client checklists
- **Cross-reference**: [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) by Frank Castle (124⭐)
- **Community**: [Solana Audit Arena](https://github.com/Frankcastleauditor/Solana-Audit-Arena) — weekly security competitions
- **Standards**: NIST SP 800-53 AU controls (Audit Logging) — recommended for persistent transaction history
- **Solana safety**: [Solana Safety Guide](https://github.com/Frankcastleauditor/Solana-Safety-Guide) — secure program development practices
- **Payments**: [Solana Foundation Payments & Commerce](https://github.com/solana-foundation/solana-dev-skill/blob/main/skill/references/payments.md) — payment UX/security checklist (6/6 compliant)

### Solana Foundation Security Checklist Compliance

Cross-referenced against the [Solana Foundation Security Checklist](https://github.com/solana-foundation/solana-dev-skill/blob/main/skill/references/security.md). Full mapping in [`docs/security_audit.md`](docs/security_audit.md).

| Category | Status | Notes |
|----------|--------|-------|
| Owner Checks | ✅ Compliant | Quasar framework enforces `owner == program_id` via typed `Account<>` |
| Signer Checks | ✅ Compliant | All mutating instructions require `Signer` + `has_one(organizer/attendee)` |
| Arbitrary CPI Prevention | ✅ Compliant | `Program<TokenProgram>` enforces CPI target is SPL Token |
| Reinitialization Prevention | ✅ Compliant | `init` constraint on PDA accounts; unique seeds prevent reuse |
| PDA Sharing Prevention | ✅ Compliant | Seeds include organizer+event_id (escrow) and event+attendee (deposit) |
| Type Cosplay Prevention | ✅ Compliant | Discriminators 1 (EventEscrow) and 2 (AttendeeDeposit) |
| Duplicate Mutable Accounts | ✅ Compliant | `require_distinct` helper checks all 9 instruction handlers for duplicate addresses |
| Revival / Close Attacks | ✅ Compliant | `close(dest=organizer)` in Quasar; vault closed atomically with escrow |
| Data Matching | ✅ Compliant | `has_one(organizer)` and `address = Seeds(...)` constraints on all accounts |
| Checked Math | ✅ Compliant | All arithmetic uses `checked_add`/`checked_sub` (SEC-009 audit) |
| Token-2022 Compatibility | ✅ Compliant | `transfer_checked()` with mint + decimals (SEC-009 fix) |
| Rent Reclamation | ✅ Compliant | `close_deposit` instruction + GC path (SEC-010 fix) |
| Formal Verification | ✅ Verified | 16 Kani harnesses, 729 checks, all pass |
| Payment UX/Security | ✅ Compliant | Custom Solana Pay flow (not Commerce Kit) — 6/6 checks pass. Deposits go to PDA escrow, not merchant wallet |

## Bug Bounty

No formal bug bounty program is currently active. This may change after the mainnet deployment. Security researchers who submit valid findings will be credited in the security audit document.

## Disclosure Policy

- **Coordinated disclosure**: We ask that vulnerabilities be kept confidential until a fix is deployed.
- **Timeline**: We aim to deploy fixes for Critical/High findings within 30 days of confirmation.
- **Credit**: Researchers who report valid vulnerabilities will be credited (with permission) in audit documentation.
