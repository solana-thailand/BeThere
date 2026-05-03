# 009 — Codebase Refactoring & Performance

## Summary

Structural and performance improvements identified during full codebase review (~22K lines). Focus on reducing latency (KV cache for Sheets reads), improving type safety (typed errors/responses), and reducing maintenance burden (DRY handler patterns).

## Priority Matrix

| # | Improvement | Impact | Effort | Priority |
|---|------------|--------|--------|----------|
| 1 | KV attendee cache (eliminate Sheets full-scan) | 100x latency reduction | 2 days | **P0** |
| 2 | Cache Google access token in KV | 100ms saved/request | 0.5 day | **P0** |
| 3 | Typed error enum (replace `Result<T, String>`) | Maintainability | 1 day | **P1** |
| 4 | Typed API responses (replace `json!({})`) | Type safety + docs | 1 day | **P1** |
| 5 | Extract shared event resolution | DRY handlers | 0.5 day | **P1** |
| 6 | Split `AppConfig` into sub-configs | Maintainability | 0.5 day | **P2** |
| 7 | Mint request struct (12 -> 3 params) | Clean code | 0.5 day | **P2** |
| 8 | Rate limiting on public endpoints | Security | 1 day | **P2** |
| 9 | Claim flow service extraction | Testability | 1 day | **P2** |
| 10 | Workers Assets (replace `include_str!`) | Deploy independence | 0.5 day | **P3** |
| 11 | Structured tracing fields | Observability | 0.5 day | **P3** |
| 12 | JSON response compression | Bandwidth | 0.5 day | **P3** |

## Key Findings

### Performance Bottleneck: Google Sheets Full-Scan
- Every check-in/lookup fetches ALL attendees from Sheets API (200-800ms)
- `get_attendee_by_id()` calls `get_attendees()` then does O(n) scan
- Google Sheets rate limit: ~100 req/100 sec
- Fix: KV attendee cache with TTL refresh

### Structural Issue: `Result<T, String>` Everywhere
- No error classification (not found vs auth vs timeout)
- Can't match on error variants
- Handlers manually wrap every response in `json!({})`

### DRY Violations
- `EventIdQuery` struct duplicated in 4 handlers
- Event resolution + access check pattern repeated in 6 handlers (12+ lines each)
- 150+ `json!()` calls across handlers

## Progress

| # | Improvement | Status |
|---|------------|--------|
| 1 | KV attendee cache | ✅ Done |
| 2 | Cache Google access token in KV | ✅ Done |
| 3 | Typed error enum (`AppError`) | ✅ Done |
| 4 | Typed API responses (replace `json!({})`) | ❌ Deferred (44 call sites, mechanical) |
| 5 | Extract shared event resolution (`ext.rs`) | ✅ Done |
| 6 | Split `AppConfig` into sub-configs | ✅ Done |
| 7 | `MintRequest` struct | ✅ Done |
| 8 | Migrate handlers to `Result<Json<T>, WorkerError>` | ✅ Done |
| 9 | Rate limiting on public endpoints | ⏭️ Skipped — use Cloudflare Rate Limiting Rules (dashboard) |
| 10 | Claim flow service extraction | ✅ Done |
| 11 | Workers Assets binding | ⏭️ Skipped — `include_str!` fallback still needed for SPA routes |
| 12 | Structured tracing fields | ✅ Done — 72 call sites converted to structured fields |

## Status

🟡 In Progress (10/12 done, 1 deferred, 2 skipped)
