# Issue #050: Durable Objects + SQLite for ACID Guarantees

> **Status: 📋 PLANNING**
> Prerequisite: Issue #046 (D1 as Primary Data Store) — Phase 2a-2c ✅ DEPLOYED
> CTO decision: Use Durable Objects through D1 to get ACID guarantees.

## Summary

Introduce Durable Objects (DO) with built-in SQLite storage for ACID-critical write operations — check-in, claim, claim_lock, and registration writes. D1 remains as the read store for analytics, dashboards, and low-contention data. This hybrid architecture gives us strict serializability where it matters without sacrificing D1's convenience for reads.

## Motivation

### CTO Decision

The CTO has directed that we route database operations through Durable Objects to gain ACID guarantees:

| ACID Property | D1 Alone | DO + SQLite |
|---------------|----------|-------------|
| **Atomicity** | Near-atomic (ON CONFLICT) | Truly atomic (single-threaded) |
| **Consistency** | Eventually consistent across replicas | Strongly consistent (single instance) |
| **Isolation** | No global lock across edge | Single-threaded execution = no races |
| **Durability** | Commits survive Workers restarts | Commits survive Workers restarts |

### Why Now

- D1 Phase 2a-2c is deployed and validated (Issue #046, Handover #087)
- The `workers-rs` crate v0.8+ supports Durable Objects with SQLite storage natively in Rust
- Production D1 has 0 rows — no data migration needed, perfect time to switch writes to DOs
- Claim locks use `INSERT ON CONFLICT DO NOTHING` which is near-atomic but not truly atomic under high contention

### workers-rs DO + SQLite Support

The `worker` crate provides `#[durable_object]` macro, `DurableObject` trait, and `SqlStorage` API:

```rust
use worker::{durable_object, DurableObject, State, Env, Result, Request, Response, SqlStorage};

#[durable_object]
pub struct EventDurableObject {
    sql: SqlStorage,
}

impl DurableObject for EventDurableObject {
    fn new(state: State, _env: Env) -> Self {
        let sql = state.storage().sql();
        sql.exec("CREATE TABLE IF NOT EXISTS ...", None).expect("init");
        Self { sql }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        // Single-threaded, ACID guaranteed
    }
}
```

Source: https://github.com/cloudflare/workers-rs#durable-objects

## Architecture

### Sharding Strategy: One DO Per Event

```
Client Request
     │
     ▼
Worker (Rust, existing Axum router)
     │
     ├─ READ paths → D1 (attendees list, stats, audit queries)
     │
     └─ WRITE paths → EventDurableObject (sharded by event_id)
                        │
                        └─ Built-in SQLite (ACID)
                             │
                             └─ Async sync → D1 (after commit)
```

Each `EventDurableObject` instance:
- Is identified by `event_id` (DO name)
- Has its own private SQLite database
- Processes requests **sequentially** (no concurrent access to same event)
- Can hold up to 10GB of SQLite data (generous for events)

### What Moves to DO (ACID-Critical Writes)

| Operation | Current (D1) | Target (DO + SQLite) | ACID Benefit |
|-----------|-------------|---------------------|--------------|
| `check_in_attendee` | `UPDATE attendees SET checked_in_at` | DO SQLite UPDATE | No double check-in race |
| `acquire_claim_lock` | `INSERT ... ON CONFLICT DO NOTHING` | DO SQLite INSERT | Truly atomic lock |
| `finalize_claim_lock` | `UPDATE claim_locks SET ...` | DO SQLite UPDATE | Atomic finalize |
| `release_claim_lock` | `DELETE FROM claim_locks` | DO SQLite DELETE | Atomic release |
| `claim_attendee` | `UPDATE attendees SET claimed_at` | DO SQLite UPDATE | No double-claim race |
| `upsert_attendee` | `INSERT ... ON CONFLICT DO UPDATE` | DO SQLite UPSERT | Atomic registration |
| `verify_deposit` | `UPDATE attendees SET deposit_status` | DO SQLite UPDATE | Atomic verification |
| `mark_refund` | `UPDATE attendees SET refund_tx_hash` | DO SQLite UPDATE | Atomic refund marking |
| `undo_check_in` | `UPDATE attendees SET checked_in_at=NULL` | DO SQLite UPDATE | Atomic undo |

### What Stays in D1 (Read-Only / Low Contention)

| Operation | Reason |
|-----------|--------|
| `get_attendee_by_id` | Read-only dashboard lookup |
| `get_attendee_by_claim_token` | Read-only claim page lookup |
| `get_attendees_by_event` | Admin list view |
| `get_attendee_with_claim_counts` | Stats aggregation |
| `count_in_person_attendees` | Dashboard stats |
| `audit_log` reads | Admin audit trail view |
| `contacts` reads/writes | Low contention, cross-event |
| `developer_profiles` | Low contention, registration-time only |
| `registration_responses` | Low contention, write-once |
| `events` reads | Event config lookups |

### Data Sync: DO SQLite → D1

After every write to DO SQLite, asynchronously sync the changed row to D1:

```
DO write (ACID) → commit → async sync to D1 via wait_until() → D1 serves reads
```

This ensures:
- D1 always has up-to-date data for reads
- If sync fails, DO is source of truth (next read can try DO directly as fallback)
- D1 serves as the analytics/reporting layer

## DO Schema (SQLite within EventDurableObject)

The DO SQLite schema mirrors the event-scoped subset of D1 tables:

```sql
-- Per-event attendee data (ACID writes)
CREATE TABLE IF NOT EXISTS attendees (
    id                  TEXT PRIMARY KEY,
    event_id            TEXT NOT NULL,
    email               TEXT NOT NULL,
    name                TEXT NOT NULL DEFAULT '',
    approval_status     TEXT NOT NULL DEFAULT 'approved',
    participation_type  TEXT NOT NULL DEFAULT 'in_person',
    checked_in_at       TEXT,
    checked_in_by       TEXT,
    claim_token         TEXT UNIQUE,
    claimed_at          TEXT,
    claim_asset_id      TEXT,
    claim_signature     TEXT,
    qr_url              TEXT,
    contact_channel     TEXT NOT NULL DEFAULT '',
    contact_handle      TEXT NOT NULL DEFAULT '',
    deposit_status      TEXT NOT NULL DEFAULT 'none',
    deposit_amount_usdc INTEGER NOT NULL DEFAULT 0,
    deposit_amount_thb  INTEGER NOT NULL DEFAULT 0,
    deposit_tx_hash     TEXT,
    deposit_slip_r2_key TEXT,
    deposit_verified_at TEXT,
    deposit_verified_by TEXT,
    refund_tx_hash      TEXT,
    refund_marked_at    TEXT,
    refund_marked_by    TEXT,
    refund_link         TEXT,
    bank_name           TEXT,
    bank_account_number TEXT,
    bank_account_name   TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    sheet_row_index     INTEGER,
    synced_at           TEXT
);

CREATE INDEX IF NOT EXISTS idx_attendees_email       ON attendees(email);
CREATE INDEX IF NOT EXISTS idx_attendees_claim_token ON attendees(claim_token) WHERE claim_token IS NOT NULL;

-- Per-event claim locks (ACID writes)
CREATE TABLE IF NOT EXISTS claim_locks (
    event_id   TEXT NOT NULL,
    token      TEXT NOT NULL,
    lock_id    TEXT NOT NULL,
    wallet     TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    asset_id   TEXT,
    signature  TEXT,
    claimed_at TEXT,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (event_id, token)
);

CREATE INDEX IF NOT EXISTS idx_claim_locks_expires ON claim_locks(expires_at);

-- Per-event staff
CREATE TABLE IF NOT EXISTS staff (
    email    TEXT NOT NULL,
    event_id TEXT NOT NULL,
    role     TEXT NOT NULL DEFAULT 'staff',
    name     TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (email, event_id)
);
```

Note: `audit_log`, `contacts`, `developer_profiles`, `registration_responses`, and `events` stay in D1 only — they don't need ACID guarantees.

## Configuration Changes

### wrangler.toml Additions

```toml
# Durable Object binding for ACID event writes
[[durable_objects.bindings]]
name = "EVENT_DO"
class_name = "EventDurableObject"

# Migration to create the DO class with SQLite storage
[[migrations]]
tag = "v1"
new_sqlite_classes = ["EventDurableObject"]
```

### worker/Cargo.toml

No new dependencies — `worker` crate v0.8 already includes `#[durable_object]` and `SqlStorage`.

## Migration Phases

### Phase 1: DO Skeleton + Claim Locks (Low Risk)

Create the `EventDurableObject` with claim lock operations. D1 claim locks remain as fallback.

**Files changed:**
- `worker/src/durable_objects/mod.rs` — new module
- `worker/src/durable_objects/event_do.rs` — DO class with SQLite claim lock CRUD
- `worker/src/lib.rs` — register DO class
- `worker/wrangler.toml` — add DO binding + migration
- `worker/src/claim.rs` — route claim lock ops through DO

**Acceptance:**
- [ ] `EventDurableObject` compiles and deploys
- [ ] Claim lock acquire/finalize/release works through DO
- [ ] D1 claim locks still work as fallback
- [ ] Existing e2e tests pass

### Phase 2: Check-in + Claim Writes Through DO

Move attendee mutation operations (check-in, claim, undo) through DO.

**Files changed:**
- `worker/src/durable_objects/event_do.rs` — add attendee mutation methods
- `worker/src/handlers/checkin.rs` — route check-in writes through DO
- `worker/src/handlers/claim.rs` — route claim writes through DO
- `worker/src/db/attendees.rs` — keep read functions, mark write functions as DO-routed

**Acceptance:**
- [ ] Check-in writes to DO SQLite, syncs to D1
- [ ] Claim writes to DO SQLite, syncs to D1
- [ ] No double check-in possible (DO single-threaded)
- [ ] Admin list still reads from D1

### Phase 3: Registration Writes Through DO

Move upsert_attendee and deposit operations through DO.

**Files changed:**
- `worker/src/durable_objects/event_do.rs` — add upsert + deposit methods
- `worker/src/handlers/registration.rs` — route through DO
- `worker/src/handlers/deposit.rs` — route deposit verify/refund through DO

**Acceptance:**
- [ ] Registration writes to DO SQLite, syncs to D1
- [ ] Deposit verification is atomic
- [ ] Refund marking is atomic

### Phase 4: D1 Writes Removal + DO→D1 Sync Hardening

Remove direct D1 writes from handlers. All writes go through DO. Add reliable sync with retry.

**Acceptance:**
- [ ] No handler writes directly to D1 for attendee data
- [ ] DO→D1 sync has retry logic
- [ ] D1 used exclusively for reads
- [ ] All e2e tests pass

## Pricing Estimate (Workers Free Plan)

> **Plan**: Workers Free — only SQLite-backed DOs available (which is what we use via `new_sqlite_classes`)
> **Behavior on limit breach**: Operations **hard-stop with an error** (no overage charges)
> **Daily limits reset at 00:00 UTC**

Source: https://developers.cloudflare.com/durable-objects/platform/pricing/

### Durable Objects (Free Plan — Daily Limits)

| Resource | Free Limit / day | BeThere (500-attendee event) | Headroom |
|----------|-------------------|------------------------------|----------|
| Requests | 100,000 | ~2,000 (check-ins + claims + registrations) | **50x** |
| Duration | 13,000 GB-s | ~100 GB-s (short-lived ~5ms requests) | **130x** |
| Rows read | 5,000,000 | ~5,000 (attendee lookups, claim checks) | **1,000x** |
| Rows written | 100,000 | ~2,000 (check-ins, claims, locks, DO→D1 sync) | **50x** |
| Storage | 5 GB (total) | ~5 MB per event | **1,000x** |

**Cost: $0/month** — well within free tier for normal events.

### D1 (Free Plan — Read-Only)

Same as current — read-only usage stays well within free tier.

### Free Plan Limit Warnings

Operations **fail with an error** when a daily limit is exceeded (no overage billing). Risk scenarios:

| Scenario | Likelihood | Limit at Risk | Mitigation |
|----------|-----------|---------------|----------|
| Mega event (> 2,500 attendees) | Low | 100K rows written/day | Upgrade to paid plan ($5/mo) |
| Bug causing retry loops | Medium | 100K requests/day | Add retry budget + alerting |
| Multiple events on same day | Medium | Shared across all events | Monitor usage per day |
| DO→D1 sync writes counted | Low | 100K rows written/day | Each sync = 1 row written; 2K total |

**Rule of thumb**: Up to ~2,500 attendees/event/day is safe on free plan. Beyond that, upgrade to Workers Paid ($5/month minimum).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| `workers-rs` DO + SQLite API gaps | Medium | High | Test with PoC first; fallback to D1 writes |
| DO cold start latency on first request | Medium | Low | DO stays warm during event (~4hr window) |
| DO→D1 sync lag causes stale reads | Medium | Medium | Read-after-write can query DO directly |
| DO per-event isolation limits cross-event queries | Low | Low | Cross-event queries already go to D1 |
| SQLite in DO has different semantics than D1 | Low | Medium | Both are SQLite; same SQL syntax |
| Free plan daily limit hit (hard-stop) | Low | High | Add usage monitoring; upgrade to paid if needed |
| Free plan only supports SQLite-backed DOs | None | None | We use `new_sqlite_classes` — exactly what's needed |

## Rollback Plan

Each phase is independently rollbackable:

| Phase | Rollback |
|-------|----------|
| 1 (claim locks in DO) | Route claim locks back to D1 directly |
| 2 (check-in/claim in DO) | Route writes back to D1 |
| 3 (registration in DO) | Route writes back to D1 |
| 4 (remove D1 writes) | Re-add D1 write paths |

The D1 write code is kept (not deleted) until Phase 4 is validated in production.

## Dependencies

- Issue #046 (D1 as Primary Data Store) — ✅ Phase 2a-2c deployed
- `worker` crate v0.8+ with `#[durable_object]` and `SqlStorage` — ✅ Already in Cargo.toml
- DO binding in `wrangler.toml` — 📋 To be added
- CTO approval on architecture — ✅ Received

## Refs

- workers-rs DO docs: https://github.com/cloudflare/workers-rs#durable-objects
- Cloudflare DO getting started: https://developers.cloudflare.com/durable-objects/get-started/
- DO SQLite API: https://developers.cloudflare.com/durable-objects/api/storage/
- Issue #037 (D1 Phase 1): `.issues/037_d1_database_migration.md`
- Issue #046 (D1 Phase 2): `.issues/046_d1_primary_data_store.md`
- Architecture doc: `docs/durable_objects_architecture.md`
- D1 migration architecture: `docs/d1_migration_architecture.md`
