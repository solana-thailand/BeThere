# Issue #046: D1 as Primary Data Store (Phase 2 — Attendees, Contacts, Events, Staff)

> **Status: 🔄 Phase 2a-2c DEPLOYED & VALIDATED** — Phase 2d (remove KV caching) remaining
> Prerequisite: Issue #037 (Phase 1 — claim locks + audit trail) ✅ COMPLETE

## Summary

Migrate from Google Sheets as primary data store to D1 for attendees, contacts, events, and staff. Google Sheets becomes an async-synced reporting layer. This eliminates Sheets API latency (50-300ms per call) from the hot path of check-ins, claims, and lookups.

## Motivation

### Current Pain Points

| Problem | Impact | Frequency |
|---------|--------|-----------|
| Sheets API on every check-in write | 200ms+ added latency | Every check-in |
| Full sheet fetch on cache miss | 300ms for large sheets | Every KV TTL expiry (~60s) |
| Column mapping resolution | Extra Sheets API call per cache miss | Every cache miss |
| Contact upsert = find + update | 2 sequential Sheets API calls | Every registration |
| KV cache invalidation complexity | 3+ KV delete calls per mutation | Every write |
| No transactional guarantees | Partial writes possible | Any concurrent mutation |

### Latency Targets After Migration

| Operation | Current | Target | Improvement |
|-----------|---------|--------|-------------|
| Check-in write | ~200ms (Sheets) | ~5ms (D1) | **40x** |
| Attendee lookup (cache miss) | ~300ms (full fetch) | ~5ms (D1 indexed) | **60x** |
| Claim token lookup | ~100ms (KV deserialize) | ~3ms (D1 indexed) | **30x** |
| Contact upsert | ~250ms (find + update) | ~5ms (SQLite UPSERT) | **50x** |
| Staff list load | ~150ms (Sheets read) | ~3ms (D1 query) | **50x** |

## Scope

### In Scope (This Issue)

| Component | Current Storage | Target | Tables |
|-----------|----------------|--------|--------|
| Attendees | Google Sheets (per-event) | D1 | `attendees` |
| Contacts | Google Sheets (master Contacts) | D1 | `contacts` |
| Events | KV + Google Sheets (Events tab) | D1 | `events` |
| Staff | Google Sheets (per-event) | D1 | `staff` |

### Out of Scope (Future Issues)

| Component | Reason |
|-----------|--------|
| Quiz questions | KV is fine — infrequent writes, TTL-based reads |
| Adventure progress | KV is fine — per-user state, TTL-based |
| Organizations | KV is fine — small index, infrequent changes |
| Google OAuth token | KV is fine — TTL-based true cache |
| R2 assets | Blob storage — no change needed |

### After Migration: What Stays in KV

| KV Key | Purpose | Why KV |
|--------|---------|--------|
| `google_token` | OAuth access token | TTL-based cache (3500s) |
| `quiz:{event_id}` | Quiz questions | TTL-based, infrequent writes |
| `adventure:{event_id}` | Adventure config | TTL-based, infrequent writes |
| `org:{org_id}` | Organization config | Small, infrequent writes |

### After Migration: What Stays in Google Sheets

Sheets becomes an **async-synced reporting layer**:
- Organizers who prefer spreadsheets still see live data
- Synced via `wait_until()` — non-blocking, best-effort
- Sheet is no longer source of truth — D1 is

## Migration Phases

### Phase 2a: Schema + Dual-Write (Low Risk)

Add D1 tables. After every Sheets write, also write to D1. No read path changes.

**Files changed:**
- `worker/migrations/0002_attendees_contacts_events.sql` — new schema
- `worker/src/db.rs` — add typed query functions for all new tables
- `worker/src/sheets/write.rs` — add D1 writes alongside existing Sheets writes

**Acceptance:**
- [x] D1 tables created, migration idempotent
- [x] Every check-in writes to both Sheets and D1
- [x] Every registration writes to both Sheets and D1
- [ ] D1 data matches Sheets data after smoke test

### Phase 2b: D1-First Reads

Change read paths to query D1 first, fall back to Sheets on error.

**Files changed:**
- `worker/src/sheets/mod.rs` — `get_attendees()` queries D1, Sheets as fallback
- `worker/src/handlers/checkin.rs` — direct D1 attendee lookup
- `worker/src/handlers/claim.rs` — direct D1 claim token query
- `worker/src/handlers/attendee.rs` — D1-backed listing
- `worker/src/contacts.rs` (sheets) — D1-backed contact operations
- `worker/src/sheets/mod.rs` — `get_staff_members()` queries D1

**Acceptance:**
- [x] Check-in reads from D1 (no Sheets API call on hot path)
- [x] Claim lookup queries D1 by indexed `claim_token`
- [x] Staff list loaded from D1
- [x] Fallback to Sheets works if D1 is empty (backward compat)

### Phase 2c: Sheets Async-Only

Move Sheets writes behind `wait_until()`. D1 is source of truth.

**Files changed:**
- `worker/src/sheets/write.rs` — wrap Sheets calls in `wait_until()`
- `worker/src/sheets/contacts.rs` — wrap Sheets calls in `wait_until()`
- `worker/src/sheets/events_tab.rs` — wrap Sheets calls in `wait_until()`

**Acceptance:**
- [x] Check-in returns before Sheets write completes
- [x] Contact upsert returns before Sheets write completes
- [ ] Sheets eventually consistent with D1 (within seconds)
- [ ] No data loss if Sheets write fails (D1 is truth)

### Phase 2d: Remove KV Attendee Caching

Remove KV cache layer for attendees since D1 serves reads fast enough.

**Files changed:**
- `worker/src/sheets/mod.rs` — remove `get_attendees()` KV cache logic
- `worker/src/sheets/mod.rs` — remove `get_claim_map_cached()` (replaced by D1 query)
- `worker/src/sheets/mod.rs` — remove `invalidate_attendee_cache()` calls
- Various handlers — remove KV cache invalidation calls

**Acceptance:**
- [ ] No KV cache for attendee data
- [ ] Attendee list response time < 50ms (D1 query)
- [ ] Reduced KV usage (cost savings at scale)

## Database Schema

See `docs/d1_migration_architecture.md` for full schema with column documentation.

### Key Design Decisions

1. **`attendees.claim_token UNIQUE`** — SQLite enforces uniqueness; no separate `claim_locks` lookup needed for claim path
2. **`contacts.email PRIMARY KEY`** — lowercase, matches current dedup behavior
3. **`events.id PRIMARY KEY`** — slug-based (e.g. `solana-bangkok-2025`), matches current KV keys
4. **`staff (email, event_id) PRIMARY KEY`** — composite key, one role per staff per event
5. **No foreign key enforcement** — D1 doesn't support FK constraints; enforced at app layer
6. **`TEXT` for timestamps** — ISO 8601 strings, matches existing patterns in `claim_locks` and `audit_log`

## Data Seed (One-Time Import)

Before Phase 2b, existing Google Sheets data must be imported to D1:

```bash
# 1. Export attendees from current Sheets
# 2. Transform to INSERT statements
# 3. Import to D1
npx wrangler d1 execute bethere-db --file=worker/migrations/0003_seed_existing_data.sql
```

A seed script (`worker/scripts/seed_d1_from_sheets.ts`) should:
1. Read all events from KV
2. For each event, fetch attendees from Sheets
3. Transform to D1 INSERT statements
4. Batch-execute against D1

## Rollback Plan

Each phase is independently rollbackable:

| Phase | Rollback |
|-------|----------|
| 2a (dual-write) | Remove D1 writes — Sheets is still truth |
| 2b (D1-first reads) | Revert to Sheets-first reads |
| 2c (async Sheets) | Move Sheets writes back to synchronous |
| 2d (remove KV cache) | Re-add KV caching layer |

Use gradual deployment (see `docs/gradual_deploy_runbook.md`):
- Deploy to 10% → monitor error rates → 50% → 100%

## Pricing Estimate

For a 500-attendee event (same analysis as Issue #037):

| Operation | Rows/day | Within Free Tier? |
|-----------|----------|-------------------|
| Attendee reads (check-in, claim, list) | ~2,000 | ✅ (5M/day free) |
| Attendee writes (check-in, registration) | ~600 | ✅ (100K/day free) |
| Contact reads/writes | ~200 | ✅ |
| Event reads/writes | ~100 | ✅ |
| Staff reads | ~50 | ✅ |
| **Total** | ~3,000 | **$0/day** |

D1 storage: ~50MB for 10K attendees × 5 events. Well within 10GB limit.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| D1 schema migration breaks existing data | Low | High | Idempotent migrations (`IF NOT EXISTS`); test on dev D1 first |
| Sheets async sync fails silently | Medium | Medium | D1 is truth; Sheets is report; add sync health check endpoint |
| D1 latency spike under load | Low | Medium | D1 is edge-co-located; indexed queries < 10ms; monitor via `/health` |
| Data divergence between D1 and Sheets | Medium | Low | Periodic reconciliation script; Sheets is non-authoritative |
| Phase 2b fallback adds complexity | Medium | Low | Feature flag: `D1_PRIMARY_READS=true/false` in env vars |

## Dependencies

- Issue #037 (D1 Phase 1) — ✅ COMPLETE
- `worker` crate `d1` feature — ✅ Already enabled
- D1 binding in `wrangler.toml` — ✅ Already configured
- Gradual deploy infrastructure — ✅ Already documented

## Refs

- Architecture doc: `docs/d1_migration_architecture.md`
- Phase 1 issue: `.issues/037_d1_database_migration.md`
- Phase 1 handover: `.handovers/076_d1_database_migration_research.md`
- D1 docs: https://developers.cloudflare.com/d1/
- Workers database connectivity: https://developers.cloudflare.com/workers/databases/connecting-to-databases/
- Gradual deploy runbook: `docs/gradual_deploy_runbook.md`
