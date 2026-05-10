# Handover 039: Full-Stack Sustainability & Green Software Audit

## What Happened

Continued from handover 038 (wallet backfill + devnet staging). This session performed a comprehensive sustainability audit across the entire BeThere platform — Worker, on-chain escrow program, and Leptos frontend.

The user asked about sustainability in a holistic sense: green software coding, energy efficiency, data lifecycle, financial sustainability, and long-term maintainability — not just "how to make money."

Three parallel audits were conducted:
1. **Worker audit** — WASM optimization, KV usage, RPC patterns, data lifecycle
2. **On-chain audit** — Account space efficiency, compute units, rent waste
3. **Frontend audit** — WASM binary, asset loading, network patterns, render efficiency

Additionally, all pyright/pylinter diagnostics in utility Python scripts (`sign_and_submit.py`, `decode_tx.py`) and JS (`solana_wallet.js`) were fixed — 0 errors, remaining warnings are pyright strict-mode noise on `urllib`/`json` dynamic types.

## Changes Made

### Diagnostics Fix (committed changes)

| File | Changes |
|------|---------|
| `scripts/e2e/sign_and_submit.py` | Moved `import nacl.signing` to top, added type annotations, fixed `pyright: ignore` rule name |
| `scripts/e2e/decode_tx.py` | Separated imports, added type annotations, fixed f-string without placeholder, added `from typing import Any` |
| `frontend-leptos/js/solana_wallet.js` | Changed unused catch `e` → `_e` in `getConnectedPublicKey` |

### Documentation Created

| File | Description |
|------|-------------|
| `.issues/011_sustainability_green_software.md` | Full sustainability audit with 7 categories, phased implementation plan |

## Key Findings

### 🔴 Energy — WASM Binary Bloat
Both Worker and Frontend compile with zero `[profile.release]` optimization. Estimated 75%+ size reduction possible with LTO + `opt-level = "z"` + strip.

### 🔴 Data Lifecycle — No TTLs
6 categories of per-event KV data persist forever. No cleanup cron exists. At scale this creates unbounded storage growth.

### 🔴 Network — Redundant RPC
`getLatestBlockhash` called fresh on every TX build (should cache 30s). Deposit confirmation polling makes 15 RPC calls per deposit.

### 🟡 On-Chain — 56% Account Waste
`EventEscrow` is 150 bytes but only needs 61 bytes. `AttendeeDeposit` is 87 bytes but only needs 38 bytes. Fields like `usdc_mint`, `vault`, `event`, `amount` are redundant.

### 🟡 Financial — $0 Revenue
No platform fee mechanism exists. Forfeited deposits go 100% to organizer. Recommended: 10% platform fee on forfeits → funds SOL subsidy → self-funding flywheel.

## Critical Decision: What to Do First

The user asked whether to finish deposit/refund feature before sustainability work. Answer:

**Phase A (non-breaking, do now):**
- `[profile.release]` optimizations (15 min, massive impact)
- TTLs on KV entries (1-2h)
- RPC blockhash caching (1h)
- `const` base58 validation (30 min)
- Lazy-load JS assets (30 min)

These don't touch the escrow program or deposit/refund flow — safe to implement in parallel.

**Phase B (breaking, do after mainnet):**
- Remove redundant on-chain fields (changes account layout = program redeploy)
- Platform fee on `claim_forfeited` (extends escrow program)
- SOL subsidy from forfeit revenue (extends deposit flow)

These change the escrow program and would invalidate the E2E devnet test. Must wait until deposit/refund is mainnet-stable.

## Issues Ref

- Issue 011: Sustainability & Green Software Engineering (NEW)

## How to Dev/Test

Phase A items are all non-breaking:
```bash
# 1. Worker WASM optimization
# Edit worker/Cargo.toml, add [profile.release] section
cargo check --target wasm32-unknown-unknown -p event-checkin-worker
cargo test -p event-checkin-worker

# 2. Frontend WASM optimization
# Edit frontend-leptos/Cargo.toml, add [profile.release] section
cd frontend-leptos && ~/.cargo/bin/trunk build

# 3. KV TTL changes — add .expiration_ttl() to writes
# Test by creating event, waiting for TTL, verifying key is gone

# 4. RPC blockhash cache — test with concurrent deposit requests
```

## Reflection

The sustainability audit revealed that the platform is architecturally sound (serverless Workers, Solana PoS, edge caching) but has **low-hanging fruit** in WASM optimization and data lifecycle. The `[profile.release]` fix alone saves 75%+ on every page load and cold start — it's a no-brainer.

The on-chain account waste (56%) is significant at scale but requires a breaking change. It's better to let the escrow program stabilize on mainnet first, then optimize in a v2 upgrade.

The financial sustainability model (forfeit fee → SOL subsidy) creates a natural flywheel that makes the platform self-funding. This should be the next major feature after mainnet deployment.

## Remaining Work

### Phase A — Non-Breaking ✅ ALL COMPLETE

| # | Item | Status | Commit |
|---|------|--------|--------|
| A1 | `[profile.release]` on Worker + Frontend | ✅ Done | (prior session) |
| A2 | TTLs on all per-attendee KV entries | ✅ Done | (prior session) |
| A3 | Cron cleanup worker | ✅ Done | `85cd2af` |
| A4 | Cache RPC blockhash (30s TTL) | ✅ Done | (prior session) |
| A5 | `const` base58 validation | ✅ Done | (prior session) |
| A6 | Lazy-load jsQR/QRious | ✅ Done | `b135b73` |
| A7 | Gate `console_log` behind feature flag | ✅ Done | `b135b73` |
| A8 | Reduce SessionTimer to 5s interval | ✅ Done | `c1571eb` |

### Phase B — After Deposit/Refund Mainnet Stable
| # | Item | Effort |
|---|------|--------|
| B1 | Remove redundant on-chain fields | 3-4h |
| B2 | Platform fee on `claim_forfeited` | 3h |
| B3 | SOL subsidy from forfeit revenue | 3h |

### From Issue 010 (Deposit/Refund)
- Phase 5: Mainnet deployment (security review + deploy)
