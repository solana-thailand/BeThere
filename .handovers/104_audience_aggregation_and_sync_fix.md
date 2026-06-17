# Handover 104 — Cross-Event Audience Aggregation + Sheet→D1 Sync Repair

> **Branch**: `feature/audience_aggregation` (5 commits ahead of `develop`, pushed to `origin`)
> **Status**: ✅ **Deployed to prod** (worker + frontend) + **PR #9 open** against `develop`.
> Visual verification of the orphan-warning toast still pending (operator click).
> **Commits**: `705397e → 5232d91` on `feature/audience_aggregation`
> **Predecessor**: handover #103 (refund gate + verified builds)
> **Created**: 2026-06-17 · **Updated**: 2026-06-17 (deploy complete + orphan guardrail + stale-dist fix)

---

## 1. What Happened

This branch delivers two related pieces of work that came out of the D1 migration tail,
plus a follow-up guardrail feature prompted by a real data anomaly found post-deploy:

1. **Cross-event audience aggregation** — a new `GET /api/contacts/audience` endpoint that
   dedupes the entire `attendees` table by `LOWER(email)` and returns per-email participation
   metrics (events joined, check-ins, approval/in-person/online counts, first/last registration
   timestamps, developer profile enrichment). Supports `format=csv` for direct browser download.
   This replaces the drift-prone denormalized `contacts.events_joined` CSV column with a fresh
   `GROUP BY` computed from real registration rows.

2. **Sheet→D1 sync repair** — the `POST /api/events/{id}/sync-sheet` handler was silently
   truncating data and aborting on `claim_token` UNIQUE collisions. Two bugs fixed:
   - **`db.exec` truncation** in the upsert path (`attendees.rs::upsert_attendee_full`) —
     migrated to the `prepare().bind_refs().run()` pattern used everywhere else in the module.
   - **`claim_token` collisions** in `sync.rs` — legacy/misaligned sheets (e.g. Luma imports
     where `claim_token` reads a garbage column) yielded repeated tokens that collided on the
     `UNIQUE` constraint and aborted the entire row. Added in-batch duplicate detection that
     nulls colliding tokens (the live check-in flow mints fresh tokens anyway; sync's job is
     identity + lifecycle, not historical claim tokens).

3. **Diagnostic surface** — added `first_error: Option<String>` to `SheetSyncResponse` because
   `tracing-wasm` buffers `warn!` and does not flush inside request handlers under
   `wrangler dev --remote`. Without this, a partial sync (e.g. 12 of 135 rows fail) shows
   `errors: 12` with zero ability to see why. Gated behind `skip_serializing_if = "Option::is_none"`
   so the happy-path response is byte-identical to before.

4. **Backfill script** — `scripts/` updated to backfill the first event (auto-minted JWT,
   corrected `sheet_id`, health check, idempotency). Event #1's config was permanently updated
   in production KV + D1 (`sheet_id → 1FMQi…`, `sheet_name → "Attendees"`).

5. **Orphan event_id guardrail (post-deploy follow-up)** — after the first prod audience
   export, one attendee (`ratchapon.poc@mahidol.ac.th`) was reported "missing". Investigation
   showed it was a **data anomaly, not a software bug**: the row exists in D1 under
   `event_id = solana-x-ai-builders-2`, but that ID is **not in the events registry**. The
   cross-event aggregation reads the `attendees` table directly (so the row shows up in the
   audience export), while the per-event admin dashboard reads from the events registry
   (so the event isn't selectable in the dropdown). Added a diff between audience-referenced
   event_ids and the registry, returning `unregistered_event_ids: Vec<String>` from the API
   and surfacing a warning toast in the admin dashboard so operators don't presume those
   attendees are missing.

---

## 2. Root Cause

### 2.1 `db.exec` truncation

The D1 worker binding's `.exec()` method has documented limitations with large/parameterized
queries — it's intended for simple DDL/DML, not multi-column upserts with many bound values.
The sync upsert path was hitting this limitation, causing silent data truncation. Every other
write in `attendees.rs` already used `prepare().bind_refs().run()`; the sync upsert was the
outlier. The fix aligns it with the rest of the module.

### 2.2 `claim_token` UNIQUE collisions

The `attendees.claim_token` column has a `UNIQUE` constraint. Sheet imports for legacy events
(Luma, etc.) populated `claim_token` from a column that — for those imports — contained
repeated/garbage values. When two rows in the same batch shared a token, the second upsert
violated the constraint and the whole row was skipped (counted as an `error`). This made
full-event syncs silently incomplete.

The fix: detect duplicate tokens within the batch (a `HashSet` of tokens appearing >1 time),
null out the colliding tokens before upsert, and let the live check-in flow mint fresh tokens
for those attendees. Sync is idempotent and re-runnable, so a partial fix now + re-run later
is safe.

### 2.3 Error visibility (`first_error`)

`tracing-wasm` (used under `wrangler dev --remote`) buffers `warn!`/`error!` spans and does
not flush them inside the request handler's lifetime. So per-row sync failures logged via
`tracing::warn!` were invisible to the operator during development — the only signal was the
`errors: N` count in the response. Adding `first_error` to the response body is the
minimum-surface way to surface at least one failure's cause without changing the logging
stack.

### 2.4 Orphan event_id anomaly (data hygiene, post-deploy)

The audience endpoint and the per-event admin dashboard read from **different sources**:
the audience path queries `attendees` directly (source of truth for registration rows),
while the admin event selector reads from the events registry (`KV → D1 fallback`). If a
row in `attendees` carries an `event_id` that isn't in the registry — e.g. a Sheet-imported
event that was later renamed/deleted, like `solana-x-ai-builders-2` — it will appear in
audience exports but be **completely invisible** in the admin UI. The fix is not to "repair"
the orphan (the data is valid; the event just isn't registered) but to **surface** the
mismatch so operators don't chase phantom missing attendees. `registered_event_ids()` is
non-fatal: if neither KV nor D1 yields the registry, it returns an empty set and the
audience endpoint still serves rows (the orphan warning simply won't fire).

### 2.5 Stale `dist/` cache trap (almost shipped stale frontend)

Caught pre-deploy: `trunk serve` (dev mode) was serving current code, but the release
`dist/` directory held a stale WASM binary built hours earlier. Root cause: the global
cargo target redirect (`~/.cargo/config.toml` → `~/.cargo/target/`) had cached a stale
release fingerprint, so `trunk build --release` skipped recompilation. Fixed with a
surgical `cargo clean -p event-checkin-frontend` and rebuild. **Two verification lessons:**

1. **Verify against `*_bg.wasm`, not the `.js` shim.** String literals live in the WASM
   data section; grepping `dist/*.js` for a Rust string will fail even when the build is
   current. Always grep `dist/*_bg.wasm`.
2. When in doubt, nuke the per-crate fingerprint with `cargo clean -p <crate>` rather
   than a full workspace clean — faster and still authoritative.

---

## 3. Changes Made

### 3.1 Audience aggregation endpoint

**`worker/src/db/contacts.rs`**

- `AudienceRow` struct — one row per distinct email, with participation metrics + developer
  profile enrichment (`#[serde(default)]` on optional fields so NULL cells map cleanly).
- `audience_aggregate(db, event_ids: Option<&[String]>)` — builds the
  `WHERE a.event_id IN (?, ?, ...)` clause + bind list dynamically, runs the aggregate query,
  deserializes via `serde_json::from_value`. Uses `safe_all_rows` for execution.

**`worker/src/handlers/contacts.rs`**

- `GET /api/contacts/audience?event_ids=a,b&format=csv`
- `AudienceQuery` — `event_ids` (comma-separated, omit/empty ⇒ all events), `format`
  (`"csv"` triggers CSV attachment).
- `AudienceResponse` — `total`, `rows`, plus `csv`/`filename` (only when `format=csv`, gated
  by `skip_serializing_if`).
- `audience_handler` — parses `event_ids`, calls `audience_aggregate`, optionally builds CSV
  via `build_audience_csv` + `audience_csv_filename`.
- `build_audience_csv` — column order mirrors `AudienceRow` field order, header names chosen
  for spreadsheet readability.
- `escape_csv` — quotes/escapes fields containing commas/quotes/newlines.

### 3.2 Sheet→D1 sync repair

**`worker/src/handlers/events/sync.rs`**

- `SheetSyncResponse` — added `first_error: Option<String>` with doc comment explaining the
  tracing-wasm rationale.
- `sync_sheet_to_d1` — added `dup_tokens` HashSet construction (counts `claim_token`
  occurrences >1), per-attendee `needs_null_token` check, clones row with `claim_token = None`
  when colliding, captures first error verbatim into `first_error`.
- Early-return path (empty sheet) sets `first_error: None`.

**`worker/src/db/attendees.rs`**

- `upsert_attendee_full` — migrated from the truncating path to `prepare().bind_refs().run()`,
  aligning with every other write in the module.

### 3.3 Backfill script

**`scripts/`** — backfill for event #1 with:

- Auto-minted JWT for staff auth.
- Corrected `sheet_id` (`1FMQi…`) and `sheet_name` (`"Attendees"`).
- Health check before sync.
- Idempotency (safe to re-run).

### 3.4 Orphan event_id guardrail (post-deploy, commit `5232d91`)

**`worker/src/handlers/contacts.rs`**

- `AudienceResponse` — added `unregistered_event_ids: Vec<String>` with
  `#[serde(skip_serializing_if = "Vec::is_empty")]`. Empty in the happy path, so the
  response is byte-identical when every referenced event is registered.
- `audience_handler` — after the aggregate query, collects every referenced `event_id`
  (split from the per-row `GROUP_CONCAT(DISTINCT event_id)` CSV), diffs against
  `registered_event_ids(&state).await`, sorts the diff, and returns it in the response.
  Non-fatal: if the registry can't be read, returns an empty set and the endpoint still
  serves rows.
- `registered_event_ids(state)` — new helper. Mirrors the resolution used in
  `list_events` / `sync_contacts_handler`: KV (`event_store::get_event_index`) first,
  then D1 (`db::events::list_events_as_meta`) fallback, then `Default::default()`.

**`frontend-leptos/src/api/contacts.rs`**

- `AudienceResponse` — added `unregistered_event_ids: Vec<String>` with
  `#[serde(default)]` so older responses still deserialize.

**`frontend-leptos/src/pages/admin.rs`**

- `handle_audience_export` — on success, if `unregistered_event_ids` is non-empty, shows
  a `ToastType::Warning` with the count, a preview of up to 3 IDs, and a `+N more`
  suffix. Wording explicitly tells the operator the attendees are visible in this
  cross-event view but not in per-event views.

---

## 4. Files Modified

| File                                  | Change                                                                                                                                                                                  |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `worker/src/db/contacts.rs`           | + `AudienceRow`, `audience_aggregate`                                                                                                                                                   |
| `worker/src/handlers/contacts.rs`     | + `AudienceQuery`, `AudienceResponse`, `audience_handler`, `build_audience_csv`, `escape_csv`, `audience_csv_filename`; + `unregistered_event_ids` field, `registered_event_ids` helper |
| `worker/src/handlers/events/sync.rs`  | + `first_error` field, `dup_tokens` logic, claim_token nulling                                                                                                                          |
| `worker/src/db/attendees.rs`          | `upsert_attendee_full` migrated to `prepare().bind_refs().run()`                                                                                                                        |
| `scripts/`                            | backfill script (event #1, JWT, sheet_id, idempotency)                                                                                                                                  |
| `frontend-leptos/src/api/contacts.rs` | + `unregistered_event_ids` field (default-empty for back-compat)                                                                                                                        |
| `frontend-leptos/src/pages/admin.rs`  | + orphan warning toast in `handle_audience_export`                                                                                                                                      |

Commits (on `feature/audience_aggregation`, all pushed to `origin`):

- `705397e feat: cross-event audience aggregation endpoint + admin export`
- `4e73933 feat(scripts): backfill first event with auto-minted JWT`
- `6bc031d fix(worker): repair sheet→D1 sync — db.exec truncation + claim_token collisions`
- `a152501 fix(scripts): correct backfill sheet_id, health check, and idempotency`
- `317f93f docs(handover): add #104 — audience aggregation + sync repair`
- `5232d91 feat(contacts): orphan event_id guardrail on audience export`

---

## 5. Reflections

### What went well

- The audience aggregation query is a single `GROUP BY LOWER(email)` over the `attendees`
  table — the source of truth — so it doesn't inherit drift from the denormalized
  `contacts.events_joined` CSV column. Bypassing that column was a deliberate correctness call.
- The `first_error` diagnostic is the minimum-surface change that solves the observability
  hole: one `Option<String>`, gated by `skip_serializing_if`, documented in-place. No logging
  stack changes, no client breakage.
- The `claim_token` collision fix is defensive and well-commented — the dup detection HashSet
  - nulling-on-collision pattern is documented with the rationale ("live check-in flow mints
    fresh tokens anyway").
- Clippy on the worker lib is clean (`cargo clippy -p worker --quiet` → exit 0), matching
  the #094 verified state.

### What was struggled with

- **`cargo clippy --all-targets` false alarm**: running clippy with `--all-targets` fails
  because the `worker` crate's test code (and the `worker-0.8.1` dependency's test code)
  references `wasm_bindgen_test` / `trybuild`, which don't resolve when clippy compiles wasm
  test crates on the host target. This is a known wasm/host target mismatch under clippy, not
  a real error. The verified command is `cargo clippy -p worker --quiet` (no `--all-targets`)
  — that's what #094 used and what's clean now.
- **`pkill` self-match**: `pkill -f "deploy.sh dev"` matched the shell running the pkill
  command itself (its argv contained the search string), so the first attempt did not take
  down the stray worker. Fixed by killing the known PIDs directly
  (`kill 48073 48080 48102 48108 48120`).

### What was solved

- Audience export endpoint shipped, CSV download path wired, filename logic scoped
  (all-events vs. scoped).
- Sync repair verified at the build level (clippy clean); the `dup_tokens` + `first_error`
  combination means future partial syncs are both more complete (fewer false errors) and more
  diagnosable (first failure visible in the response body).
- Stray `dev --remote` worker process killed (was PID 48073 + wrangler children).
- **Production deploy complete**: worker (sync fix + audience API + orphan guardrail) and
  frontend rebuilt after clearing the stale dist cache, then uploaded to
  `https://bethere.solana-thailand.workers.dev` via `worker/deploy.sh`.
- **Orphan anomaly resolved (data, not code)**: `ratchapon.poc@mahidol.ac.th` was confirmed
  present in D1 under `event_id = solana-x-ai-builders-2`; the root cause was an unregistered
  event_id, not a query bug. The new guardrail surfaces this class of issue to the operator
  on every audience export.
- **Stale `dist/` trap caught pre-deploy**: `cargo clean -p event-checkin-frontend` + rebuild
  restored a current WASM binary. Verification switched from grepping `*.js` to grepping
  `*_bg.wasm` (string literals live in the WASM data section).
- **PR #9 opened** against `develop` with all 5 feature/fix commits.

---

## 6. Remain Work

### Immediate (post-deploy verification, operator action)

- [ ] **Visual verification of orphan toast** — on prod admin dashboard, click
      "Export Audience (All Events)" and confirm the warning toast fires for
      `solana-x-ai-builders-2` (expected count: 1). This is the last unverified step.
- [ ] **Merge PR #9** — `feature/audience_aggregation` → `develop`. Rebase before merge
      per gitflow; no merge commits.

### Decision pending

- [x] **`first_error` diagnostic** — **KEPT and deployed**. It's harmless
      (`skip_serializing_if`), scoped to staff-only endpoints, and solves a real `tracing-wasm`
      observability hole. Confirmed working in prod.

### State to be aware of

- Event #1's config was **permanently changed** (`sheet_id → 1FMQi…`,
  `sheet_name → "Attendees"`) in production KV + D1. Intended, but worth noting for the audit
  trail.

### Optional follow-ups (carried over, not blocking)

- [ ] `AuditAction::AudienceExported` variant for structured audit logging.
- [ ] In-app audience table view (`get_audience()` JSON client is ready).
- [ ] Org-scoped audience filtering.
- [ ] Integration test for the sync path — would have caught the `db.exec` bug before it
      reached prod.
- [ ] E2E test for the D1 aggregate query.
- [ ] Reconcile or rename `solana-x-ai-builders-2` in the registry — either restore the
      registry entry (so it shows up in the admin event selector) or backfill-rename the
      attendee rows to the canonical id. Decision belongs to the event ops owner.
- [ ] Investigate `worker/deploy.sh` asset-size false positive (`6884 bytes` instead of
      the real JS payload size) — the underlying upload succeeds, but the post-deploy check
      is misleading. Likely the `grep -o 'event-checkin-frontend-[a-z0-9]*\.js'` regex
      doesn't match the current hash format.

---

## 7. How to Dev/Test

### Verify the audience endpoint locally

```bash
# Start the worker against remote D1 (reads prod data; safe for GET)
cd event-checkin/worker && bash deploy.sh dev --remote
# → http://localhost:8787

# As staff, hit the JSON endpoint:
curl -s -H "Cookie: <staff_session>" \
  "http://localhost:8787/api/contacts/audience" | jq '.data.total'
# Expect: 135 (or current distinct-email count)

# CSV download path:
curl -s -H "Cookie: <staff_session>" \
  "http://localhost:8787/api/contacts/audience?format=csv" | jq -r '.data.filename'
# Expect: "audience-all.csv"

# Scoped to specific events:
curl -s -H "Cookie: <staff_session>" \
  "http://localhost:8787/api/contacts/audience?event_ids=islanddao-v4-demo,other-event&format=csv"
```

### Verify the sync fix

```bash
# Trigger a sync for an event with known legacy claim_token collisions:
curl -X POST -H "Cookie: <staff_session>" \
  "http://localhost:8787/api/events/<event_id>/sync-sheet" | jq
# Expect: errors: 0 (or low), first_error: null, synced ≈ total_in_sheet
# Pre-fix: errors was high, rows silently dropped
```

### Clippy gate (use the verified command)

```bash
# CORRECT — matches #094 verified state, exit 0
cargo clippy -p worker --quiet

# WRONG — fails with wasm_bindgen_test errors in the worker-0.8.1 dep's test code
# (host-target clippy can't resolve wasm test crates). Do NOT use --all-targets here.
# cargo clippy -p worker --quiet --all-targets
```

### Deploy (with stale-dist guard)

```bash
# 1. Rebuild frontend cleanly. If trunk skips recompilation due to a stale
#    fingerprint under ~/.cargo/target, force it with a per-crate clean:
cd event-checkin/frontend-leptos
cargo clean -p event-checkin-frontend
bash build.sh

# 2. VERIFY the fresh build actually contains your latest Rust changes.
#    String literals live in *_bg.wasm (the .js shim won't contain them):
rg "unregistered_event_ids" dist/*_bg.wasm   # expect a hit
rg "solana-x-ai-builders" dist/*_bg.wasm     # sanity check (if applicable)

# 3. Deploy worker + assets:
cd ../worker && bash deploy.sh
# Note: the post-deploy asset-size check may print a misleadingly small number
# (e.g. "6884 bytes") even when the upload succeeded — verify via the URL instead.

# 4. Verify on prod:
# - GET /api/contacts/audience → expect 135 rows
# - POST /api/events/<id>/sync-sheet → expect errors: 0, first_error: null
# - In admin UI: Export Audience (All Events) → expect orphan warning toast
#   naming solana-x-ai-builders-2
```

---

## 8. Issues Ref

- Predecessor: handover #103 (refund gate + verified builds)
- Related (error-swallowing pattern): handover #094 (`.first().ok().flatten()` → `map_err()?`)
- Related (D1 migration): handover #088, #089 (D1 dual-write, KV quiz removal)
- Branch: `feature/audience_aggregation` (5 commits: `705397e → 5232d91`, pushed to `origin`)
- **PR**: #9 — `feature/audience_aggregation` → `develop` (open, pending review + merge)
- **Prod URL**: https://bethere.solana-thailand.workers.dev
- Remote: `git@github.com:solana-thailand/BeThere.git`
- Production state: worker + frontend both updated; orphan guardrail live.

---

## 9. Commit Plan

Commits already on `feature/audience_aggregation` (in order, oldest first):

1. `705397e feat: cross-event audience aggregation endpoint + admin export`
2. `4e73933 feat(scripts): backfill first event with auto-minted JWT`
3. `6bc031d fix(worker): repair sheet→D1 sync — db.exec truncation + claim_token collisions`
4. `a152501 fix(scripts): correct backfill sheet_id, health check, and idempotency`
5. `317f93f docs(handover): add #104 — audience aggregation + sync repair`
6. `5232d91 feat(contacts): orphan event_id guardrail on audience export`

**Status** (all done, only verification remains):

- ✅ Pushed: `git push -u origin feature/audience_aggregation`
- ✅ PR #9 opened: `feature/audience_aggregation` → `develop` (gitflow)
- ✅ Deployed worker + frontend to prod (`bash worker/deploy.sh`, with stale-dist clean)
- ⏳ Visual verify: orphan-warning toast on audience export (operator click)
- ⏳ Merge PR #9 into `develop` (rebase, no merge commit)

Per standing gitflow rule: rebase onto `develop` before merge, do not merge with a merge commit.
