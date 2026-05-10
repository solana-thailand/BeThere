# 011 — Sustainability & Green Software Engineering

## Summary

Holistic sustainability audit across energy, data lifecycle, network, on-chain, financial, and operational dimensions. Covers optimization of WASM binaries, KV data hygiene, RPC caching, on-chain account space, and a self-funding revenue model.

## Motivation

The platform currently:
- Compiles WASM with zero optimization (2-5x larger than necessary)
- Stores event data in KV forever (no TTL, no cleanup)
- Makes redundant Solana RPC calls (no blockhash caching)
- Stores 56% redundant bytes on-chain (locked SOL as rent)
- Earns $0 revenue (no platform fee mechanism)

## Findings by Category

### 1. ENERGY — WASM Binary Bloat 🔴

**Both Worker and Frontend Cargo.toml have no `[profile.release]` section.**

Estimated impact:
- Worker WASM: 2-5x larger than necessary → more CPU on every cold start
- Frontend WASM: 3-6 MB unoptimized → 500 KB-1.5 MB optimized (75%+ reduction)

**Fix**: Add to both `Cargo.toml`:
```toml
[profile.release]
codegen-units = 1
lto = true
opt-level = "z"
strip = true
panic = "abort"
```

**Also**:
- Gate `console_log` behind feature flag in frontend (strip debug strings from prod)
- Add `wasm-opt -Oz` to Trunk.toml `[tools]` section
- `validate_wallet_address` allocates `HashSet<char>` on every call — replace with `const` lookup table

**Files**: `worker/Cargo.toml`, `frontend-leptos/Cargo.toml`, `worker/src/solana.rs#L170-183`

### 2. DATA LIFECYCLE — No TTLs, No Cleanup 🔴

**6 categories of KV entries persist forever with no expiration:**

| Key Pattern | Current TTL | Should Be |
|---|---|---|
| `event:{id}` (EventConfig) | ♾️ None | `event_end + 1 year` |
| `event:{id}:quiz:progress:{token}` | ♾️ None | `event_end + 30 days` |
| `event:{id}:adventure:progress:{token}` | ♾️ None | `event_end + 30 days` |
| `event:{id}:deposit:status:{id}` | ♾️ None | `event_end + refund_deadline + 90 days` |
| `event:{id}:deposit:thb:{id}` | ♾️ None | `event_end + refund_deadline + 90 days` |
| `event:{id}:claim:lock:{token}` (finalized) | ♾️ None | `event_end + 90 days` |

**Already good**: Attendee cache (300s TTL), Google OAuth token (3500s TTL), Staff cache (60s TTL)

**Fix**:
1. Add `.expiration_ttl()` to all per-attendee KV writes
2. Add cron-triggered cleanup worker (`[triggers] crons = ["0 3 * * *"]`)
3. Archive should cascade-delete or TTL-expire associated keys

**Files**: `event_store.rs`, `quiz.rs`, `adventure.rs`, `claim.rs`, `wrangler.toml`

### 3. NETWORK — Redundant RPC Calls 🟡

| Issue | Impact | Fix |
|---|---|---|
| `getLatestBlockhash` called fresh on every TX build | N identical calls for N concurrent deposits | Cache in KV with 30s TTL |
| `confirm_deposit_handler` polls RPC on every request | 15 RPC calls per deposit (2s × 30s) | Cache confirmed result, return `retry_after_ms` |
| `backfill_wallets_handler` makes N sequential RPC calls | 100 deposits = 100 sequential `getTransaction` | Batch with concurrent futures |
| Waitlist: uncached token + sheet fetch | 2 network round trips per signup | Use cached access token + KV cache |
| No pagination on attendee list API | 200KB+ per response for large events | Add `?page=&limit=` params |

**Files**: `solana_escrow.rs#L470-520`, `deposit.rs#L337-436`, `deposit.rs#L1507-1560`, `waitlist.rs#L86-102`

### 4. ON-CHAIN — Account Space Bloat 🟡

> ⚠️ **BREAKING CHANGE** — modifying account layout requires program redeployment + re-testing. Do this AFTER deposit/refund feature is finalized on mainnet.

| Account | Current | Minimum | Waste |
|---|---|---|---|
| `EventEscrow` | 150 bytes | 61 bytes | 59% (89 bytes) |
| `AttendeeDeposit` | 87 bytes | 38 bytes | 56% (49 bytes) |

**Redundant fields:**
- `usdc_mint` in EventEscrow — never read on-chain (32 bytes)
- `vault` in EventEscrow — derivable from ATA seeds (32 bytes)
- `event` in AttendeeDeposit — already in PDA seeds (32 bytes)
- `amount` in AttendeeDeposit — always equals `deposit_amount` (8 bytes)
- `deposited_at` + `Clock::get()` — never validated (8 bytes + 100 CU)
- `is_active` — never set to false (1 byte)
- `total_deposited` + `total_refunded` — derivable off-chain (16 bytes)

**At 10K events/year, 50 attendees avg**: ~$680K in unnecessarily locked rent.

**Files**: `bethere-escrow/src/state.rs`, all instruction files

### 5. COMPUTE — Worker Code Efficiency 🟡

| Issue | Fix |
|---|---|
| Full attendee list fetched for single-row lookups (`sheets.rs#L274-299`) | Build `HashMap` after deserialize |
| `execute_claim` reads attendees twice | Reuse result within request context |
| `list_events` loads full configs for access check | Include `staff_emails` in `EventMeta` |

### 6. FRONTEND — Asset & Render Waste 🟡

| Issue | Waste | Fix |
|---|---|---|
| Google Fonts: 5 weights externally | ~150 KB + 2 blocking DNS | Self-host, reduce to 3 weights |
| jsQR + QRious loaded unconditionally | ~49 KB per page | Lazy-load on Scanner/Claim pages |
| `console_log` Level::Debug in prod | String bloat in WASM | Feature-gate behind `debug-log` |
| QR Scanner dual polling (Rust + JS) | CPU waste | Single loop with callback |
| SessionTimer 1s interval during Live | Unnecessary re-renders | 5s interval |
| No API response caching | Re-downloads on every nav | In-memory cache with TTL |
| Inline `style=` with `format!()` | New allocation per render | Use CSS classes |

**Files**: `index.html#L73-84`, `Cargo.toml`, `scanner.rs#L240-245`, `claim.rs#L183-205`, `api.rs`

### 7. FINANCIAL — Self-Funding Loop 🟡

> ⚠️ **DEPENDS ON DEPOSIT/REFUND FEATURE** — implement platform fee after mainnet escrow is stable.

Current state: Platform earns **$0**. No fee mechanism exists anywhere.

**Recommended model:**
1. **Platform fee on forfeited deposits** (10% of forfeits → platform treasury)
2. **SOL subsidy from forfeit revenue** (self-funding: forfeits → SOL for future attendees)
3. **Tiered organizer plans** (Free/Pro/Enterprise with different fee percentages)

```
Forfeited deposits → 10% platform fee → funds SOL subsidy → more attendees deposit → more events → flywheel
```

**Implementation**: Add `platform_fee_bps: u16` + `platform_wallet: Address` to `EventEscrow`, modify `claim_forfeited` to split.

## Implementation Phases

### Phase A — Non-Breaking ✅ Complete

| # | Item | Category | Effort | Status |
|---|------|----------|--------|--------|
| A1 | `[profile.release]` on Worker + Frontend | Energy | 15 min | ✅ Done (prior session) |
| A2 | TTLs on all per-attendee KV entries | Data Lifecycle | 1-2h | ✅ Done (prior session) |
| A3 | Cron cleanup worker | Data Lifecycle | 2-3h | ✅ Done |
| A4 | Cache RPC blockhash (30s TTL) | Network | 1h | ✅ Done (prior session) |
| A5 | `const` base58 validation | Energy | 30 min | ✅ Done (prior session) |
| A6 | Lazy-load jsQR/QRious | Network | 30 min | ✅ Done |
| A7 | Gate `console_log` behind feature flag | Energy | 30 min | ✅ Done |
| A8 | Reduce SessionTimer to 5s interval | Energy | 15 min | ✅ Done |

**Total: ~5-7h — all complete**

### Phase B — After Deposit/Refund Mainnet Stable 🟡

| # | Item | Category | Effort |
|---|------|----------|--------|
| B1 | Remove redundant on-chain fields | On-chain | 3-4h |
| B2 | Platform fee on `claim_forfeited` | Financial | 3h |
| B3 | SOL subsidy from forfeit revenue | Financial | 3h |
| B4 | Tiered organizer plans (KV + Stripe) | Financial | 1-2 days |
| B5 | Frontend API response caching | Network | 1h |
| B6 | Attendee list pagination | Network | 1h |
| B7 | HashMap for attendee lookups | Compute | 1h |

**Total: ~3-5 days**

## Green Software Principles Applied

1. **Energy Efficiency** — Smaller WASM binaries, fewer CPU cycles, zero-alloc validation
2. **Carbon Awareness** — Serverless Workers = shared compute (no idle servers), Solana PoS = ~0.0000025 kWh/TX
3. **Hardware Efficiency** — Edge deployment (Cloudflare 300+ PoPs), minimal client-side footprint
4. **Data Lifecycle** — TTLs on all transient data, cron cleanup, no unbounded growth
5. **Network Efficiency** — Cached RPC calls, lazy-loaded assets, paginated APIs
6. **Financial Sustainability** — Self-funding flywheel from forfeited deposits

## Refs

- [Green Software Foundation](https://greensoftware.foundation/)
- [Software Carbon Intensity Spec](https://github.com/Green-Software-Foundation/software_carbon_intensity)
- [Cloudflare Workers Carbon Report](https://blog.cloudflare.com/understand-and-reducing-your-carbon-footprint-on-cloudflare-workers/)
- Solana Energy Report: ~0.0000025 kWh per transaction (Proof of Stake)
- Issue 010: Deposit/Refund Escrow (current deposit/refund feature)

## Status

✅ Phase A complete — all 8 items implemented and verified (39/39 tests pass).
🔴 Phase B blocked on Issue 010 Phase 5 (mainnet deployment).

### Phase A Commits

| Commit | Items |
|--------|-------|
| (prior session) | A1: WASM profile release, A2: KV TTLs, A4: RPC blockhash cache, A5: const base58 |
| `b135b73` | A6: Lazy-load QR libraries, A7: Feature-gate console_log |
| `c1571eb` | A8: SessionTimer 5s interval |
| `85cd2af` | A3: Cron cleanup worker |
