# Handover #076: D1 Database Migration Research & Plan

## What Happened

Researched three Rust crates/libraries for potential integration into BeThere:

1. **`tokio-rs/io-uring`** — Linux-only kernel async I/O. **Not applicable** — our runtime is Cloudflare Workers (WASM/V8), not a Linux server. Cannot run in WASM sandbox or on macOS dev machine.

2. **`Lokathor/bytemuck`** — Safe zero-copy byte reinterpretation. **Not applicable** — our domain types (`String`, `HashMap`, `Vec`) are not Pod (Plain Old Data). We use `serde_json` + `wincode` serialization, not raw struct↔bytes casts. No GPU/graphics pipeline to benefit from it.

3. **`tursodatabase/turso`** — In-process SQLite in Rust with encryption. **Partially applicable** — good fit conceptually, but doesn't compile to WASM. The Rust crate requires a filesystem. The JS binding exists but we'd need to cross the WASM↔JS bridge manually.

This led to evaluating **Cloudflare D1** (native SQLite for Workers) as the right solution.

Also evaluated **Durable Objects + SQLite** — they offer strict serializability (truly atomic single-threaded execution), which is even stronger than D1 for claim locks. But the added complexity (Worker frontend + DO class + per-entity routing) isn't justified at BeThere's scale (< 100 concurrent claims). D1's `INSERT ON CONFLICT DO NOTHING` is near-atomic and sufficient.

Note: No D0 or D2 products exist in Cloudflare's lineup. The full storage landscape is documented in Issue #037.

### D1 Evaluation Results

| Factor | Assessment |
|--------|-----------|
| Integration | `worker = "0.8"` crate already has `env.d1("DB")` — no new deps |
| Cost | Free tier: 5M reads/day, 100K writes/day, 5 GB storage — far exceeds our needs |
| Latency | Edge-co-located with Workers — sub-ms for indexed queries |
| Limits | 10 GB per DB, single-threaded (~1K qps at 1ms avg) — fine for event-scale |
| WASM compat | Native — D1 binding works through the `worker` crate's JS interop |
| Encryption at rest | **Not built-in** — would need app-level AES-GCM via SubtleCrypto |
| Backward compat | D1 runs alongside KV — hybrid migration possible |

### Decision: D1 for Phase 1 (claim locks + audit trail)

**Why claim locks first:**
- Current `claim.rs` uses write-first-verify-after CAS pattern with eventual consistency gap
- D1 `INSERT ... ON CONFLICT DO NOTHING` is truly atomic
- Directly impacts correctness of the money-adjacent claim flow

**Why audit trail second:**
- Current `audit_store.rs` reads full JSON array (up to 1000 entries), appends, rewrites
- D1 `INSERT INTO audit_log` is O(1) — no read before write
- Audit writes happen on every mutation (check-in, claim, deposit) — high frequency

**Why KV stays for caches:**
- Google token cache, blockhash cache, sheet cache are TTL-based true caches
- KV's eventual consistency + TTL is the right semantic for caches
- No migration needed

## Where Is the Plan/Code/Test

### Plan
- **Issue**: `.issues/037_d1_database_migration.md` — full implementation plan with schema, file changes, acceptance criteria, pricing estimate

### No Code Yet
This session was research + documentation only. No code was written.

## Reflection / Struggling / Solved

### Solved
- **Turso vs D1 decision**: Turso's encryption at rest was tempting, but the WASM compat issue is a hard blocker for Workers. D1's native integration wins decisively. Encryption can be added at app level later if needed.
- **Migration phasing**: Evaluated "big bang" vs incremental. Incremental wins — D1 and KV coexist, so we can migrate claim locks first, validate, then audit trail, then attendees in future issues.

### No Struggles
Clean research session with clear conclusions.

## Remain Work

1. **Create D1 database** — `npx wrangler d1 create bethere-db` and update `wrangler.toml`
2. **Write migration SQL** — `worker/migrations/0001_initial.sql`
3. **Implement `db/` module** — Typed D1 query layer in `worker/src/db.rs`
4. **Migrate claim locks** — Replace KV CAS with atomic SQL in `claim.rs`
5. **Migrate audit trail** — Replace JSON array with SQL INSERT in `audit_store.rs`
6. **Test** — Verify atomic lock behavior, audit append performance, no regression

## Issues Ref

- Issue #037: `.issues/037_d1_database_migration.md`

## How to Dev/Test

### Prerequisites
```bash
cd worker && npm install  # ensure wrangler is available
```

### Create D1 database
```bash
npx wrangler d1 create bethere-db
# Copy the returned database_id into wrangler.toml [[d1_databases]]
```

### Run migration
```bash
npx wrangler d1 execute bethere-db --file=worker/migrations/0001_initial.sql
# For local dev:
npx wrangler d1 execute bethere-db --local --file=worker/migrations/0001_initial.sql
```

### Verify
```bash
npx wrangler d1 execute bethere-db --command "SELECT name FROM sqlite_master WHERE type='table'"
# Should show: claim_locks, audit_log
```

### Test after implementation
```bash
cd worker && cargo check --target wasm32-unknown-unknown
# Run existing test suite — all 68 tests should pass
```
