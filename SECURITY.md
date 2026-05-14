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

The BeThere escrow has undergone an internal security audit with 11 findings. See [`docs/security_audit.md`](docs/security_audit.md) for full details.

### Force Delete (Devnet Cleanup)

The `DELETE /api/events/{id}/delete` endpoint with `?force=true` allows SuperAdmin to hard-delete events in Draft or Archived status, bypassing the SEC-004 escrow guard. This is intended **only for devnet cleanup** and is gated at the handler level to SuperAdmin role only. When force mode is used, an explicit warning is logged server-side. In normal mode (no `force` param), only Archived events with closed escrows can be hard-deleted.

### Critical / High (Must Fix Before Mainnet)

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| SEC-001 | 🔴 Critical | Check-in gate enables complete fund theft | ✅ Fixed (Phase 3) |
| SEC-002 | 🟠 High | Escrow fields mutable after on-chain init | ✅ Fixed (Phase 1) |

### Medium (Should Fix Before Mainnet)

| ID | Title | Status |
|----|-------|--------|
| SEC-003 | No maximum deposit cap | ✅ Fixed (Phase 1) |
| SEC-004 | Archive doesn't deactivate on-chain escrow | ✅ Fixed (Phase 1) |
| SEC-005 | Explorer links hardcoded to devnet | ✅ Fixed (Phase 2) |
| SEC-009 | `transfer()` not `transfer_checked()` (Token-2022) | ✅ Fixed (Phase 3) |
| SEC-010 | AttendeeDeposit PDAs never closed (rent leak) | ✅ Fixed (Phase 4) |
| SEC-011 | No `event_end` guard on `mark_checked_in` | ✅ Fixed (Phase 3) |

## Security Audit References

- **Internal audit**: [`docs/security_audit.md`](docs/security_audit.md)
- **Protocol design**: [`docs/escrow_protocol.md`](docs/escrow_protocol.md)
- **Cross-reference**: [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) by Frank Castle
- **Community**: [Solana Audit Arena](https://github.com/Frankcastleauditor/Solana-Audit-Arena) — weekly security competitions
- **Standards**: NIST SP 800-53 AU controls (Audit Logging) — recommended for persistent transaction history
- **Solana safety**: [Solana Safety Guide](https://github.com/Frankcastleauditor/Solana-Safety-Guide) — secure program development practices

## Bug Bounty

No formal bug bounty program is currently active. This may change after the mainnet deployment. Security researchers who submit valid findings will be credited in the security audit document.

## Disclosure Policy

- **Coordinated disclosure**: We ask that vulnerabilities be kept confidential until a fix is deployed.
- **Timeline**: We aim to deploy fixes for Critical/High findings within 30 days of confirmation.
- **Credit**: Researchers who report valid vulnerabilities will be credited (with permission) in audit documentation.
