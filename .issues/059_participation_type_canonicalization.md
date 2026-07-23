# 059 — `participation_type` Canonicalization (write-path unification + backfill)

> Spin-off from #058. The no-show bug is fixed and the **read side** is now
> consolidated onto the `ParticipationType` enum (domain + worker, Tier A — done).
> This issue covers the **write side + stored data**, which is riskier and must
> not be done as a blind migration. Write-path unification (Step 3.2) shipped
> 2026-06-27; backfill (Step 3.3) shipped 2026-07-22.

## Status
- ✅ **Step 3.2 (write-path unification) DONE** — commit `7114c03`, deployed to
  `main` at `b432ac5` via Handover 121 (2026-06-27). No data change — D1 still
  holds legacy values until 3.3 backfill runs. But all NEW writes are now
  canonical; the mess stops growing.
- ✅ **Step 3.3 (backfill) DONE** — PR #25 merged to `develop` (`d6074ed`),
  migration `0024_attendees_participation_type_canonicalize.sql` applied to prod
  `bethere-db` (2026-07-22). **148 rows canonicalized, 0 data loss** verified:
  - Before: `online`×207, `Online`×111, `in_person`×86, `""`×21, `In-Person`×16, `test`×1 (total 442)
  - After:  `online`×318, `in_person`×123, `test`×1 (total **442 — unchanged**)
  - `walkin` rows: 0 (preservation clause was a harmless no-op).
  - Backup at `/tmp/bethere-backup-pre-0024.sql` (1.3M, 2790 INSERTs).
  - Pre-apply: local dry-run validated semantics (9-row seed across all variants
    → `in_person`×4, `online`×3, `test`/`walkin` untouched) + idempotency.
  - Post-apply: `wrangler d1 migrations list --remote` clean; `GROUP BY` shows
    only canonical values + the preserved `test` sentinel.
- ✅ **Step 3.4 (SQL predicate simplification) DONE** — PR #26 merged to `develop`
  (`4146261`, 2026-07-22). Three sites simplified (all mirror `Attendee::is_in_person`):
  `dashboard::IN_PERSON_PREDICATE` (const), `attendees::count_in_person_attendees`
  (inline dead_code), `contacts::audience_aggregate` (2 CASE WHEN blocks).
  Predicate: `(participation_type = 'in_person' OR TRIM(participation_type) = '')`.
  Dropped `IS NULL` (schema is `NOT NULL`) + the three `%in-person%`/`%in person%`/
  `%in_person%` LIKEs (no legacy variants post-backfill; write paths unified).
  Kept `OR TRIM(...) = ''` defensively to mirror `ParticipationType::parse("") == InPerson`
  so SQL and Rust classifier can never disagree. Walk-in detection unaffected
  (queries `= 'walkin'` literally, separate from these predicates). Gating condition
  verified pre-merge: prod has 0 non-canonical values and 0 empty/null rows.
  CI: domain tests 104 pass, worker tests 246 pass, clippy clean.
- ✅ **Worker redeployed** — prod worker `bethere` redeployed at Version
  `66622091-677b-4750-95f3-b17b914a5e8d` (2026-07-22, standard `wrangler deploy`).
  The simplified predicates from PR #26 are now live on prod. Frontend assets
  reused (no source changes since the 16:42 build, so `dist/` was current —
  deploy reported "No updated asset files to upload"). Smoke-tested: health 200,
  `/api/contacts/audience` (exercises the simplified `audience_aggregate`) 401
  (live + auth-gated), all protected routes 401 (not 404). D1 distribution
  unchanged (deploy doesn't touch data): 318 online, 123 in_person, 1 test.

## 1. Root cause (why prod is messy)
`attendees.participation_type` is written by **5+ independent paths using two
different conventions**, so the column can never stay canonical:

| Write path | File | Convention written |
|---|---|---|
| Sheet→D1 sync | `worker/src/handlers/events/sync.rs::normalize_participation_type` | snake_case `in_person` / `online` |
| Registration | `worker/src/handlers/register.rs::resolve_participation_type` | display-case `In-Person` / `Online` |
| Manual override | `worker/src/handlers/attendee.rs` (`ALLOWED_PARTICIPATION_TYPES = ["In-Person","Online"]`) | display-case `In-Person` / `Online` |
| Deposit auto-switch / slip-upload | `worker/src/handlers/deposit/{usdc,thb}/…` (sheet write-back) | display-case `Online` |
| Walk-ins | `worker/src/claim/mint.rs` | `"In-Person"` **or** the sentinel `"walkin"` |

Two compounding facts make a naive backfill wrong:
1. **`"walkin"` is an overloaded sentinel** — queried literally
   (`participation_type = 'walkin'`) in `db/attendees.rs` and `claim/mint.rs` to
   detect walk-ins. Canonicalizing it away would break walk-in detection.
2. **The Sheet is two-way synced** and organizers read it — write-back paths emit
   display-case `"In-Person"`/`"Online"`. Canonicalizing D1 alone would desync it
   from the Sheet.
3. **`upsert_attendee_full` overwrites unconditionally**
   (`participation_type = excluded.participation_type`), so a backfill is
   **partially undone on the next sync** unless every write path is unified first.

Prod distribution (re-verified during #058): `""`×21, `In-Person`×16, `Online`×109,
`in_person`×66, `online`×53, `test`×1.

## 2. Already done (Tier A — read side, no data change)
- `ParticipationType` enum in `domain/src/models/attendee.rs` (`InPerson`/`Online`/`Other`,
  canonical `as_str()` = `in_person`/`online`/`other`, `parse()` handles all prod
  variants incl. `physical`).
- `Attendee::is_in_person()` delegates to the enum (behavior preserved).
- Worker read-side matchers consolidated onto the enum:
  `register.rs::is_online_participation`, the `enforce_capacity` inline check
  (also fixes a latent bug where canonical `in_person`/`physical` rows were
  mis-counted as not-in-person).
- Unit tests added (none existed before).
- **Not changed (intentionally):** any write path, any stored value, the Sheet.

## 3. Proposed design (the part needing sign-off)

### 3.1 Pick one canonical storage convention
**Recommendation: store snake_case** (`in_person` / `online` / `walkin`) in D1,
matching the existing `normalize_participation_type` and the `EventFormat`
`as_str()` convention. Display-case ("In-Person") lives only at the **presentation
boundary** (Sheet cells + UI labels), never in D1.

### 3.2 Unify ALL write paths to 3.1 (must precede backfill)
- `register.rs::resolve_participation_type` → return `in_person`/`online`.
- `attendee.rs` manual override → accept & store `in_person`/`online`
  (`ALLOWED_PARTICIPATION_TYPES` updates; frontend sends the canonical token).
- `events/sync.rs::normalize_participation_type` → delegate to
  `ParticipationType::as_str()` (already close; preserve `walkin`/`test` pass-through).
- Deposit auto-switch / slip-upload sheet write-back → keep writing display-case to
  the **Sheet** (organizer-facing), but store canonical in **D1**.
- Walk-ins → standardize on `"walkin"` sentinel in D1 (do NOT map to `in_person`;
  walk-in detection depends on the literal). Decide whether the Sheet shows
  "Walk-in" or "In-Person".
- **Decision needed:** does the frontend participation-type toggle
  (`api::update_participation_type`) send `in_person`/`online` (canonical) or keep
  `In-Person`/`Online`? Frontend has its own `utils::is_in_person` /
  `get_participation_badge` copies (WASM, can't share the domain enum directly) —
  these should derive a **display label** from the canonical value rather than
  substring-match.

### 3.3 Backfill (only after 3.2 is deployed and stable)
One-time D1 migration, **guarded and idempotent**, run after a verified backup:

```sql
-- BACK UP bethere-db FIRST (npx wrangler d1 export bethere-db --remote --output …).
-- Preserve walk-ins; canonicalize only the ambiguous in-person/online/empty/junk.
UPDATE attendees SET participation_type = 'in_person'
 WHERE participation_type IS NULL
    OR TRIM(participation_type) = ''
    OR LOWER(participation_type) LIKE '%in-person%'
    OR LOWER(participation_type) LIKE '%in person%'
    OR LOWER(participation_type) LIKE '%in_person%'
    OR LOWER(participation_type) LIKE '%physical%';

UPDATE attendees SET participation_type = 'online'
 WHERE LOWER(participation_type) LIKE '%online%'
    OR LOWER(participation_type) LIKE '%virtual%';

-- Leave 'walkin' and 'test' untouched (test is 1 row — manual review).
```

Notes:
- Ordering matters: run the `online` UPDATE **after** the in-person one only if no
  value contains both tracks (none do in prod). Safe given current data.
- The empty→`in_person` mapping is a judgment call (legacy default); acceptable
  because `is_in_person` already treats empty as in-person.
- After backfill, the 3 SQL predicates (`dashboard.rs`, `db/attendees.rs`,
  `contacts.rs`) can collapse to `participation_type = 'in_person'` (no `LIKE`).

### 3.4 Rollout sequence
1. Land 3.2 (write-path unification) behind the existing tests + new ones; deploy.
2. Observe one full sync cycle — confirm new writes are canonical.
3. Backup D1 → run 3.3 backfill → verify counts (`GROUP BY participation_type`
   should show only `in_person`/`online`/`walkin`).
4. Simplify the SQL predicates (optional, cleanup).
5. Frontend: switch display to label-derived (separate, low-risk).

## 4. Risks / open questions
- **`walkin` semantics** must survive both unification and backfill — it's a status
  marker, not a participation mode. Confirm walk-ins should remain distinct.
- **Sheet vs D1 divergence**: organizers read display-case in the Sheet. Need to
  agree the Sheet stays display-case while D1 is canonical.
- **Frontend copies** (`utils::is_in_person`, `get_participation_badge`,
  `claim::is_online_participant`) are independent WASM substring matchers — they
  keep working but should eventually derive from a canonical value.
- **No registration/capacity integration tests** exist; 3.2 changes the
  registration write value — add an e2e/integration test before deploying 3.2.

## 5. Why this is a separate approval
Tier A (read side) was safe: no stored data changed. Tier B changes **stored data,
the Google Sheet, and walk-in semantics**, and a backfill done before write-path
unification gets reverted by the next sync. It must be a deliberate, sequenced
release, not an autonomous migration.
