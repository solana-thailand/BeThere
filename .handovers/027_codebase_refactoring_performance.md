# 027 — Codebase Refactoring & Performance Improvements

## What Happened

Full codebase review (~22K lines) followed by implementation of structural, performance, and maintainability improvements. Two performance-critical KV caches added, handler DRY violations eliminated, config reorganized, and typed error infrastructure laid.

## Branch

- Based on `main` at latest commit

## Changes Summary

### P0 — Performance (Latency Reduction)

| Change | Before | After | Improvement |
|--------|--------|-------|-------------|
| Google access token KV cache | RSA-JWT signed every request (~100ms) | Cached in KV for 3500s | ~100ms/request saved |
| Attendee data KV cache | Full Sheets API scan every request (~200-800ms) | Cached in KV for 30s | ~200-800ms/request saved |
| Cache invalidation on write | N/A | Mutations invalidate cache immediately | Stale data max 30s |

**Net effect:** Check-in latency expected to drop from 500-2000ms to 50-200ms for cached reads.

### P1 — Structural (Maintainability)

| Change | Files | Lines Removed |
|--------|-------|---------------|
| Shared `EventIdQuery` struct | 4 handlers → 1 `ext.rs` | ~40 lines |
| Shared `resolve_event_with_access()` | 6 handlers → 1 function | ~120 lines |
| Shared `resolve_event()` | 5 handlers → 1 function | ~80 lines |
| Shared `resolve_kv()` helper | 8 call sites → 1 function | ~24 lines |

### P1 — Type Safety (Foundation)

| Change | Purpose |
|--------|---------|
| `AppError` enum (domain) | 7 variants: NotFound, Unauthorized, Forbidden, Validation, External, RateLimited, Internal |
| `WorkerError` newtype (worker) | Axum `IntoResponse` bridge for `AppError` |
| `IntoAppError` trait | Ergonomic `Result<T, String>` → `Result<T, AppError>` conversion |

### P2 — Config Reorganization

| Change | Before | After |
|--------|--------|-------|
| `AppConfig` fields | 22 flat fields | 10 fields in 5 sub-configs |
| Sub-configs | N/A | `SolanaConfig`, `NftConfig`, `ServerConfig`, `EventDefaults` |
| Event fields in global config | 5 fields mixed in | Isolated in `EventDefaults` |

### P2 — API Cleanliness

| Change | Before | After |
|--------|--------|-------|
| `mint_compressed_nft` params | 12 positional `&str` params | 1 `&MintRequest` struct |
| `#[allow(clippy::too_many_arguments)]` | Required | Removed |

## Files Changed

| File | Change |
|------|--------|
| `domain/src/models/error.rs` | **NEW** — `AppError` enum, `IntoAppError` trait |
| `domain/src/models/mod.rs` | Added `pub mod error` |
| `domain/src/config/types.rs` | Added `SolanaConfig`, `NftConfig`, `ServerConfig`, `EventDefaults`; reorganized `AppConfig` |
| `domain/src/config/mod.rs` | Exported new sub-config types |
| `worker/src/error.rs` | **NEW** — `WorkerError` newtype with `IntoResponse` |
| `worker/src/lib.rs` | Added `mod error` |
| `worker/src/handlers/ext.rs` | **NEW** — `EventIdQuery`, `resolve_event_with_access`, `resolve_event`, `resolve_kv` |
| `worker/src/handlers/mod.rs` | Added `pub mod ext` |
| `worker/src/handlers/checkin.rs` | Removed local `EventIdQuery`, use `resolve_event_with_access`, `resolve_kv` |
| `worker/src/handlers/attendee.rs` | Same consolidation |
| `worker/src/handlers/claim.rs` | Same consolidation + `MintRequest` struct usage |
| `worker/src/handlers/qr.rs` | Same consolidation |
| `worker/src/handlers/quiz.rs` | Same consolidation |
| `worker/src/handlers/adventure.rs` | Same consolidation |
| `worker/src/sheets.rs` | Google token KV cache, attendee KV cache, cache invalidation |
| `worker/src/solana.rs` | `MintRequest` struct, refactored `mint_compressed_nft` |
| `worker/src/state.rs` | Construct sub-config structs in `from_env` |
| `worker/src/event_store.rs` | Updated field paths for sub-configs |
| `worker/src/auth.rs` | Updated test `AppConfig` construction |
| `worker/src/handlers/metadata.rs` | Updated field paths for sub-configs |
| `.issues/009_codebase_refactoring_performance.md` | **NEW** — Issue tracking |
| `README.md` | Updated architecture section + performance layers table |

## Test Results

```
cargo test --all: 34 passed, 0 failed
cargo check -p event-checkin-domain: clean
cargo check -p event-checkin-worker: 1 warning (WorkerError unused — expected)
cargo clippy -p event-checkin-worker: 1 warning (same)
```

## Architecture Decisions

### Why KV cache instead of in-memory?
- Cloudflare Workers are stateless — no persistent in-memory cache across requests
- KV is the only shared state mechanism available in the Workers runtime
- TTL-based expiry ensures staleness is bounded

### Why 30-second TTL for attendee cache?
- Events have 30-500 attendees, changes are infrequent (only on check-in/claim)
- Write-through invalidation keeps cache fresh after mutations
- 30s TTL is a safety net for edge cases (direct Sheets edits by organizers)

### Why `EventDefaults` instead of removing event fields?
- `seed_from_config()` and `from_global_config()` still need global event defaults
- Removing would break the seed/migrate flow
- Isolating in a sub-struct makes the intent clear: these are defaults, not runtime config

### Why `WorkerError` newtype instead of `impl IntoResponse for AppError`?
- Rust orphan rule: can't implement a foreign trait for a foreign type
- `AppError` is in `domain` (foreign to `axum`), `IntoResponse` is from `axum` (foreign to `AppError`)
- Newtype wrapper is the idiomatic solution

## Remaining Work

### 🔴 From Issue 009 (Not Started)
- [ ] Migrate handlers from `json!({})` to typed `ApiResponse<T>` responses (P1)
- [ ] Extract claim flow into service function in `worker/src/claim.rs` (P2)
- [ ] Rate limiting on public endpoints (P2)
- [ ] Workers Assets to replace `include_str!("index.html")` (P3)
- [ ] Structured tracing fields (P3)
- [ ] JSON response compression (P3)

### 🟡 Technical Debt
- [ ] Migrate handlers to return `Result<Json<T>, WorkerError>` instead of `Json<serde_json::Value>`
- [ ] Add integration tests for KV cache hit/miss paths
- [ ] Monitor cache hit rates in production logging

## How to Dev/Test

```bash
# All unit tests
cargo test --all

# Domain crate only
cargo test -p event-checkin-domain

# Worker crate only (includes crypto, auth, sheets, solana tests)
cargo test -p event-checkin-worker

# Check compilation
cargo check -p event-checkin-worker

# Clippy
cargo clippy -p event-checkin-worker --all-targets
```

## Issues Ref

- `.issues/009_codebase_refactoring_performance.md` — Full priority matrix and tracking
