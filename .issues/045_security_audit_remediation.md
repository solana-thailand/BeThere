# Issue 045: Security Audit Remediation

## Status: In Progress

## Summary
Penetration test identified 14 vulnerabilities (2 Critical, 2 High, 5 Medium, 5 Low). P0 and P1 fixes applied.

## Vulnerabilities

### 🔴 Critical (Fixed)
| ID | Title | Fix |
|----|-------|-----|
| VULN-001 | Deposit webhook has zero authentication | Added Bearer token auth to `deposit_webhook_handler` — rejects if `WEBHOOK_SECRET` empty |
| VULN-002 | USDC deposit initiation fully unauthenticated | Moved `POST /deposit/usdc` to `attendee_authed` router |

### 🔴 High (Fixed)
| ID | Title | Fix |
|----|-------|-----|
| VULN-003 | Deposit TX callback unauthenticated | Kept public (Solana Pay needs it), but protected by existing deposit record requirement |
| VULN-004 | Confirm deposit endpoint unauthenticated | Moved `GET /deposit/usdc/confirm` to `attendee_authed` router |

### 🟠 Medium (Fixed)
| ID | Title | Fix |
|----|-------|-----|
| VULN-005 | Webhook secret optional — auth bypass by default | Both webhook handlers now **reject** requests when `WEBHOOK_SECRET` is empty |
| VULN-006 | DEV_MODE token bypass inherits SuperAdmin | Startup guard refuses if `DEV_MODE=1` and redirect_uri contains production domain |
| VULN-008 | R2 storage routes unauthenticated — IDOR for financial docs | Moved `/storage/slips/*` and `/storage/refunds/*` to `attendee_authed` router |

### 🟠 Medium (Remaining)
| ID | Title | Plan |
|----|-------|------|
| VULN-007 | FNV-1a hash for on-chain event ID | Replace with blake3 or SHA-256 (requires on-chain program change) |
| VULN-009 | Rollover deposit allows cross-event fund movement | Add deposit ownership check |
| VULN-010 | No application-level rate limiting | Add per-endpoint rate limiting |
| VULN-012 | Hold deposit uses claims email without ownership check | Verify email matches deposit owner |

### 🟡 Low (Remaining)
| ID | Title |
|----|-------|
| VULN-011 | No JWT revocation/blacklist on logout |
| VULN-013 | CSP allows `unsafe-inline` and `wasm-unsafe-eval` |
| VULN-014 | Google Sheet role grants global event access |

## Files Changed
- `worker/src/handlers/mod.rs` — Route reorganization (VULN-002, 004, 008)
- `worker/src/handlers/deposit/usdc/handlers.rs` — Bearer auth on webhook (VULN-001)
- `worker/src/handlers/escrow_index.rs` — Hardened webhook auth (VULN-005)
- `worker/src/state.rs` — DEV_MODE guard + webhook secret error logging (VULN-005, 006)

## Validation
- `cargo check` — clean
- `cargo clippy` — zero warnings
- `cargo test` — 21/21 pass
