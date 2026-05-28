# Issue #041: Worker Runtime Optimization + File Reorganization

## Summary

Two-phase effort: (1) runtime performance optimization — caching, parallelizing, reducing per-request overhead, and (2) file reorganization — splitting files exceeding the 1024-line guideline.

Performance audit found 6 HIGH, 4 MEDIUM, and 4 LOW impact opportunities. The Worker currently rebuilds the entire Axum router on every request, re-fetches KV/D1 bindings on every request, and has systemic read-modify-write patterns on large KV arrays.

## Phase 1: Runtime Optimization (Quick Wins)

### HIGH Impact

| # | Issue | File | Fix | Est. Savings |
|---|-------|------|-----|-------------|
| H1 | Router rebuilt every request | `lib.rs:63-75` | Cache `Router` in `OnceLock`, rebuild only state-dependent parts | **~1-3ms/req** |
| H2 | KV/D1 bindings fetched every request | `state.rs:147-151` | Cache `quiz_kv`, `events_kv`, `d1` in `OnceLock` alongside config | **3 JS↔WASM crossings/req** |
| H3 | `console_log::init` called every request | `lib.rs:68` | Guard with `OnceLock` | **1 JS interop/req** |
| H4 | Sequential RPC calls in indexer | `escrow_indexer.rs:745-780` | Use `stream::buffered(5)` for parallel `fetch_transaction` | **~80% indexer time** |
| H5 | Sequential Sheets lookups in deposit confirm | `usdc.rs:624-780` | `join!(mapping, attendee, config)` | **200-500ms** |
| H6 | Audit log: read-rewrite 500-item array | `audit_store.rs:148` | Make D1 primary for audit reads, stop KV read-modify-write | **O(n)→O(1) writes** |

### MEDIUM Impact

| # | Issue | File | Fix |
|---|-------|------|-----|
| M1 | `increment_deposit_counter` non-atomic | `event_store.rs:1220-1244` | D1 `UPDATE counter = counter + 1` |
| M2 | On-chain events: full array rewrite per event | `escrow_indexer.rs:462-484` | Store events under individual keys |
| M3 | N+1 KV reads for deposit lists | `event_store.rs:1168-1210` | Batch read or D1 query |
| M4 | `chrono` is ~40KB in WASM | `Cargo.toml:27` | Evaluate `web-time` + manual RFC3339 |

### LOW Impact (Easy Wins)

| # | Issue | File |
|---|-------|------|
| L1 | `correlation_id.clone()` String alloc every request | `middleware.rs:120` |
| L2 | Rate limiter `ip.to_string()` alloc every rate-limited req | `middleware.rs:314` |
| L3 | `extract_client_ip` double-allocation fallback | `middleware.rs:330-346` |
| L4 | `walkin_prefix.clone()` inside pagination loop | `usdc.rs:223` |

## Phase 2: File Reorganization

Split files exceeding 1024-line guideline. Done alongside Phase 1 since we're touching the same files.

### Worker (`worker/src/`)

| File | Lines | Split Into |
|------|------:|-----------|
| `handlers/deposit/escrow.rs` | 1922 | `escrow/{mod,handlers,workflows,types}.rs` |
| `event_store.rs` | 1363 | `event_store/{mod,read,write,schema}.rs` |
| `escrow_indexer.rs` | 1268 | `escrow_indexer/{mod,webhook,poller,store}.rs` |
| `solana_escrow/tx_builders.rs` | 1247 | `tx_builders/{mod,init,deposit,refund,rollover,close}.rs` |
| `handlers/deposit/thb.rs` | 1134 | `thb/{mod,handlers,credit,batch}.rs` |
| `handlers/deposit/usdc.rs` | 1129 | `usdc/{mod,handlers,pay,webhook}.rs` |
| `claim.rs` | 1006 | `claim/{mod,lock,mint}.rs` |
| `middleware.rs` | 398 | `middleware/{mod,headers,rate_limit,correlation,cache}.rs` |
| `http.rs` | ~200 | Move Sheets types to `sheets/` module |

### On-Chain (`bethere-escrow/src/`)

| File | Lines | Split Into |
|------|------:|-----------|
| `tests.rs` | 3776 | `tests/{mod,create_event,deposit,checkin,rollover,refund,close}.rs` |

## Progress

### Phase 1: Runtime Optimization
- [x] H1: Cache app_router in OnceLock
- [x] H2: Cache KV/D1 bindings in OnceLock
- [x] H3: Guard console_log with OnceLock
- [x] H4: Parallelize RPC calls in indexer (stream::buffered(5))
- [x] H5: Parallelize Sheets lookups in deposit confirm (futures_util::join!)
- [ ] H6: D1 primary for audit reads
- [ ] M1: Atomic deposit counter via D1
- [ ] M2: Individual KV keys for on-chain events
- [ ] M3: Batch KV reads for deposit lists
- [ ] L1-L4: Reduce per-request allocations

### Phase 2: File Reorganization
- [x] `tests.rs` split (safest — non-production)
- [x] `escrow.rs` split
- [x] `event_store.rs` split
- [x] `escrow_indexer.rs` split
- [x] `tx_builders.rs` split
- [x] `thb.rs` split
- [x] `usdc.rs` split
- [x] `claim.rs` split
- [x] `middleware.rs` split

## Refs

- Issue #009 (original refactoring — 10/12 done)
- Issue #022 (architecture refactor + perf — Phase 3 complete)
- Issue #039 (Cloudflare platform improvements — R2/Queues/Workflows)
- Performance audit sub-agent analysis (2025-05-28)
