# 022 — Architecture Refactor + Performance Optimization

## Summary

Two-phase effort: (1) split monolithic files into focused submodules, (2) DRY Solana TX builder boilerplate, (3) performance optimization pass.

## Phase 1: File Splitting (Complete)

| Before | Lines | After | Lines |
|--------|------:|-------|------:|
| `handlers/deposit.rs` | 2554 | `deposit/{mod,usdc,thb,escrow}.rs` | 54+734+509+1355 |
| `solana_escrow.rs` | 2440 | `solana_escrow/{mod,crypto,wire,tx_builders}.rs` | 178+348+434+1055 |
| `sheets.rs` | 1119 | `sheets/{mod,write}.rs` | 640+498 |

## Phase 2: TX Builder DRY (Complete)

Introduced `EscrowCtx` (shared program IDs + PDA resolution), `acct_sw/acct_w/acct_r` (compact AccountMeta), `finalize_tx` (single serialization pipeline).

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| `tx_builders.rs` lines | 1533 | 1055 | **-31%** |
| `pubkey_from_base58` calls | 78 | ~25 | **-68%** |
| `AccountMeta {}` blocks | 69 | 0 | **-100%** |
| `serialize_transaction` calls | 8 | 1 | **-88%** |

## Phase 3: Performance Optimization (Complete ✅)

### HIGH IMPACT — Latency

| # | Finding | File | Est. Savings | Status |
|---|---------|------|-------------|--------|
| H1 | Walk-in Sheets sync blocks response — use `wait_until` | `walkin.rs#L236-298` | **500-1500ms** | ✅ |
| H2 | Claim lookup fetches ALL attendees to find one by token | `claim.rs#L270-290` | **50-2000ms** | ✅ |
| H3 | Full KV scan to count deposits (USDC + THB) | `usdc.rs#L134`, `thb.rs#L78` | **100-300ms** | ✅ |
| H4 | `my_registrations` N sequential config + attendee loads | `register.rs#L435-520` | **200-1000ms** | ✅ |
| H5 | `cancel_status` two sequential KV scans → `join!` | `escrow.rs#L1308-1327` | **50-200ms** | ✅ |
| H6 | `backfill_wallets` sequential RPC → parallel | `escrow.rs#L528-653` | **~80% time** | ✅ |
| H7 | `resolve_event_by_escrow` scans ALL configs → reverse KV index | `escrow_indexer.rs` | **~180ms→~5ms** | ✅ |
| H8 | Deposit webhook verifies on-chain synchronously → detached | `usdc.rs` | **100-500ms** | ✅ |

### MEDIUM IMPACT — Bandwidth

| # | Finding | File | Est. Savings | Status |
|---|---------|------|-------------|--------|
| B1 | `list_attendees` no pagination → cursor-based | `attendee.rs`, `ext.rs` | **~80% bytes** | ✅ |
| B2 | Sheets API reads `A2:Z` when ~18 cols needed | `sheets/mod.rs#L346` | **~30-40%** | ✅ |
| B3 | Attendee list response includes `claim_token` per row → slim DTO | `api.rs`, `attendee.rs` | **~1 field/row** | ✅ |
| B4 | No `Cache-Control` on public endpoints | `public_event.rs` | **~100% cached** | ✅ |
| B5 | QR base64 SVG re-generated per request → KV cached (1h TTL) | `attendee.rs` | **2-5KB+CPU** | ✅ |

### LOW IMPACT — Correctness

| # | Finding | File | Details | Status |
|---|---------|------|---------|--------|
| C1 | Claim lock TOCTOU race (double-mint risk) | `claim.rs#L37-73` | Correctness | ✅ |
| C2 | `get_column_mapping` fetched 2-3x per claim → cached once | `claim.rs#L396-718` | Eliminates 1-2 KV | ✅ |
| C3 | `AppState::from_env` called every request → `OnceLock` cached | `state.rs` | ~1-3ms | ✅ |
| C4 | `is_staff` linear scan → `HashSet` O(1) | `state.rs`, `escrow_index.rs` | O(n)→O(1) | ✅ |
| C5 | Quiz + adventure KV checks sequential → `join!` | `claim.rs#L493-543` | 5-20ms | ✅ |
| C6 | RPC URL format repeated 12+ times → `full_rpc_url()` | `deposit/escrow.rs`, `usdc.rs` | Cleanliness | ✅ |

## Commits

| SHA | Description |
|-----|-------------|
| `23a84ce` | refactor: split monoliths + DRY tx builders (Phase 1–2) |
| `a2b8dac` | feat: performance optimizations H5, C5, C6, B4 (Phase 3 initial) |
| `7369f06` | feat: performance optimizations H1, H3, B2 (Phase 3 batch 1) |
| `20ae34b` | feat: correctness fix C1 + performance H2, H4, H6 (Phase 3 batch 2) |
| _(pending)_ | feat: performance C4, C2, H8, H7 (Phase 3 batch 3) |
| _(pending)_ | feat: performance B3, B5, C3, B1 (Phase 3 batch 4 — final) |

## Status

✅ **All 19 items complete.** Phase 1, 2, and 3 all done. 73 tests passing, zero clippy warnings.
