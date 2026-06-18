# Plan 010 — Free-Tier Optimization (Consolidated View + Data Safety)

## Context

- **Today**: 2026-06-18
- **Demo Day**: 2026-06-23 (IslandDAO V4, Koh Samui)
- **Submission deadline**: 2026-06-22 midnight UTC
- **Hard constraint**: stay on Cloudflare Workers **Free tier** ($0/mo)
- **Hard constraint**: **no data loss** — D1 rows, R2 objects, KV keys must survive any optimization

This plan consolidates the scattered free-tier optimization work that already lives across issues #039, #041, #046, #053 and handover #100. It does **not** replace them — it adds three things they lack:

1. A single snapshot of DONE vs PENDING (so pre-Demo-Day work is unambiguous)
2. A **data-safety risk framework** for every optimization (the user's explicit concern)
3. A Demo-Day-first priority ordering

---

## 1. Architecture Snapshot (verified 2026-06-18)

```
┌─────────────────────────────────────────────────────────────────┐
│  Cloudflare Edge                                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Worker "bethere" (Rust→WASM, Axum-on-Workers)            │  │
│  │  • Smart Placement: ON (co-located with D1/Sheets/Helius) │  │
│  │  • Router + KV/D1 bindings cached in OnceLock per isolate │  │
│  │  • Rate limiting: in-memory middleware                    │  │
│  │  • WASM: 4.9 MB raw / 1.47 MB gzip (49% of 3 MB limit)   │  │
│  └────────────┬─────────────┬───────────────┬─────────────────┘  │
│               │             │               │                    │
│         ┌─────▼─────┐ ┌─────▼─────┐ ┌───────▼───────┐            │
│         │  D1 "DB"  │ │ R2 "ASSETS│ │ KV "EVENTS"   │            │
│         │ (primary) │ │ _BUCKET"  │ │ (legacy cache,│            │
│         │ 19 tables │ │ slips,    │ │  writes being │            │
│         │           │ │ refunds,  │ │  eliminated)  │            │
│         └───────────┘ │ badges    │ └───────────────┘            │
│                       └───────────┘                              │
│  Static Assets: Leptos WASM 3.9 MB + CSS 296 KB (served by CDN)  │
│  Cron: daily cleanup 03:00 UTC (I/O-bound, not CPU-bound)        │
└─────────────────────────────────────────────────────────────────┘
            │              │                │
       Helius RPC    Google Sheets    Solana escrow
       (cNFT mint)   (attendee sync)  (deposit/refund)
```

**Business flows**: register → deposit (USDC escrow OR THB slip→R2) → check-in → quiz/adventure gate → claim (Helius cNFT) → refund (escrow TX).

---

## 2. Free-Tier Limit Status (measured where possible)

| Limit | Free cap | Current usage | Source | Headroom |
|---|---|---|---|---|
| Requests/day | 100,000 | event-scale traffic | — | Plenty for single event |
| **CPU time / request** | **10 ms** | not measured | — | **Unknown — measure this** |
| CPU time / cron | 10 ms | low (I/O-bound cleanup) | code review | OK |
| Memory / isolate | 128 MB | slip upload = main risk | — | OK if streaming |
| Subrequests / request | 50 | D1+R2+RPC+Sheets+Helius | — | OK |
| Simultaneous connections | 6 | parallel fan-out | — | Watch |
| **Worker size (gzip)** | **3 MB** | **1.47 MB** | `ls -lh` | **51% free** |
| **KV writes/day** | **1,000** | **510 (51%) pre-fix** | `scripts/diag_kv_usage.py` | **Fix pending deploy** |
| KV reads/day | 100,000 | ~12K (12%) | diag script | OK |
| Static asset file size | 25 MiB | 3.9 MB WASM | `ls` | OK |
| Env vars / Worker | 64 | ~15 + secrets | wrangler.toml | OK |
| Cron triggers / account | 5 | 1 | wrangler.toml | OK |

**Two real risks**: (a) **KV writes** — already fixed on branch, not deployed; (b) **CPU time per request** — never measured, could be the silent killer on the escrow mint/refund path.

---

## 3. Data-Safety Risk Framework

Every optimization below is classified by what it can and cannot affect. **No optimization in this plan deletes or mutates stored data.** Risks are about *service disruption during the change*, not data loss.

| Tier | What it touches | Data risk | Rollback |
|---|---|---|---|
| 🟢 **Zero-risk** | Compile flags, build pipeline only | None — same code, smaller binary | Redeploy previous bundle |
| 🟡 **Low-risk** | Additive caching, headers | Stale reads for TTL window | Delete KV key / lower TTL |
| 🟠 **Medium-risk** | Logic refactor (batching, streaming) | Wrong behavior if bug | `wrangler rollback` + D1 backup first |
| 🔴 **Avoid pre-Demo-Day** | Auth algo, trust boundary, architecture | Session invalidation, security change | Plan separately |

**Universal safety protocol for any 🟠 change:**
```bash
# 1. Backup D1 before
npx wrangler d1 export bethere-db --output backup-$(date +%Y%m%d).sql

# 2. Test locally
cargo test -p event-checkin-worker
wrangler dev  # smoke-test the changed path

# 3. Deploy
cd worker && ./deploy.sh

# 4. Rollback button (does NOT roll back D1 — that's why step 1 matters)
npx wrangler rollback
```

R2 and KV are immutable stores — a deploy cannot delete them. The only way to lose R2/KV data is explicit `delete()` calls in code or lifecycle rules.

---

## 4. DONE — Already Shipped (do not redo)

These are completed work from issues #039 / #041 / #046 / #053. Listed here so nobody re-does them.

| Item | Where | Issue |
|---|---|---|
| Smart Placement | `wrangler.toml [placement]` | #039 P0.1 |
| R2 binding + storage helpers | `worker/src/storage.rs` | #039 P0.3 |
| Slip + refund proof → R2 | `handlers/deposit/thb.rs` | #039 |
| Rate limiting middleware | `middleware/rate_limit.rs` | #039 P0.2 |
| Router cached in `OnceLock` | `lib.rs` | #041 H1 |
| KV/D1 bindings cached in `OnceLock` | `state.rs` | #041 H2 |
| `console_log` guarded by `OnceLock` | `lib.rs` | #041 H3 |
| Parallel RPC calls (indexer) | `escrow_indexer/` | #041 H4 |
| Parallel Sheets lookups (deposit) | `usdc/` | #041 H5 |
| Release profile: `opt-level=z, lto, strip, codegen-units=1, panic=abort` | `Cargo.toml` | #041 |
| File splits (all >1024-line files) | `worker/src/**` | #041 Phase 2 |
| D1 as primary data store | 19 migrations | #046, #053 |
| Frontend served as static assets (SPA mode) | `wrangler.toml [assets]` | — |

---

## 5. PENDING — Prioritized Pre-Demo-Day Work

### 🔴 P0 — Must-do before Jun 22 (blocking free-tier safety)

#### P0.1 — Deploy the KV write elimination fix
- **Branch**: `feature/kv_write_elimination` (2 commits, clean FF from `develop`)
- **Risk**: 🟢 Zero — read paths already D1-first, only write paths removed
- **Impact**: drops KV writes from ~510/day → ~0/day (frees 51% of write quota)
- **Action**: `cd worker && ./deploy.sh`, then verify with `python3 scripts/diag_kv_usage.py --days 3`
- **Ref**: handover #100, issue #053

#### P0.2 — Measure CPU time per request (the unknown risk)
- **Why**: 10 ms free-tier CPU cap is the one limit we have zero data on. Escrow TX building + Solana sig verify could blow past it silently.
- **How**: `wrangler tail` → look at `cpuTime` in invocation logs. Hit each hot path once:
  - `POST /api/checkin` (staff scan)
  - `POST /api/claim/{token}` (cNFT mint)
  - `POST /api/deposit/usdc/confirm` (escrow deposit)
  - `POST /api/deposit/refund` (escrow refund)
- **Risk**: 🟢 Zero (read-only observation)
- **Decision rule**: if any path >7 ms CPU, flag for P1 optimization. If >10 ms, it's already failing in prod — needs immediate attention.

---

### 🟡 P1 — Zero-risk wins (do if time permits, no data concern)

#### P1.1 — Verify `wasm-opt -Oz` runs in the build pipeline
- **File**: `worker/build.sh` (or wherever `wasm-bindgen` is invoked)
- **Check**: is `optimize-wasm.sh` actually called? If not, wire it in.
- **Risk**: 🟢 Zero — binary shrinking only
- **Expected gain**: 10–20% WASM size reduction → more 3 MB headroom
- **Verify**: `wrangler deploy --dry-run` reports gzip size before/after

#### P1.2 — Move `badge.svg` / `badge_production.svg` out of `include_str!`
- **Current**: compiled into WASM binary via `include_str!`
- **Target**: serve from R2 (bucket already exists)
- **Risk**: 🟡 Low — if R2 fetch fails, badge generation fails. Mitigation: keep `include_str!` as fallback during transition, or upload to R2 first + verify before switching.
- **Impact**: small WASM shrink + cleaner architecture
- **Ref**: issue #039 P0.3 (marked low priority there)

---

### 🟠 P2 — Medium-risk, needs testing (skip if Demo Day is close)

These are the unfinished items from issue #041. Each requires a code change + D1 backup + local test. Safe to defer past Demo Day.

| Item | File | Risk | Ref |
|---|---|---|---|
| H6: D1 primary for audit reads (stop KV read-modify-write) | `audit_store.rs` | 🟠 | #041 |
| M1: Atomic deposit counter via D1 `UPDATE x = x + 1` | `event_store/write.rs` | 🟠 | #041 |
| M2: Individual KV keys for on-chain events (not array rewrite) | `escrow_indexer/store.rs` | 🟠 | #041 |
| M3: Batch KV reads for deposit lists | `event_store/read.rs` | 🟠 | #041 |
| Stream slip uploads to R2 (avoid buffering whole body) | `handlers/deposit/thb.rs` | 🟠 | — |

---

### 🔴 AVOID pre-Demo-Day — real risk, plan separately

| Item | Why risky |
|---|---|
| JWT RS256 → HS256 | Invalidates all existing sessions. Security decision, not just optimization. |
| Offload Solana signature verification | Changes trust boundary. |
| Move cleanup cron off-Worker (GitHub Action) | New external dependency, new failure mode. Cron is I/O-bound so CPU isn't actually the constraint — no urgent need. |
| Split Worker via Service Bindings | Architectural change. You're at 1.47 MB gzip / 3 MB — 51% headroom, no need yet. |
| Remove `EVENTS` KV binding | Blocked by CF versions-API binding-parity bug. Code already handles `None`-KV gracefully. |

---

## 6. Pre-Demo-Day Checklist (Jun 18 → Jun 22)

```
[ ] P0.1  Deploy KV write elimination (feature/kv_write_elimination)
[ ] P0.1  Verify writes dropped: python3 scripts/diag_kv_usage.py --days 3
[ ] P0.2  wrangler tail — record cpuTime for: checkin, claim, deposit, refund
[ ] P0.2  If any path >10ms CPU, escalate immediately
[ ] P1.1  Confirm wasm-opt -Oz runs in build (or wire it in)
[ ] P1.2  (Optional) Move badge SVGs to R2
[ ]        D1 backup: npx wrangler d1 export bethere-db --output backup-pre-demo.sql
[ ]        Smoke-test full flow on production: register → deposit → check-in → claim
```

**Do NOT attempt P2 or AVOID items between now and Jun 22.** The risk/reward is wrong that close to submission.

---

## 7. If You Hit a Limit During Demo Day

| Symptom | Likely limit | Immediate action |
|---|---|---|
| Error 1027 (cloudflare page) | Requests/day (100K) | Wait for midnight UTC reset, or fail-open route |
| Error 1102 "exceeded resource limits" | CPU time (10 ms) or Memory (128 MB) | Find the offending path via `wrangler tail`, cache or simplify it |
| Writes silently failing | KV writes/day (1K) | Confirm P0.1 deployed; if not, deploy now |
| 413 on slip upload | Request body (100 MB free) | Stream the upload instead of buffering |

---

## 8. What This Plan Does NOT Do

- Does not duplicate technical detail already in issues #039, #041, #046, #053 — read those for implementation specifics.
- Does not recommend upgrading to Workers Paid. The user has explicitly committed to free tier through Demo Day.
- Does not introduce new architectural patterns. The existing OnceLock + D1-primary + R2-for-blobs design is sound.
- Does not touch the Solana escrow program (separate crate, separate risk surface).

---

## Refs

- Issue #039 — Cloudflare platform improvements (R2, rate limit, smart placement — all DONE)
- Issue #041 — Worker runtime optimization + file reorg (Phase 1 mostly done, Phase 2 done)
- Issue #046 — D1 primary data store migration
- Issue #053 — KV→D1 remaining migration (write paths still pending — closed by handover #100)
- Handover #100 — KV write elimination (fix on branch, NOT DEPLOYED)
- Handover #104 — audience aggregation and sync fix (latest)
- CF limits: https://developers.cloudflare.com/workers/platform/limits/

## Related Plans

- Plan 009 — Event Poster (the `/e/{slug}` lightbox work that triggered this conversation)
