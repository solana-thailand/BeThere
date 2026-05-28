# Issue #039: Cloudflare Workers Platform Improvements

## Summary

Consolidated plan for expanding BeThere's use of the Cloudflare Workers platform. Covers R2, Queues, Workflows, Durable Objects, Smart Placement, Rate Limiting, Analytics Engine, Gradual Deploy, and OpenTelemetry. Each feature is evaluated against the project roadmap, current pain points, and real pricing.

## Current Bindings (Already in Production)

| Binding | Product | Usage | Plan |
|---------|---------|-------|------|
| `QUIZ` | KV | Quiz questions + progress | Free |
| `EVENTS` | KV | Event registry + per-event config | Free |
| `DB` | D1 | Claim locks + audit trail (Issue #037) | Free |
| `[assets]` | Static Assets | Leptos WASM frontend (SPA mode) | Free |
| `[triggers]` | Cron | Daily cleanup at 03:00 UTC | Free |

## Current Free Plan Constraints

These limits are why some improvements need the $5/mo Workers Paid plan:

| Limit | Free | Paid ($5/mo) | BeThere Impact |
|-------|------|-------------|----------------|
| Requests | 100K/day | 10M/mo + $0.30/M | ✅ Sufficient for single events |
| CPU time | 10ms/invocation | 30ms default (up to 5min) | ⚠️ Tight for escrow TX building + Google Sheets in one request |
| KV reads | 100K/day | 10M/mo + $0.50/M | ✅ OK |
| KV writes | 1K/day | 1M/mo + $5.00/M | ⚠️ ~400 writes/event day (check-in + cache invalidation) |
| D1 reads | 5M/day | 25B/mo included | ✅ Generous |
| D1 writes | 100K/day | 50M/mo included | ✅ Generous |
| Worker size | 3MB gzip | 10MB gzip | ⚠️ WASM binary + badge SVGs approach limit |
| Subrequests | 50/request | 10K/request | ✅ OK |

## Scattered TODOs (Consolidated from Handovers)

These items appear as "TODO" or "technical debt" across multiple handovers but have never been tracked in a single issue:

| Feature | Handovers Mentioning It | Quote |
|---------|------------------------|-------|
| **R2 for slip storage** | #029, #032, #065, #066, #074 | "R2 image storage for slips — THB slip upload currently accepts URL string; need R2 bucket for actual file upload" (#029) |
| **R2 for badge images** | #065 | "Upload to R2 instead of KV (base64 images bloat storage)" |
| **R2 for refund proof** | #074 | "R2 upload widget for refund proof (instead of manual URL paste)" |
| **Rate limiting** | #003, #004, #027, #030, #032 | "Rate limiting on public endpoints (P2)" (#027), "Rate limiting on confirmation endpoint" (#030) |
| **Smart Placement** | None | Not mentioned anywhere in project |
| **Queues** | #037 | "❌ No async jobs (yet)" |
| **Workflows** | None | Not mentioned anywhere in project |
| **Durable Objects** | #037 | "Considered, too complex for now" |
| **Analytics Engine** | #037 | "❌ No analytics dashboard" |

---

## Improvement Plan — Prioritized

### P0: Zero-Cost, Zero-Risk (Free Plan, < 1 day each)

#### 1. Smart Placement

**Status**: Not documented. Zero mentions in project.

**What**: Add `[placement] mode = "smart"` to `wrangler.toml`. Cloudflare automatically places the Worker close to the most-used backend (D1, Google Sheets API, Helius RPC).

**Why**: Check-in latency is ~500ms–2s. Most of that is network round-trips to Google Sheets (US) + Helius RPC (US). Smart Placement reduces this by co-locating the Worker with backends.

```toml
# wrangler.toml — add after [triggers]
[placement]
mode = "smart"
```

**Cost**: $0 on all plans.

**Risk**: Zero. Cloudflare routes automatically. Can be removed in 1 second.

**Effort**: 1 line of config.

---

#### 2. Workers Rate Limiting (Built-in)

**Status**: Mentioned as TODO in handovers #003, #004, #027, #030, #032. All say "Add rate limiting" but none were implemented. Currently handled by "Cloudflare Rate Limiting Rules (dashboard config)" per `middleware.rs` comments.

**What**: Use Cloudflare's built-in per-Worker rate limiting instead of dashboard rules. Version-controlled, programmatic.

**Why**: Public endpoints (`/auth/callback`, `/deposit/usdc/webhook`, `/claim/{token}`, `/register`) have no rate limiting. The dashboard rules aren't version-controlled.

```toml
# wrangler.toml — add rate limit binding
[[unsafe.bindings]]
name = "RATE_LIMITER"
type = "ratelimit"
namespace_id = "rate_limit_api"
```

**Cost**: $0 on all plans.

**Risk**: Low. Only affects abusers. Can set generous limits initially.

**Effort**: ~1 hour (config + middleware guard on sensitive routes).

**Affected files**: `wrangler.toml`, `worker/src/middleware.rs`.

---

#### 3. R2 for NFT Metadata + Badge Assets + Slip Images

**Status**: Mentioned as TODO in 5+ handovers (#029, #032, #065, #066, #074). Issue #010 mentions `slip_url: "https://r2.../slip-abc123.jpg"` as the target. Currently using base64 data URLs and `include_str!` SVGs.

**What**: Add R2 bucket binding. Store:
- NFT metadata JSON (for Helius cNFT minting — replaces external Arweave/IPFS dependency)
- Badge SVG files (replaces `include_str!("badge.svg")` — reduces WASM binary size)
- THB slip images (replaces base64 data URLs in KV — 6x storage reduction)
- Refund proof images (replaces manual URL paste)
- Walk-in CSV exports (presigned URL download)

**Why**:
- Current `badge.svg` + `badge_production.svg` are compiled into WASM binary via `include_str!` — wastes Worker size budget
- THB slips stored as base64 data URLs in KV — bloats storage and KV write budget
- NFT metadata currently needs external hosting (Arweave/IPFS) — R2 is zero-egress, self-hosted

```toml
# wrangler.toml
[[r2_buckets]]
binding = "ASSETS"
bucket_name = "bethere-assets"
```

**Free tier**: 10GB storage, 1M Class A ops/mo, 10M Class B ops/mo, **zero egress**.

**Cost**: $0 for typical event usage (< 1GB of images/metadata).

**Risk**: Low. `worker` crate supports R2 via `env.r2("ASSETS")`.

**Effort**: ~1 day (binding + upload helpers + migrate badge SVGs + slip upload refactor).

**Affected files**: `wrangler.toml`, `worker/src/state.rs`, `worker/src/metadata.rs` (badge SVGs), `worker/src/handlers/deposit/thb.rs` (slip upload), `worker/src/solana.rs` (NFT metadata hosting).

> **Note**: Runtime optimization (router caching, KV binding caching, parallel API calls) tracked in Issue #041.

---

### P1: High-Impact, Needs Workers Paid Plan ($5/mo)

#### 4. Queues — Async Deposit/Refund/Sheet Sync

**Status**: Listed as "❌ No async jobs (yet)" in Issue #037. The current `wait_until()` pattern for background Google Sheets writes is fragile — if the isolate evicts before completion, the write is lost.

**What**: Add Cloudflare Queues for:
- **Google Sheets sync** — offloaded from request path entirely (check-in response returns immediately)
- **NFT minting** — fire-and-forget queue instead of blocking claim response
- **USDC refund queue** — durable, auto-retry failed on-chain TXs
- **Batch THB refunds** — consumer processes N refunds in one run

**Why**:
- `wait_until()` is best-effort — no delivery guarantee, no retry, no dead letter queue
- Google Sheets API rate limit (~100 req/100s) means concurrent check-ins can fail silently
- Escrow refund failures (RPC timeout, wrong nonce) currently require manual retry

```toml
# wrangler.toml
[[queues.producers]]
queue = "sheets-sync"
binding = "SHEETS_QUEUE"

[[queues.consumers]]
queue = "sheets-sync"
max_batch_size = 10
max_batch_timeout = 5
max_retries = 3

[[queues.producers]]
queue = "mint-queue"
binding = "MINT_QUEUE"

[[queues.consumers]]
queue = "mint-queue"
max_batch_size = 1
max_retries = 3
```

**Free tier**: 10K ops/day (~3,333 messages/day). Paid: 1M ops/mo + $0.40/M.

**Cost**: $0 on Free for dev/small events. $5/mo plan for production.

**Risk**: Medium. Queues consumer is a separate handler (`#[event(queue)]`) — needs `worker` crate queue support verification.

**Effort**: ~2-3 days (queue bindings + consumer handlers + migrate `wait_until()` call sites).

**Affected files**: `wrangler.toml`, `worker/src/lib.rs` (queue handler), `worker/src/handlers/checkin.rs`, `worker/src/handlers/claim.rs`, `worker/src/sheets/write.rs`.

**Blocks**: Phase 10 (mainnet) — production reliability requires guaranteed delivery.

---

#### 5. Workflows — Escrow Lifecycle Orchestration

**Status**: Not mentioned anywhere in project.

**What**: Use Cloudflare Workflows for the 5-step escrow lifecycle:
1. `create_event` → Workflow starts, stores event params
2. `deposit` → Workflow tracks each deposit (sub-step)
3. `mark_checked_in` → Workflow records check-in
4. Sleep until `event_end + refund_deadline`
5. Auto `claim_forfeited` → Workflow wakes and processes

**Why**:
- Current escrow state is manually tracked in KV with cron-based cleanup
- "After event ends, auto-claim-forfeited" requires manual organizer action or cron polling
- Transient Helius RPC failures require manual retry
- Workflows provide: durable state, automatic retry, sleep until deadline, step-by-step debugging

```toml
# wrangler.toml
[[workflows]]
name = "escrow-lifecycle"
binding = "ESCROW_WORKFLOW"
class_name = "EscrowWorkflow"
```

**Pricing**: Same as Workers (100K req/day Free, 10M/mo + $0.30/M Paid). Storage: 1GB free.

**Cost**: $0 on Free. $5/mo plan for production.

**Risk**: Medium-High. Workflows API is newer; `worker` crate support needs verification. The 10ms CPU limit on Free makes multi-step workflows impractical.

**Effort**: ~3-5 days (Workflow definition + integrate with existing TX builders + testing).

**Blocks**: Phase 11 (platform fees on forfeited deposits) — needs reliable auto-claim-forfeited.

---

#### 6. Gradual Deployments

**Status**: ✅ Runbook created at `docs/gradual_deploy_runbook.md`

**What**: Use Wrangler's gradual deployment for Phase 10 (mainnet rollout).

```bash
npx wrangler versions upload  # Upload new version
npx wrangler deployments create --version <id> --percentage 10  # 10% traffic
```

**Why**: Mainnet deployment of escrow TXs is high-risk. Gradual deploy lets you:
- Ship to 10% of traffic first
- Monitor error rates before full rollout
- Instant rollback if TXs fail on mainnet

**Cost**: $0. Built into Wrangler.

**Risk**: Zero.

**Effort**: ~30 min (Wrangler config + rollback procedure documentation).

**Blocks**: Phase 10 (mainnet deployment).

**Deliverables**: `docs/gradual_deploy_runbook.md` (mermaid flow, pre-deploy checklist, monitoring commands, rollback procedure, post-deploy verification).

Also created `docs/cloudflare_bug_report_10013.md` — ready to file at https://github.com/cloudflare/workers-sdk/issues/new/choose

---

### P2: Future Phase (Phase 12+ Multi-Org SaaS)

#### 7. Durable Objects — Real-Time Event Coordination

**Status**: Listed as "Considered, too complex for now" in Issue #037.

**What**: Per-event Durable Object with SQLite storage for:
- Single-writer coordination (no double check-ins)
- Real-time attendee count (WebSocket to admin dashboard)
- Atomic deposit status transitions

**Why**: KV is eventually consistent. For 500+ concurrent check-ins at a live event, D1's `INSERT ON CONFLICT` is sufficient but Durable Objects would be the "correct" solution for strictly serializable per-event state.

**Cost**: Free tier: 100K req/day, 13K GB-sec/day. Paid: 1M req/mo + $0.15/M + $12.50/M GB-sec duration.

**When**: Phase 12 (multi-organizer SaaS) when concurrent access patterns justify it.

---

#### 8. Analytics Engine — Event Metrics

**Status**: Listed as "❌ No analytics dashboard" in Issue #037.

**What**: Track time-series metrics:
- Check-in velocity (attendees/minute)
- Deposit conversion funnel (registered → deposited → checked-in → claimed)
- Per-event attendance analytics

**Why**: Admin stats currently require D1 queries. Analytics Engine provides high-cardinality, time-series data at scale.

**Cost**: Requires Workers Paid plan. ~$5/mo + minimal usage.

**When**: Phase 13 (Learning & Credentials platform).

---

#### 9. OpenTelemetry Export

**Status**: Not mentioned anywhere in project.

**What**: Export traces and logs to Grafana/Honeycomb/Sentry.

**Why**: Current observability is `tracing::info!` + `wrangler tail`. Correlation IDs exist (`middleware.rs`) but are only visible in logs. OTEL would provide end-to-end request flow visibility.

**Cost**: $0 on Workers side. External service (Grafana/Honeycomb) may have its own cost.

**When**: Post-mainnet when production debugging becomes critical.

---

## Pricing Summary by Scenario

| Scenario | Monthly Cost | What Changes |
|----------|-------------|--------------|
| Current (Free plan) | **$0** | As-is |
| + Smart Placement + Rate Limiting + R2 (Free plan) | **$0** | P0 items only |
| + Queues + Workflows (Paid plan) | **$5** | Production reliability |
| + DO + Analytics (Paid plan) | **$5–15** | Multi-org SaaS scale |

## Recommended Sequence

```
Now (Free plan):
  1. Smart Placement (1 line)        — immediate latency improvement
  2. R2 binding (1 day)              — unblocks NFT metadata, fixes slip storage debt
  3. Rate Limiting (1 hour)          — security hardening

Pre-Mainnet ($5/mo):
  4. Gradual Deploy (30 min)         — safe mainnet rollout
  5. Queues (2-3 days)               — production reliability for sheets sync + minting
  6. Workflows (3-5 days)            — escrow lifecycle automation

Post-Mainnet ($5/mo):
  7. Durable Objects                 — Phase 12 multi-org
  8. Analytics Engine                — Phase 13 learning platform
  9. OpenTelemetry                   — production observability
```

## Acceptance Criteria

### P0 (Free Plan)
- [x] `[placement] mode = "smart"` in `wrangler.toml`
- [x] R2 bucket binding added (`ASSETS_BUCKET` → `bethere-assets`)
- [x] `worker::Bucket` cached per isolate in `AppState.r2`
- [x] R2 storage helper module (`worker/src/storage.rs`)
- [x] Rate limiting on auth, claim, deposit, webhook endpoints (in-memory middleware)
- [x] All existing tests pass (21 worker + 39 on-chain = 60 total)
- [x] Slip upload handler migrates data URLs to R2 (backward compatible)
- [x] Refund proof handler migrates data URLs to R2 (backward compatible)
- [x] R2 serving endpoint `GET /api/storage/{key:path}` with prefix security
- [ ] Badge SVGs served from R2 (not `include_str!`) — low priority, SVGs are small
- [ ] WASM binary size reduced (badge SVGs removed) — blocked on badge migration

### P1 (Workers Paid Plan)
- [ ] Queues for sheets sync, minting, refund processing
- [ ] `wait_until()` calls replaced with queue sends
- [ ] Workflows for escrow lifecycle (sleep until deadline + auto-claim-forfeited)
- [ ] Gradual deployment procedure documented
- [ ] Zero regression on existing E2E tests

## Refs

- Workers pricing: https://developers.cloudflare.com/workers/platform/pricing/
- Workers limits: https://developers.cloudflare.com/workers/platform/limits/
- R2 pricing: https://developers.cloudflare.com/r2/pricing/
- Queues pricing: https://developers.cloudflare.com/queues/platform/pricing/
- Workflows pricing: https://developers.cloudflare.com/workflows/reference/pricing/
- Durable Objects pricing: https://developers.cloudflare.com/durable-objects/platform/pricing/
- Rate Limiting: https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/
- Smart Placement: https://developers.cloudflare.com/workers/configuration/placement/
- Gradual Deployments: https://developers.cloudflare.com/workers/configuration/versions-and-deployments/gradual-deployments/
- Issue #037 (D1 migration — Cloudflare Storage Landscape table)
- Issue #010 (deposit/refund — R2 slip storage mentioned)
- Handovers #029, #032, #065, #066, #074 (R2 TODOs)
- Handovers #003, #004, #027, #030, #032 (Rate limiting TODOs)

## Related Issues
- Issue #041 — Worker Runtime Optimization + File Reorganization
