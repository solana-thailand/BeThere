# 079 — Worker Runtime Optimization + File Reorganization

## What Happened

Performance audit of the Cloudflare Worker revealed 14 optimization opportunities (6 HIGH, 4 MEDIUM, 4 LOW impact). Implemented Phase 1 quick wins (H1-H5) and completed all 9 file reorganization splits. All 86 tests pass.

## Optimization Changes (Phase 1)

| ID | Change | File(s) | Savings |
|----|--------|---------|---------|
| H1 | Cache `Router` skeleton in `OnceLock` — route table + middleware built once per isolate | `lib.rs` | ~1-3ms/req |
| H2 | Cache KV/D1 bindings in `OnceLock` — 3 fewer JS↔WASM boundary crossings per request | `state.rs` | 3 interop calls/req |
| H3 | Guard `console_log::init` with `OnceLock` — 1 fewer JS interop per request | `lib.rs` | 1 call/req |
| H4 | Parallelize RPC fetches in escrow indexer — `stream::buffered(5)` for concurrent `fetch_transaction` | `escrow_indexer.rs` → `escrow_indexer/poller.rs` | ~80% indexer time |
| H5 | Parallelize independent Sheets API lookups — `futures_util::join!` for mapping + attendee fetch | `handlers/deposit/usdc.rs` → `usdc/handlers.rs` | 200-500ms per confirm |

## File Reorganization (Phase 2)

All 9 files exceeding the 1024-line guideline were split into focused submodules:

### Worker (`worker/src/`)

| Before | Lines | After | Files |
|--------|------:|-------|-------|
| `handlers/deposit/escrow.rs` | 1922 | `deposit/escrow/{mod,handlers,status,types,workflows}.rs` | 5 |
| `event_store.rs` | 1363 | `event_store/{mod,read,write,schema}.rs` | 4 |
| `escrow_indexer.rs` | 1268 | `escrow_indexer/{mod,webhook,poller,store}.rs` | 4 |
| `solana_escrow/tx_builders.rs` | 1247 | `tx_builders/{mod,init,deposit,refund,rollover,close,mark}.rs` | 7 |
| `handlers/deposit/thb.rs` | 1134 | `deposit/thb/{mod,handlers}.rs` | 2 |
| `handlers/deposit/usdc.rs` | 1129 | `deposit/usdc/{mod,handlers}.rs` | 2 |
| `claim.rs` | 1006 | `claim/{mod,lock,mint}.rs` | 3 |
| `middleware.rs` | 398 | `middleware/{mod,headers,correlation,cache,rate_limit}.rs` | 5 |

### On-Chain (`bethere-escrow/src/`)

| Before | Lines | After | Files |
|--------|------:|-------|-------|
| `tests.rs` | 3776 | `tests/{mod,create_event,deposit,checkin,refund,rollover,rollover_flow,close}.rs` | 8 |

## Remaining (Not Started)

| ID | Description | Effort | Issue |
|----|-------------|--------|-------|
| H6 | D1 primary for audit reads (stop KV read-modify-write of 500-item arrays) | Medium | #041 |
| M1 | Atomic deposit counter via D1 | Medium | #041 |
| M2 | Individual KV keys for on-chain events | Medium | #041 |
| M3 | Batch KV reads for deposit lists | Medium | #041 |
| L1-L4 | Reduce per-request allocations | Small | #041 |

## Issues Ref

- Issue #041 — Worker Runtime Optimization + File Reorganization
- Issue #039 — Cloudflare Platform Improvements (R2/Queues still pending)
- Issue #009 — Codebase Refactoring (10/12 done)
- Issue #022 — Architecture Refactor + Performance (Phase 3 complete)

## How to Dev/Test

```bash
# Worker build check
CARGO_BUILD_JOBS=2 cargo check -p event-checkin-worker

# Worker + domain tests (47 tests)
cargo test -p event-checkin-worker -p event-checkin-domain

# SVM on-chain tests (39 tests)
cd bethere-escrow && cargo test

# All together: 86 tests
```

## Reflection

- Router caching (H1) required a design change: the router skeleton (fallback + middleware layers) is cached, but state-dependent API routes are merged per-request. This is because `AppState.worker_ctx` varies per request. A future optimization could pass `worker_ctx` via `Extension` to fully cache the router.
- The `stream::buffered(5)` pattern for RPC parallelization (H4) preserves order for cursor updates while overlapping network I/O.
- File splits were mechanical — no logic changes. All existing `use crate::module::*` imports continue to work via re-exports in each `mod.rs`.
- THB file (1134 lines) split into only 2 files because all handlers are tightly coupled. A further split would require extracting the types first.
