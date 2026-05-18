# 064 — Architecture Refactor + Performance Optimization

## What Happened

Continued from conversation thread "Event Checkin Code Architecture Assessment". Three-phase effort, now fully complete.

### Phase 1-2: File Splitting + TX Builder DRY (Committed as `23a84ce`)
- Split 3 monolithic files (6113 lines) into 10 focused submodules
- Introduced `EscrowCtx`, `acct_sw/acct_w/acct_r`, `finalize_tx`
- 47 tests passing, zero diagnostics

### Phase 3: Performance Optimization — 19 items across 4 batches

#### Batch 1 (committed `a2b8dac`): H5, C5, C6, B4
- H5: `cancel_status` two KV scans → `futures::join!`
- C5: Quiz + adventure KV checks → `futures::join!`
- C6: `SolanaConfig::full_rpc_url()` replaces 12 `format!` blocks
- B4: `Cache-Control` middleware on public endpoints (60s/120s), `no-store` on auth

#### Batch 2 (committed `7369f06`): H1, H3, B2
- H1: Walk-in Sheets sync blocks response → detached via `wait_until`
- H3: Full KV scan to count deposits → counter key `deposit_count:{currency}`
- B2: Sheets API reads `A2:Z` → precise column range via column mapping

#### Batch 3 (committed `20ae34b`): C1, H2, H4, H6
- C1: Claim lock TOCTOU race → KV-based lock with atomic check-and-set
- H2: Claim lookup fetches ALL attendees → KV index `claim_token:{token}` → `api_id`
- H4: `my_registrations` N sequential loads → `futures::join!` parallel
- H6: `backfill_wallets` sequential RPC → parallel batch with `futures::stream`

#### Batch 4 (uncommitted — batch 3+4 pending): C4, C2, H8, H7, B3, B5, C3, B1
- C4: `is_staff` linear scan → `HashSet<String>` O(1) lookup
- C2: `get_column_mapping` fetched 2-3× per claim → cached once early
- H8: Deposit webhook verifies on-chain synchronously → detached via `wait_until`
- H7: `resolve_event_by_escrow` scans ALL configs → reverse KV index `escrow:{address}`
- B3: Attendee list returns full `AttendeeResponse` (11 fields) → slim `AttendeeListItem` (10 fields, no `claim_token`)
- B5: QR base64 SVG re-generated per request → KV cached with 1h TTL (`qr:{api_id}`)
- C3: `AppState::from_env` called every request → `OnceLock<Arc<AppConfig>>` cached once per isolate
- B1: `list_attendees` no pagination → cursor-based (`row_index` cursor, configurable limit)

## Plan/Code/Test

- Issue: `.issues/022_architecture_refactor_perf_optimization.md`
- Implementation: 14 files changed across domain, worker, and frontend-leptos crates
- Tests: 73 tests passing (26 domain + 47 worker), zero clippy warnings

## Reflection

- Batch 3+4 items were split across two conversation threads due to context length
- The `OnceLock` approach for C3 is clean — only `Arc::clone` per request, config built once
- The QR KV caching (B5) uses best-effort semantics — failures degrade gracefully to fresh generation
- Cursor-based pagination (B1) defaults to limit=200 to avoid breaking the admin dashboard's client-side filtering; the API is ready for consumers that want true pagination
- AttendeeListItem (B3) drops only `claim_token` — a modest reduction but the pattern establishes a clear list-vs-detail DTO boundary

## Remain Work

- ✅ All 19 Phase 3 items complete
- No remaining work for issue 022

## Issues Ref

- `.issues/022_architecture_refactor_perf_optimization.md`

## How to Dev/Test

```bash
# Build check
cargo check --workspace

# Lint
cargo clippy --workspace --all-targets

# Test all
cargo test --workspace

# Test worker only
cargo test -p event-checkin-worker
```
