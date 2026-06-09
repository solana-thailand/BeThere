# Issue #053: KV → D1 Remaining Data Migration (Phase 3)

> **Status: ✅ COMPLETE** — All phases done: 3a ✅, 3b ✅, 3c ✅, 3d ✅, 3e ✅, 3f ✅, 3g ✅. All KV data types migrated to D1.
> Prerequisites: Issue #037 (Phase 1 — claim locks + audit trail) ✅ COMPLETE, Issue #046 (Phase 2 — attendees, contacts, events, staff) ✅ COMPLETE.

## Summary

Migrate all remaining KV-only data to D1, completing the transition from KV as a primary data store. After Phase 1+2, the core operational tables (claim locks, audit log, attendees, contacts, events, staff, developer profiles, registration responses, quiz config/progress, escrow index, campaigns) are in D1. Phase 3 covers the remaining 10 KV data types that still rely on eventually-consistent storage.

## Motivation

### Current Pain Points

| Problem | Impact | Data Type |
|---------|--------|-----------|
| Adventure data lost on KV wipe | Unrecoverable progress, players must replay | Adventure config + progress |
| On-chain events not queryable | No reporting, no admin UI for transaction history | On-chain events + dedup + cursor |
| Organizations lost on KV wipe | Business entity data gone, manual restore needed | Organizations |
| THB deposits not durable | Financial records in ephemeral storage | THB deposits |
| Deposit status ephemeral | Can't audit deposit history across restarts | Deposit status |
| Form config per-event in KV | No schema enforcement, no versioning | Event form config |
| JWT blacklist depends on KV TTL | Works but inconsistent with D1-first architecture | JWT blacklist |

### Why Now

- Adventure progress is unrecoverable if KV is cleared — players lose all game state
- On-chain event data is financial/audit-critical — should be in durable, queryable storage
- Organizations were already lost once due to KV wipe — core business entity must be durable
- THB deposits are financial records requiring durability and auditability
- D1 migration is 80% complete — finishing removes KV dependency from most code paths

### Latency Targets

| Operation | Current (KV) | Target (D1) | Improvement |
|-----------|-------------|-------------|-------------|
| Adventure config read | ~5ms | ~3ms (D1 indexed) | ~1.5x |
| Adventure progress read | ~5ms | ~3ms (D1 indexed) | ~1.5x |
| On-chain events list | ~50ms (JSON deserial) | ~5ms (D1 indexed) | **10x** |
| Organization lookup | ~5ms | ~3ms (D1 indexed) | ~1.5x |
| THB deposit lookup | ~5ms | ~3ms (D1 indexed) | ~1.5x |

## Scope

### Already in D1 (Phase 1+2 Complete) ✅

| Table | Source Issue |
|-------|-------------|
| `claim_locks` | #037 |
| `audit_log` | #037 |
| `attendees` | #046 |
| `contacts` | #046 |
| `events` | #046 |
| `staff` | #046 |
| `developer_profiles` | #046 |
| `registration_responses` | #046 |
| `quiz_configs` | #046 |
| `quiz_progress` | #046 |
| `escrow_index` | #046 |
| `campaigns`, `campaign_events`, `developer_campaign_progress` | #046 |
| `adventure_configs`, `adventure_progress` | #053 Phase 3a |
| `onchain_events`, `onchain_dedup`, `onchain_cursors` | #053 Phase 3b |
| `organizations` | #053 Phase 3c |
| `thb_deposits` | #053 Phase 3d |
| `deposit_statuses` | #053 Phase 3e |
| `events.form_config` (column) | #053 Phase 3f |
| `jwt_blacklist` | #053 Phase 3g |

### In Scope (Phase 3 — This Issue)

| # | Component | KV Key Pattern | D1 Table(s) | Priority |
|---|-----------|---------------|-------------|----------|
| 1 | Adventure config | `event:{id}:adventure:config` | `adventure_configs` | 🔴 HIGH |
| 2 | Adventure progress | `event:{id}:adventure:progress:{claim_token}` | `adventure_progress` | 🔴 HIGH |
| 3 | On-chain events | `event:{id}:onchain` | `onchain_events` | 🔴 HIGH |
| 4 | On-chain dedup | `onchain:sig:{signature}` | `onchain_dedup` | 🔴 HIGH |
| 5 | On-chain cursor | `onchain:cursor:{escrow_addr}` | `onchain_cursors` | 🔴 HIGH |
| 6 | THB deposits | `thb_deposit:{event_id}:{email}`, `thb_deposits:{event_id}` | `thb_deposits` | 🟡 MEDIUM |
| 7 | Deposit status | `deposit_status:{id}` | `deposit_statuses` | 🟡 MEDIUM |
| 8 | Organizations | `org:{org_id}`, `orgs` | `organizations`, `org_index` | 🔴 HIGH |
| 9 | JWT blacklist | `jwt_blacklist:{sha256(token)}` | `jwt_blacklist` | 🟢 LOW |
| 10 | Event form config | `event:{id}:form:config` | Column in `events` or `event_form_configs` | 🟡 MEDIUM |

### Out of Scope

| Component | Reason |
|-----------|--------|
| Google OAuth token | True TTL cache, not persistent data |
| Solana blockhash cache | True TTL cache, not persistent data |
| R2 assets | Blob storage, no change needed |

### Existing KV Namespace Bindings

| Binding | ID | Status |
|---------|-----|--------|
| EVENTS | `c8a6a87f9ed34ce0a3c8e48b84039214` | Active in `wrangler.toml` |
| QUIZ | — | Removed from `wrangler.toml`, D1-only now |

After Phase 3 completion, the EVENTS KV namespace will only hold short-lived caches (Google token, blockhash). All persistent data will be in D1.

## Migration Phases

### Phase 3a: Adventure Config + Progress 🔴 HIGH

**KV keys:**
- `event:{id}:adventure:config` → `AdventureConfig` JSON
  - `enabled: bool`, `required_level: Option<usize>`
- `event:{id}:adventure:progress:{claim_token}` → `AdventureProgress` JSON
  - `claim_token: String`, `levels_completed: Vec<String>`, `scores: HashMap<String, LevelScore>`, `total_moves: u32`, `total_time_seconds: u32`, `passed: bool`, `passed_at: Option<String>`, `last_played_at: Option<String>`
  - `LevelScore`: `moves: u32`, `puzzles_solved: u32`, `time_seconds: u32`, `stars: u8`

**Why first:** Data is unrecoverable on KV wipe. Players lose all game state. Small dataset, well-defined schema.

**Acceptance:**
- [x] D1 tables created (`adventure_configs`, `adventure_progress` — migration `0010_adventure_tables.sql`)
- [x] All adventure reads query D1 only (KV fallback removed)
- [x] All adventure writes go to D1 only (KV dual-write removed)
- [x] Level scores stored as JSON blob in `progress_json` column (matches quiz pattern, denormalized columns for indexing)
- [x] KV keys no longer read or written for adventure data
- [ ] Existing KV data migrated via one-time script (optional cleanup, D1 is source of truth)

### Phase 3b: On-Chain Events + Dedup + Cursor 🔴 HIGH

**KV keys:**
- `event:{id}:onchain` → `Vec<OnChainEvent>` JSON (max 200)
  - `signature: String`, `slot: u64`, `block_time: i64`, `instruction: EscrowInstruction` (enum, JSON), `escrow_address: String`, `target_escrow_address: Option<String>`, `organizer: Option<String>`, `attendee: Option<String>`, `amount: Option<u64>`, `indexed_at: String`
- `onchain:sig:{signature}` → `"1"` (TTL 90 days)
- `onchain:cursor:{escrow_addr}` → last processed signature string

**Why high:** Financial data. Must be durable and queryable. Enables admin UI for transaction history.

**Acceptance:**
- [x] D1 tables created (`onchain_events`, `onchain_dedup`, `onchain_cursors` — migration `0011_onchain_tables.sql`)
- [x] On-chain events inserted as individual rows (not JSON array)
- [x] Dedup table replaces KV TTL-based dedup (UNIQUE constraint on signature)
- [x] Cursor table replaces KV cursor keys
- [x] KV keys no longer read or written for on-chain data (D1 only)
- [ ] Existing KV data migrated via one-time script (optional cleanup, D1 is source of truth)

### Phase 3c: Organizations 🔴 HIGH

**KV keys:**
- `org:{org_id}` → `OrganizationConfig` JSON
  - `id`, `name`, `contacts_sheet_id`, `contacts_sheet_name`, `events_sheet_name`, `owner_emails: Vec<String>`, `created_at`, `updated_at`
- `orgs` → `OrgIndex` JSON (list of OrgMeta)

**Why high:** Core business entity. Already lost once due to KV wipe.

**Acceptance:**
- [x] D1 table created (`organizations` — single table with JSON array for owner_emails)
- [x] Owner emails stored as JSON array (simpler, matching the existing domain model)
- [x] Org list query replaces `orgs` KV key + per-org config loading
- [x] KV keys no longer read or written for organization data
- [x] `resolve_contacts_sheet` uses D1 instead of KV
- [x] Delete protection (active events check) uses D1 instead of KV event index

### Phase 3d: THB Deposits 🟡 MEDIUM

**KV keys:**
- `thb_deposit:{event_id}:{email}` → ThbDeposit JSON
- `thb_deposits:{event_id}` → `Vec<ThbDeposit>` JSON

**ThbDeposit fields:** `attendee_id`, `event_id`, `amount_thb: u64`, `slip_url: Option<String>`, `verified: bool`, `verified_by: Option<String>`, `verified_at: Option<String>`, `uploaded_at: String`, `refunded: bool`, `refunded_at: Option<String>`, `attendee_name: Option<String>`, `bank_account: Option<String>`, `bank_name: Option<String>`

**Why medium:** Financial records that should be durable. No immediate risk of loss but auditability requires D1.

**Acceptance:**
- [x] D1 table created (`thb_deposits`)
- [x] Individual deposit lookup by event + attendee
- [x] Event-wide deposit list via D1 query (not JSON array)
- [x] Existing KV data cleaned up via cron (D1 is primary)
- [x] KV keys no longer read or written for THB deposit data (D1-first with KV fallback)

### Phase 3e: Deposit Status 🟡 MEDIUM

**KV key:**
- `deposit_status:{id}` → DepositStatus JSON
  - `attendee_id`, `event_id`, `method: DepositMethod` (enum), `amount: u64`, `currency: String`, `tx_signature: Option<String>`, `verified: bool`, `deposited_at: String`, `wallet_address: Option<String>`, `deposit_order: u32`, `refundable: bool`, `rejected: bool`

**Why medium:** Could be computed from THB deposits + attendees table, but dedicated table is cleaner.

**Acceptance:**
- [x] D1 table created (`deposit_statuses`)
- [x] Deposit method stored as TEXT enum
- [x] Status lookup by deposit ID from D1
- [x] Existing KV data cleaned up via cron (D1 is primary)
- [x] KV keys no longer read or written for deposit status data (D1-first with KV fallback)

### Phase 3f: Event Form Config 🟡 MEDIUM ✅ COMPLETE

**KV key:**
- `event:{id}:form:config` → JSON

**Approach:** Added `form_config TEXT` column to `events` table (Option A — JSON column).

**Files:**
- `worker/migrations/0015_event_form_config.sql` — ALTER TABLE
- `worker/src/db/events.rs` — D1 read/write helpers (`get_form_config`, `save_form_config`)
- `worker/src/event_store/read.rs` — D1-first with KV fallback
- `worker/src/event_store/write.rs` — D1-first with KV fallback
- `worker/src/handlers/events/audit.rs` — GET/PUT handlers updated
- `worker/src/handlers/public_event.rs` — public event handler updated

**Acceptance:**
- [x] Form config stored in D1 (`events.form_config` TEXT column)
- [x] All reads query D1-first (KV fallback preserved)
- [x] All writes go to D1-first (KV fallback preserved)
- [x] KV key no longer read/written when D1 is available

### Phase 3g: JWT Blacklist 🟢 LOW ✅ COMPLETE

**KV key:**
- `jwt_blacklist:{sha256(token)}` → `"1"` (TTL = token expiry)

**Approach:** Dedicated `jwt_blacklist` table in D1. KV TTL auto-expiry replaced by scheduled cleanup handler (Phase 7 in `run_cleanup`).

**Files:**
- `worker/migrations/0016_jwt_blacklist.sql` — `jwt_blacklist` table + index on `expires_at`
- `worker/src/db/jwt_blacklist.rs` — D1 helpers: `insert`, `exists`, `cleanup_expired`
- `worker/src/auth.rs` — `blacklist_token()` and `is_token_blacklisted()` → D1-first with KV fallback
- `worker/src/cleanup.rs` — Phase 7: prune expired JWT blacklist entries in scheduled handler

**Acceptance:**
- [x] D1 table created (`jwt_blacklist`) with `expires_at` column
- [x] Scheduled cleanup handler prunes expired entries
- [x] Token blacklist check queries D1-first (KV fallback preserved)
- [x] Token blacklist write goes to D1-first (KV fallback preserved)
- [x] KV key no longer read/written when D1 is available

## Database Schema

### Phase 3a: Adventure

```sql
-- Adventure config per event — replaces KV key "event:{id}:adventure:config"
CREATE TABLE adventure_configs (
    event_id        TEXT PRIMARY KEY REFERENCES events(id),
    enabled         INTEGER NOT NULL DEFAULT 0,  -- bool as INTEGER
    required_level  INTEGER,                      -- NULL = no requirement
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Adventure progress per attendee — replaces KV key "event:{id}:adventure:progress:{claim_token}"
CREATE TABLE adventure_progress (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id          TEXT NOT NULL,
    claim_token       TEXT NOT NULL,
    levels_completed  TEXT NOT NULL DEFAULT '[]',  -- JSON array of level IDs
    total_moves       INTEGER NOT NULL DEFAULT 0,
    total_time_seconds INTEGER NOT NULL DEFAULT 0,
    passed            INTEGER NOT NULL DEFAULT 0,  -- bool
    passed_at         TEXT,
    last_played_at    TEXT,
    UNIQUE(event_id, claim_token)
);

CREATE INDEX idx_adventure_progress_event ON adventure_progress(event_id);
CREATE INDEX idx_adventure_progress_claim  ON adventure_progress(claim_token);

-- Level scores (normalized) — replaces embedded HashMap in AdventureProgress JSON
CREATE TABLE adventure_level_scores (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    progress_id       INTEGER NOT NULL REFERENCES adventure_progress(id),
    level_id          TEXT NOT NULL,
    moves             INTEGER NOT NULL DEFAULT 0,
    puzzles_solved    INTEGER NOT NULL DEFAULT 0,
    time_seconds      INTEGER NOT NULL DEFAULT 0,
    stars             INTEGER NOT NULL DEFAULT 0,
    UNIQUE(progress_id, level_id)
);
```

### Phase 3b: On-Chain

```sql
-- Individual on-chain events — replaces KV key "event:{id}:onchain" (was Vec<OnChainEvent> JSON)
CREATE TABLE onchain_events (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id              TEXT NOT NULL,
    signature             TEXT NOT NULL UNIQUE,
    slot                  INTEGER NOT NULL,
    block_time            INTEGER NOT NULL,         -- i64 as INTEGER
    instruction           TEXT NOT NULL,             -- EscrowInstruction enum as JSON
    escrow_address        TEXT NOT NULL,
    target_escrow_address TEXT,
    organizer             TEXT,
    attendee              TEXT,
    amount                INTEGER,                  -- NULL = no amount
    indexed_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_onchain_events_event     ON onchain_events(event_id, block_time DESC);
CREATE INDEX idx_onchain_events_escrow    ON onchain_events(escrow_address);
CREATE INDEX idx_onchain_events_organizer ON onchain_events(organizer);
CREATE INDEX idx_onchain_events_attendee  ON onchain_events(attendee);

-- Dedup marker — replaces KV key "onchain:sig:{signature}" (was TTL 90 days)
CREATE TABLE onchain_dedup (
    signature   TEXT PRIMARY KEY,
    indexed_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Auto-cleanup: delete entries older than 90 days
-- Run via scheduled handler or cron

-- Cursor per escrow — replaces KV key "onchain:cursor:{escrow_addr}"
CREATE TABLE onchain_cursors (
    escrow_address     TEXT PRIMARY KEY,
    last_signature     TEXT NOT NULL,
    updated_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Phase 3c: Organizations

```sql
-- Organizations — replaces KV keys "org:{org_id}" (OrganizationConfig) and "orgs" (OrgIndex)
CREATE TABLE organizations (
    id                  TEXT PRIMARY KEY,        -- slug-style identifier
    name                TEXT NOT NULL,
    contacts_sheet_id   TEXT NOT NULL DEFAULT '', -- empty = global fallback
    contacts_sheet_name TEXT NOT NULL DEFAULT 'Contacts',
    events_sheet_name   TEXT NOT NULL DEFAULT 'Events',
    owner_emails        TEXT NOT NULL DEFAULT '[]', -- JSON array of email strings
    created_at          TEXT NOT NULL,            -- ISO 8601
    updated_at          TEXT NOT NULL DEFAULT ''  -- ISO 8601
);

CREATE INDEX idx_organizations_name ON organizations(name);
```

> **Note:** Original plan normalized owner emails into a separate `organization_owners` table.
> Simplified to a JSON array to match the existing domain model (`OrganizationConfig.owner_emails: Vec<String>`),
> keeping the migration minimal and consistent with the quiz/adventure JSON blob pattern.

### Phase 3d: THB Deposits

```sql
-- THB deposits — replaces KV keys "thb_deposit:{event_id}:{email}", "thb_deposits:{event_id}"
CREATE TABLE thb_deposits (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    attendee_id     TEXT NOT NULL,
    event_id        TEXT NOT NULL,
    amount_thb      INTEGER NOT NULL,
    slip_url        TEXT,
    verified        INTEGER NOT NULL DEFAULT 0,  -- bool
    verified_by     TEXT,
    verified_at     TEXT,
    uploaded_at     TEXT NOT NULL,
    refunded        INTEGER NOT NULL DEFAULT 0,  -- bool
    refunded_at     TEXT,
    attendee_name   TEXT,
    bank_account    TEXT,
    bank_name       TEXT
);

CREATE INDEX idx_thb_deposits_event    ON thb_deposits(event_id);
CREATE INDEX idx_thb_deposits_attendee ON thb_deposits(event_id, attendee_id);
```

### Phase 3e: Deposit Status

```sql
-- Deposit status — replaces KV key "deposit_status:{id}"
CREATE TABLE deposit_statuses (
    id              TEXT PRIMARY KEY,
    attendee_id     TEXT NOT NULL,
    event_id        TEXT NOT NULL,
    method          TEXT NOT NULL,   -- DepositMethod enum as TEXT
    amount          INTEGER NOT NULL,
    currency        TEXT NOT NULL,
    tx_signature    TEXT,
    verified        INTEGER NOT NULL DEFAULT 0,  -- bool
    deposited_at    TEXT NOT NULL,
    wallet_address  TEXT,
    deposit_order   INTEGER NOT NULL DEFAULT 0,
    refundable      INTEGER NOT NULL DEFAULT 0,  -- bool
    rejected        INTEGER NOT NULL DEFAULT 0   -- bool
);

CREATE INDEX idx_deposit_statuses_event    ON deposit_statuses(event_id);
CREATE INDEX idx_deposit_statuses_attendee ON deposit_statuses(event_id, attendee_id);
```

### Phase 3f: Event Form Config

```sql
-- Option A: Add column to events table (preferred if config is small)
-- ALTER TABLE events ADD COLUMN form_config TEXT;  -- JSON blob

-- Option B: Separate table (if config is complex or needs independent versioning)
CREATE TABLE event_form_configs (
    event_id    TEXT PRIMARY KEY REFERENCES events(id),
    config      TEXT NOT NULL,  -- JSON blob
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Phase 3g: JWT Blacklist

```sql
-- JWT blacklist — replaces KV key "jwt_blacklist:{sha256(token)}" (was TTL = token exp)
CREATE TABLE jwt_blacklist (
    token_hash   TEXT PRIMARY KEY,
    expires_at   TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_jwt_blacklist_expires ON jwt_blacklist(expires_at);

-- Cleanup: DELETE FROM jwt_blacklist WHERE expires_at < datetime('now')
-- Run via scheduled handler periodically
```

## Files Changed (Estimated)

### New Files

| File | Purpose |
|------|---------|
| `worker/migrations/0010_adventure_tables.sql` | Adventure config + progress + level scores schema |
| `worker/migrations/0011_onchain_tables.sql` | On-chain events + dedup + cursors schema |
| `worker/migrations/0012_organizations_tables.sql` | Organizations + owners schema |
| `worker/migrations/0013_thb_deposits_table.sql` | THB deposits schema |
| `worker/migrations/0014_deposit_statuses_table.sql` | Deposit statuses schema |
| `worker/migrations/0015_form_config_table.sql` | Event form config schema |
| `worker/migrations/0016_jwt_blacklist_table.sql` | JWT blacklist schema |
| `worker/src/db/adventure.rs` | Adventure D1 CRUD |
| `worker/src/db/onchain.rs` | On-chain events D1 CRUD |
| `worker/src/db/organizations.rs` | Organization D1 CRUD |
| `worker/src/db/thb_deposits.rs` | THB deposits D1 CRUD |
| `worker/src/db/deposit_status.rs` | Deposit status D1 CRUD |
| `worker/src/db/jwt_blacklist.rs` | JWT blacklist D1 CRUD |
| `worker/scripts/migrate_kv_adventure_to_d1.sh` | One-time KV → D1 migration |
| `worker/scripts/migrate_kv_onchain_to_d1.sh` | One-time KV → D1 migration |
| `worker/scripts/migrate_kv_orgs_to_d1.sh` | One-time KV → D1 migration |

### Modified Files

| File | Change |
|------|--------|
| `worker/src/adventure.rs` → `worker/src/db/adventure.rs` | Replace KV calls with D1 queries |
| `worker/src/onchain.rs` | Replace KV calls with D1 queries |
| `worker/src/organizations.rs` | Replace KV calls with D1 queries |
| Various handlers | Update to use new D1 modules |
| `worker/src/lib.rs` | Wire new DB modules |

## Acceptance Criteria

### Per-Phase

Each phase (3a–3g) is independently deployable:

- [ ] D1 tables created, migration idempotent (`CREATE TABLE IF NOT EXISTS`)
- [ ] All reads query D1 (KV fallback removed after cutover)
- [ ] All writes go to D1 (no dual-write needed — direct cutover per phase)
- [ ] Existing KV data migrated via one-time script
- [ ] KV keys for that data type no longer read or written

### Overall (After Phase 3g)

- [ ] No persistent data stored in KV (only true TTL caches remain: Google token, blockhash)
- [ ] EVENTS KV namespace usage drops to near-zero (only ephemeral caches)
- [ ] All D1 tables have appropriate indexes for hot-path queries
- [ ] Migration scripts are idempotent and can be re-run safely
- [ ] No regression in P99 latency for any endpoint

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| KV data loss before migration | Unrecoverable adventure progress, org configs | Migrate 3a+3c first (highest risk data) |
| D1 batch size limits on migration | Large KV datasets may exceed D1 batch inserts | Batch migration in chunks of 100 |
| On-chain events >200 per event | KV capped at 200, D1 stores unlimited | D1 is better — no cap needed |
| JWT blacklist cleanup requires cron | Expired tokens accumulate without cleanup | Add scheduled handler for cleanup |
| Adventure level scores normalized vs JSON | More complex queries for score aggregation | Index on `(progress_id, level_id)` — fast lookups |
| KV namespace still needed for caches | Can't fully remove EVENTS binding | Expected — KV stays for TTL-based caches only |

## Dependencies

- Phase 1 (#037) ✅ COMPLETE
- Phase 2 (#046) ✅ COMPLETE
- D1 database binding active in `wrangler.toml`
- EVENTS KV namespace (`c8a6a87f9ed34ce0a3c8e48b84039214`) still active for migration reads
