# Issue #037: Cloudflare D1 Database Migration (Phase 1 — Claim Locks + Audit Trail)

## Summary

Migrate from Cloudflare KV to Cloudflare D1 (SQLite) for claim locks and audit trail — the two highest-value targets where KV's limitations cause correctness and performance issues. Run hybrid: D1 for structured data, keep KV for edge caches.

## Background

### Why D1 over Turso

| Factor | D1 | Turso |
|--------|----|----|
| Workers integration | Native (`env.d1()`) | Requires HTTP client |
| WASM compat | Built into `worker` crate | No Rust WASM backend |
| Edge latency | Co-located with Workers | Network round-trip |
| Encryption at rest | Not built-in (app-level needed) | Built-in |
| Cost | Free tier sufficient | Separate billing |

D1 wins for our Workers architecture. Turso would only make sense if we self-hosted a Rust server.

### Why D1 over Durable Objects

Durable Objects (DO) also offer SQLite storage with strict serializability — a single DO per event would serialize all claim operations, eliminating even theoretical races. However:

| Factor | D1 | Durable Objects + SQLite |
|--------|----|--------------------------|
| Consistency | Eventually consistent reads (by default) | Strict serializability (single-threaded) |
| Claim lock | `INSERT ON CONFLICT DO NOTHING` (near-atomic) | Truly atomic — only one request processes at a time |
| Latency | Sub-ms indexed reads, but separate from Worker | Co-located compute + storage — zero network hop |
| Complexity | Familiar SQL, simple binding | Need Worker frontend + DO class + per-entity routing |
| Pricing | Rows read/written + storage (Free tier generous) | Duration (GB-sec) + requests + storage |
| Scale model | One DB, many queries | One DO per entity (e.g. per event) |
| `worker` crate support | `env.d1("DB")` — built-in | Requires `worker::DurableObject` class implementation |

For BeThere's scale (< 100 concurrent claims per event), D1's near-atomic `ON CONFLICT` is sufficient. The TOCTOU window is negligible. DO's added complexity (routing layer, DO class, lifecycle management) isn't justified. DO would make sense if we needed real-time collaboration (multiplayer) or per-user stateful compute.

### Full Cloudflare Storage Landscape

No D0 or D2 products exist. The complete lineup:

| Product | Purpose | BeThere relevance |
|---------|---------|------------------|
| KV | Eventually-consistent key-value | ✅ Already using (caches) |
| D1 | Managed SQLite database | ✅ Planned (this issue) |
| Durable Objects | Single-threaded stateful compute + SQLite | Considered, too complex for now |
| R2 | S3-compatible object storage | ❌ No large files |
| Hyperdrive | Accelerate existing Postgres/MySQL | ❌ No external DB |
| Queues | Message queuing | ❌ No async jobs (yet) |
| Vectorize | Vector embeddings DB | ❌ No AI search |
| Pipelines | Streaming ingestion → R2 | ❌ No streaming |
| Analytics Engine | Time-series metrics | ❌ No analytics dashboard |

### KV Pain Points Being Solved

1. **Claim lock TOCTOU race** (`claim.rs`) — Current write-first-verify pattern has an eventual-consistency gap. D1's `INSERT ... ON CONFLICT DO NOTHING` is atomic.
2. **Audit read-modify-write** (`audit_store.rs`) — Reads entire JSON array, appends, rewrites. D1's `INSERT INTO audit_log` is O(1) append.
3. **No queries** — Attendee filtering requires full KV prefix scans + client-side deserialization. D1 enables indexed `SELECT` queries.
4. **No schema enforcement** — JSON blobs in KV have no type safety at storage layer.

## Scope

### Phase 1 (This Issue)

| Component | Current (KV) | Target (D1) | Why first |
|-----------|-------------|-------------|-----------|
| **Claim locks** | `acquire/finalize/release_claim_lock` in `claim.rs` | `claim_locks` table | Correctness — atomic lock acquisition |
| **Audit trail** | `audit_store.rs` (JSON array read-modify-write) | `audit_log` table | Performance — O(1) append vs O(n) rewrite |

### Phase 2+ (Future Issues, documented here for planning)

| Component | Notes |
|-----------|-------|
| Attendees | `attendees` table — indexed queries by event, status, claim_token |
| Events + orgs | `events` + `organizations` tables — multi-event management |
| Quiz + adventure | `quiz_questions` + `adventure_progress` tables |
| Walk-in records | `walkin_attendees` table |
| Deposit records | `usdc_deposits` + `thb_deposits` tables |

### Stay in KV (no migration)

| Cache | Reason |
|-------|--------|
| Google access token | TTL-based, true cache |
| Solana blockhash | TTL-based, true cache |
| Attendee sheet cache | TTL-based, true cache |
| Sheet staff cache | TTL-based, true cache |

## Database Schema (Phase 1)

```sql
-- Claim dedup locks — replaces KV key "event:{id}:claim_lock:{token}"
CREATE TABLE claim_locks (
    event_id   TEXT NOT NULL,
    token      TEXT NOT NULL,
    lock_id    TEXT NOT NULL,
    wallet     TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    -- Finalized fields (NULL until mint completes)
    asset_id   TEXT,
    signature  TEXT,
    claimed_at TEXT,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (event_id, token)
);

CREATE INDEX idx_claim_locks_expires ON claim_locks(expires_at);

-- Append-only audit log — replaces KV JSON arrays
CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id    TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    actor       TEXT NOT NULL,
    action      TEXT NOT NULL,
    target      TEXT NOT NULL,
    description TEXT NOT NULL,
    metadata    TEXT  -- JSON string
);

CREATE INDEX idx_audit_event_time ON audit_log(event_id, timestamp DESC);
CREATE INDEX idx_audit_action     ON audit_log(action);
```

## Files Changed

### New Files

| File | Purpose |
|------|---------|
| `worker/migrations/0001_initial.sql` | Schema migration for D1 |
| `worker/src/db.rs` | D1 database access layer (typed queries) |
| `worker/src/db/claim_locks.rs` | Claim lock CRUD using D1 |
| `worker/src/db/audit.rs` | Audit log CRUD using D1 |

### Modified Files

| File | Change |
|------|--------|
| `worker/wrangler.toml` | Add `[[d1_databases]]` binding |
| `worker/src/state.rs` | Add `db: Option<worker::D1Database>` to `AppState` |
| `worker/src/claim.rs` | Replace KV lock calls with D1 `claim_locks` module |
| `worker/src/audit_store.rs` | Replace KV read-modify-write with D1 `INSERT` |
| `worker/src/lib.rs` | Pass D1 binding to AppState |
| `worker/src/handlers/ext.rs` | Add `resolve_db()` helper alongside `resolve_kv()` |

### Unchanged (Phase 1)

| File | Reason |
|------|--------|
| `worker/src/adventure.rs` | Stays on KV — Phase 2 |
| `worker/src/quiz.rs` | Stays on KV — Phase 2 |
| `worker/src/sheets/*` | Stays on KV cache |
| `worker/src/event_store.rs` | Stays on KV — Phase 2 |
| `domain/` | No changes — shared types work with both KV and D1 |
| `frontend-leptos/` | No changes — API surface unchanged |

## Implementation Plan

### Step 1: D1 Binding + Migration

```toml
# wrangler.toml
[[d1_databases]]
binding = "DB"
database_name = "bethere-db"
database_id = ""  # filled after `wrangler d1 create`
```

```bash
npx wrangler d1 create bethere-db
npx wrangler d1 execute bethere-db --file=worker/migrations/0001_initial.sql
```

### Step 2: State + DB Layer

Add `db: Option<worker::D1Database>` to `AppState`. Create `db/` module with typed query functions using `worker::D1Database::prepare().bind().run()`.

### Step 3: Migrate Claim Locks

Replace the 3 functions in `claim.rs`:
- `acquire_claim_lock` → `INSERT INTO claim_locks ... ON CONFLICT(event_id, token) DO NOTHING`
- `finalize_claim_lock` → `UPDATE claim_locks SET asset_id=?, signature=?, claimed_at=? WHERE event_id=? AND token=?`
- `release_claim_lock` → `DELETE FROM claim_locks WHERE event_id=? AND token=?`

### Step 4: Migrate Audit Trail

Replace `audit_store.rs` functions:
- `append_event_audit` → `INSERT INTO audit_log (event_id, actor, action, target, description, metadata)`
- `append_global_audit` → same (use a sentinel event_id like `__global__` or add a `scope` column)
- `get_event_audit` → `SELECT * FROM audit_log WHERE event_id=? ORDER BY timestamp DESC LIMIT ?`
- `get_global_audit` → `SELECT * FROM audit_log WHERE event_id='__global__' ORDER BY timestamp DESC LIMIT ?`

### Step 5: Handler Updates

Update handler call sites to pass `db` alongside `kv`. Both available during migration — handlers that haven't been migrated yet continue using KV.

## Acceptance Criteria

### Functional
- [x] Claim lock acquisition is atomic — D1 INSERT ON CONFLICT DO NOTHING
- [x] Claim lock release on mint failure works
- [x] Claim lock finalization stores asset_id + signature
- [x] Audit entries append in O(1) via D1 INSERT
- [x] Audit queries return newest-first with correct limit
- [x] Existing claim lock KV keys still readable (dual-write to KV maintained)
- [ ] All existing tests pass unchanged (pending verification)

### Configuration
- [x] D1 binding is optional — worker starts fine without it (graceful fallback to KV)
- [x] `wrangler.toml` has `[[d1_databases]]` section
- [x] Migration script is idempotent (IF NOT EXISTS)

### Performance
- [ ] Claim lock acquisition latency < 10ms (D1 indexed query — pending deployment test)
- [ ] Audit append latency < 10ms (pending deployment test)
- [ ] No regression on check-in latency (< 500ms end-to-end — pending deployment test)

## Pricing Estimate

For a 500-attendee event with full lifecycle:

| Operation | Rows read | Rows written | Count |
|-----------|-----------|-------------|-------|
| Claim lock acquire | 0 (INSERT) | 1 | ~500 |
| Claim lock finalize | 1 | 1 | ~400 |
| Audit append | 0 | 1 | ~2000 |
| Audit read (admin) | ~200 | 0 | ~50 |
| **Total** | ~250 | ~2900 | — |

**Daily cost: $0** (well within Free tier: 5M reads/day, 100K writes/day)

## Out of Scope

- Attendee/events/quiz/adventure migration (Phase 2+)
- Encryption at rest (future: app-level AES-GCM via SubtleCrypto)
- Turso integration (only if self-hosting)
- Read replication (not needed at this scale)

## Refs

- D1 docs: https://developers.cloudflare.com/d1/
- D1 pricing: https://developers.cloudflare.com/d1/platform/pricing/
- D1 limits: https://developers.cloudflare.com/d1/platform/limits/
- `worker` crate D1 API: `env.d1("DB")` → `worker::D1Database`
- Current claim lock implementation: `worker/src/claim.rs` L51-163
- Current audit implementation: `worker/src/audit_store.rs`
