# 020 — SQL parameter binding across `worker/src/db`

Status: **Phase 1 done and staging-verified** (on `develop`).
**Phase 2 done and runtime-verified against a local D1** (on
`feature/sql_binding_phase2`) — see §4.4 for what was driven and §6 for the
three pre-existing production bugs the run surfaced.

Follow-up to the item flagged but deliberately not fixed in
`016_campaign_create_ux.md` §"Not fixed, and still open".

## 1. Why

Every helper in `worker/src/db` built its SQL with `format!`, interpolating
values into single-quoted literals. Some fields were hand-escaped with
`.replace('\'', "''")`, most were not, and nothing enforced which was which.
`bind_refs` was already available and already used in
`db/contacts.rs` and `db/event_summaries.rs`, so this was drift, not a
missing capability.

Two distinct failure modes:

1. **Injection** on the unescaped free-text columns.
2. **Silent data loss.** `sync_event_to_d1` logs and swallows `upsert_event`
   errors (`event_store/write/index.rs:41` — "KV remains the source of truth").
   So an apostrophe in any *unescaped* column produced a SQL syntax error that
   never surfaced: the event saved to KV and quietly stopped syncing to D1.
   Read paths backed by D1 (e.g. the public past-events list) would then serve
   stale data with no error anywhere. This was the more likely real-world bug
   of the two — `community_links` labels, `nft_symbol`, `poster_url`,
   `video_url`, `sheet_name`, `promptpay_id` and `updated_by` were all
   interpolated raw.

## 2. Phase 1 — what changed

| Commit | Site | Was |
|---|---|---|
| `a3a5c64` | `campaigns::create_campaign` / `update_campaign` | `title`, `description`, `completion_criteria`, `reward_type`, `reward_config` interpolated raw |
| `6edc04e` | `events::upsert_event` | 37 string columns; 7 hand-escaped, 30 raw |
| `93364e3` | `events::save_form_config` | organizer form JSON, hand-escaped |
| `93364e3` | `events::get_event_raw` | lookup `value` from a URL path segment, raw |
| `93364e3` | `event_summaries::set_recap` | organizer markdown + image url, hand-escaped |
| `02f7e38` | `contacts::clear_contact_pii`, `developers::clear_developer_pii`, `developers::delete_developer_responses` | `email` in the `WHERE`, raw |

Design notes:

- **Numeric columns stay interpolated** in `upsert_event`. They are Rust
  `i64`/`bool`/`u32`, so they cannot carry SQL metacharacters, and
  `D1Type::Integer` only accepts `i32` — several columns (`event_start_ms`,
  `on_chain_event_id`) exceed that. Routing them through `D1Type::Real(f64)`
  would have changed the stored type for no safety gain. The `?` placeholders
  and the `args` array are therefore interleaved with `format!` slots; a
  comment above `args` states the ordering contract.
- All ad-hoc `.replace('\'', "''")` escaping is gone. It was *correct*
  (SQLite has no backslash escapes) but applied inconsistently, which is
  exactly the failure mode that let 30 columns through unescaped.
- `delete_developer_responses` now returns the real affected-row count from
  `run()`'s `meta.changes`; `exec` could not report it. The caller
  (`handlers/privacy.rs:219`) discards the value, so no behaviour change.

## 3. Verification

Deployed to `bethere-staging` and driven over HTTP with payloads carrying
`'`, `"` and `'); DROP TABLE …; --`, then read back — from the API and, for
events, directly out of D1 via `wrangler d1 execute --remote`:

- campaign create + update → values stored byte-exact, table intact
- event create → `name`, `location`, `tagline`, `description`, `nft_symbol`,
  `nft_name_template`, `poster_url`, `video_url`, `community_links` all
  round-trip byte-exact; numeric columns unchanged (`event_start_ms`
  `1788500000000` stored as INTEGER)
- `GET /events/{id}` with `id = x' OR 1=1--` → same response as any other
  missing id, no SQL error
- form-config PUT/GET → nested labels and options round-trip byte-exact
- recap PUT/GET → markdown with quotes round-trips; the `publish: false`
  path exercises the `D1Type::Null` branch
- column/value alignment in `upsert_event` checked mechanically: 57 columns,
  57 values, and the `args` order matches the `?` columns in sequence

All test rows were deleted afterwards.

**Not runtime-verified:** the three PDPA erasure queries (`02f7e38`). Their
`email` comes from the verified JWT claim, not from request input, so the
severity is defence-in-depth rather than an open vector, and reproducing it
would mean minting a token with a quoted email *and* erasing the staging
super-admin's own data. Compile-checked only.

## 4. Phase 2 — done on `feature/sql_binding_phase2` (NOT runtime-verified)

Branch: `feature/sql_binding_phase2`, off `develop` after Phase 1.
Kept off `develop` deliberately: `develop` is the staging-verified state, this
branch is compile-and-test-verified only. **No staging deploy was run for
Phase 2**, so nothing here has executed against a real D1.

### 4.1 Two corrections to the Phase 1 audit

The Phase 1 scope in §4 (as originally written) was wrong twice:

1. **The file glob missed code.** The audit ran `rg "'\{" worker/src/db/*.rs`,
   which does not recurse — it missed `db/attendees/` entirely — and it never
   looked outside `worker/src/db` at all. Real SQL is also built in
   `handlers/profile.rs`, `handlers/campaigns.rs` and `cleanup.rs`.
2. **"All residual sites are ids/emails" was false.**
   `handlers/profile.rs:215` (`update_my_profile`) interpolated
   `display_name`, `github_handle`, `discord_handle`, `twitter_handle`,
   `telegram_handle`, `primary_role`, `learning_goals`, `company_org` and
   `location_city` — user-typed free text on a *self-service* endpoint, the
   broadest input surface in the app. It was consistently escaped on every
   field, so it was not injectable, but it belonged in Phase 1's free-text
   bucket, not Phase 2's.

The corrected sweep is `rg "'\{" worker/src --type rust` filtered for SQL
keywords. After Phase 2 it returns only two hits, both error-message strings
(`org_store.rs:136`, `event_store/write/lifecycle.rs:93`).

### 4.2 A second broken escaper, found while converting

`db/event_summaries.rs:93` (`upsert_summary`) defined:

```rust
let esc = |s: &str| s.replace('"', "''");
```

It replaced **double** quotes with two single quotes. So it never escaped
apostrophes — the thing the adjacent comment said it was for — and it
corrupted any value containing `"`. Applied to `event_id`, `frozen_at` and
`frozen_by`. Reachable inputs are a slug and an admin email, so no live
exploit, but the function did the opposite of what it claimed. Deleted, not
repaired.

This is the second escaping defect in the same class as the inconsistent
`.replace` coverage in `upsert_event`, and it is the argument against
hand-escaping generally: a wrong escaper and a right one look identical at a
glance.

### 4.3 Converted in Phase 2

- `db/campaigns.rs` — all 28 remaining predicates (`get_campaign`,
  `campaign_exists`, `list_campaigns`, `update_campaign_status`,
  `delete_campaign`, the four `campaign_events` helpers, the five progress
  helpers, `mark_reward_claimed{,_with_mint}`, `totals_sql`, the drop-off
  query, the three `on_event_checkin` queries, `get_campaign_for_event`,
  `list_campaign_event_summaries`)
- `db/events.rs` — `get_form_config`, `set_recap_published_flag`,
  `set_post_event_registration`, `delete_event`
- `db/event_summaries.rs` — `upsert_summary`
- `db/attendees/management.rs` — `get_attendees_by_email`,
  `clear_attendee_pii`, `set_marketing_consent`
- `db/jwt_blacklist.rs` — `insert`, `exists`
- `handlers/profile.rs` — `update_my_profile` (12 bound fields)
- `handlers/campaigns.rs` — cascade progress delete
- `cleanup.rs` — `onchain_events` delete

Notes:

- `list_campaigns` builds its `WHERE` dynamically. Placeholders and args are
  now pushed onto two parallel `Vec`s in the same `if let`, so they cannot
  drift out of step.
- `totals_sql` changed from `fn(&str) -> String` to `fn() -> &'static str`.
  Its unit test (which guards the `COALESCE` fix for the stats 500) was
  updated to assert `WHERE campaign_id = ?`; the COALESCE assertion is
  untouched, which is the part that matters.
- `set_marketing_consent` previously read its row count from
  `D1ExecResult::count()`; it now reads `meta.changes` from `run()`. Same
  semantics, but this one **is** a behaviour change worth watching — the
  caller does use the count, unlike `delete_developer_responses`.
- Numeric values stay interpolated throughout, for the `D1Type::Integer`
  i32-range reason given in §2.
- `db/onchain_events.rs::cleanup_old_dedup_entries` still interpolates
  `days` into `'-{days} days'`. It is an `i64`; a negative value yields a
  malformed SQLite modifier that evaluates to NULL, not an injection. Left
  as-is.

### 4.4 Phase 2 runtime verification — done locally (2026-09-03)

`cargo clippy -D warnings` clean and 212 lib tests pass, but that only proves it
compiles. Every real defect in this workstream was found by running the code, so
Phase 2 got the same treatment Phase 1 got — except **locally**, not on staging,
because the standing instruction is that nothing leaves this machine:

    npx wrangler d1 migrations apply bethere-db --local
    npx wrangler dev --local --port 8788        # .dev.vars sets DEV_MODE=1

Real HTTP against `127.0.0.1:8788` with `Authorization: Bearer dev-token`,
payloads carrying `'`, `"` and `'); DROP TABLE …; --`, read back from the API
and from local D1 with `wrangler d1 execute --local`.

| Path | Result |
|---|---|
| campaign create / get / exists / update / status / delete | hostile free text round-trips byte-exact |
| `list_campaigns` — 0, 1 and 2 filters, matching and non-matching | dynamic `WHERE` binds correctly in every arity; `?status=x' OR 1=1--` returns empty, not everything |
| `set_campaign_events` / `list_campaign_events` | `event_id` values `ev-o'neil` and `ev2" x` both round-trip — the `"` case is exactly what the deleted `esc` in §4.2 corrupted |
| `campaign_stats` (`totals_sql` + drop-off) | correct with 0 and with 1 enrolment |
| `update_my_profile` (12 placeholders) | all 12 fields round-trip byte-exact, INSERT and ON CONFLICT paths |
| `set_marketing_consent` (`meta.changes`) | 0 with no matching row, 1 with one, and the row really flipped — the §4.3 behaviour change is correct |
| `jwt_blacklist::insert` / `exists` | logout with a real HS256 JWT inserts the hash; the next request on that token 401s "token has been revoked" |
| `upsert_summary` (3 placeholders in a 19-column INSERT) | column alignment proven with *distinguishable* values: `registered=1, checked_in=1, in_person_*=1`, the rest 0, and two distinct ms columns |
| `set_recap` / `set_recap_published_flag` | markdown with quotes round-trips; `publish:false` exercises the `D1Type::Null` branch |
| `save_form_config` / `get_form_config` | nested labels with `'` and `"` round-trip |
| `set_post_event_registration` | open + close |
| campaign cascade delete | campaigns, campaign_events and developer_campaign_progress all cleared |
| PDPA erasure (`get_attendees_by_email`, `clear_attendee_pii`, `clear_contact_pii`, developer erasure) | **found broken — see §6.2**; now verified erasing |
| cron `cleanup.rs` | **found broken — see §6.3**; now completes |

Not exercised locally: registration and the Sheets sync, which need a real
Google Sheet. Check-in was driven by seeding the attendee row directly into
local D1.

**Safety, learned the hard way (2026-09-04).** `--local` sandboxes D1, KV and R2
— **not** Google Sheets: `worker/.dev.vars` holds live service-account
credentials, and every request that resolves a role already reads the real staff
sheet. Grep a handler for `sheets::` before firing it. The trap is that a
handler can reach Sheets without saying so in its name: cleaning up fixture
events via `DELETE /api/events/{id}/delete` runs
`handlers::events::lifecycle::hard_delete_event`, which calls
`sheets::events_tab::delete_event_tab`. That call is guarded by
`if !resolved.sheet_id.is_empty()`, and `CONTACTS_SHEET_ID` is set in neither
`wrangler.toml` nor `.dev.vars`, so it was skipped — confirmed by the absence of
any `sheets/mod.rs` line in the dev log between "event hard-deleted" and "event
permanently deleted". That was luck, not design. Prefer
`wrangler d1 execute --local` for fixture teardown; if you do use the API,
diff the dev log for `sheets::` afterwards.

**`hard_delete_event` cascades only to `event_summaries` (fixed 2026-09-04,
`db5b2a1`).** It used to remove the KV entry and the `events` row only, which
orphaned the frozen funnel snapshot forever — `event_summaries` is keyed by
`event_id` with no FK to `events`, so nothing else would ever collect it.
`sync_delete_event_from_d1` now also calls `db::event_summaries::delete_summary`,
logging (not propagating) a failure so the summary delete cannot mask a
successful event delete.

`attendees`, `audit_log` and `credit_ledger` are still left behind **on
purpose** — they carry standalone record-keeping value that outlives the event.
For fixture teardown that means the delete is still not total: verify the tables
you seeded are actually empty afterwards rather than assuming.

Verified against local D1: created `cascade-probe` via `POST /api/events`,
seeded an `event_summaries` row directly, then
`DELETE /api/events/cascade-probe/delete?force=true` → both tables 0. The dev
log showed only the read-only staff-sheet fetch from role resolution between
"event hard-deleted" and "event permanently deleted" — no `events_tab` write.

### 4.5 Email validation — done

The original claim here ("no email validator exists anywhere") was wrong: a
real one, `is_plausible_email`, was private to `handlers::register::signup`.
The actual problem was inconsistency — registration used that check, walk-in
used `email.contains('@')`, and waitlist used `contains('@') && contains('.')`,
so the same address could be accepted at one endpoint and rejected at another.

Moved verbatim to `event_checkin_domain::validation::is_plausible_email` and
used by all three ingress points. Rules: length 3..=254, no whitespace, exactly
one `@` with a non-empty local part, and a domain containing a dot that is
neither leading nor trailing. Deliberately not RFC 5322 — it also rejects the
synthetic `wallet:<address>` identity, which must never be stored as a contact
email. Tests moved out of the `signup.rs` inline `mod tests` into
`domain/tests/email_validation.rs` (4 tests).

Verified live against `POST /api/waitlist`: `user@nodot`, `user@.com` and
`has space@example.com` → 400; `ok@example.com` → 200 (previously the first two
were accepted).

## 5. Out of scope, noticed while here

`GET /api/events/{id}` returned **500** for a missing event, not 404
(`"internal error: failed to read event: event '…' not found"`). **Fixed.**

`event_store::resolve_event{,_or_fallback}` returned `Result<_, String>`, and
every handler mapped that string to `AppError::Internal`, so asking for an event
that does not exist looked like an outage. They now return a typed
`ResolveError::{NotFound, Backend}` with `From<ResolveError> for AppError`, so a
miss is a 404 and only a genuine KV/D1 read or parse failure is a 500. Four call
sites updated (`handlers::ext` ×2, `handlers::events::read`,
`handlers::deposit::usdc`); this also fixes the quiz/adventure/claim paths, which
share `ext::resolve_event`.

Verified on local D1: `GET /api/events/does-not-exist` → `404 {"error":"not
found: event 'does-not-exist' not found"}`, a real event still `200`, and
`GET /api/quiz/questions?event_id=nope-nope` → `404` (was 500).

## 6. Three pre-existing production bugs the Phase 2 run surfaced

None of these are regressions — all three are present on `main` and are live in
production. All three were invisible to reading the code and to the test suite,
and all three were found by driving real requests.

### 6.1 `results::<T>()` panics on a narrow projection

`worker-0.8.1`'s `D1Result::results::<T>()` does
`serde_wasm_bindgen::from_value(result).unwrap()` per row. A struct with fields
the `SELECT` does not project is therefore **not** a recoverable `Err` — it is an
unconditional wasm panic, and the `match … Err(e) => warn!` arm around it is
dead code. (`first::<T>()` is fine; it propagates with `?`.)

Two sites had this:

- `on_event_checkin` — `SELECT DISTINCT campaign_id` deserialized into the
  4-field `CampaignEventRow`. It runs inside `ctx.wait_until`, so every check-in
  on an event belonging to a campaign panicked the isolate after the 200 had
  been sent. **Campaign auto-progress has never once worked**, and the panic
  also took down whatever else was queued in that `wait_until` — the Google
  Sheets `mark_checked_in` write shares it.
- `campaign_collection_mints` — `SELECT reward_config` into the 10-field
  `CampaignRow`. Reachable from two call sites in `handlers/wallet.rs`; panics
  as soon as one active `nft_certificate` campaign exists.

Fixed in `32eacb0` with dedicated `CampaignIdRow` / `RewardConfigRow`
projection structs. All 19 `.results::<T>()` sites were audited; the other 17
align with their projections.

After the fix, a check-in writes `developer_campaign_progress`
(`events_completed=1, total_required=1, is_complete=1`) and `my-progress`,
`/progress` and `/stats` all report it. That is the first time this feature has
produced a row.

### 6.2 Both PDPA erasure updates fail on NOT NULL and the failure is swallowed

- `clear_attendee_pii` set `contact_channel` and `contact_handle` to NULL; both
  are `TEXT NOT NULL DEFAULT ''`.
- `clear_developer_pii` set `company_org` and `location_city` to NULL; same.

SQLite aborts the whole `UPDATE`, so **nothing** was erased — not the name, not
the email, not the social handles. `handlers/privacy.rs` logs the error at WARN
and continues, so `POST /api/privacy/delete-request` returned
`200 {"status":"completed"}` while the PII sat untouched. Observed:
`d1_attendees_cleared: 0`, `d1_developer_cleared: 0`, row unchanged.

Fixed in `eb1549a` by blanking those four columns instead of nulling them.
Verified: the same request now reports `d1_attendees_cleared: 2`,
`d1_developer_cleared: 1`, and the rows really are `[DELETED]` / `''` / NULL.

Note this is the exact code Phase 1 §3 could only compile-check.

**Follow-up — done.** `delete_request` reported `"completed"` even when every D1
write failed. `DeletionSummary` now carries a `failures` list; the status is
computed as: any failure with no successful D1 write → `failed`; any failure
with at least one success → `partial`; otherwise the previous
`completed`/`partial`/`blocked` time-gate result. A write failure outranks the
time gate, because a caller who sees `blocked` retries after the event ends
while one who sees `completed` assumes their data is gone. `failures` carries
only operation names — the D1 error text (SQL + JS stack) stays in the log.
The frontend (`pages/data_privacy.rs`) gained a `Deletion Failed` state and a
distinct `partial`-with-failures message.

Two further defects found while making that change, both the same class as the
bugs above and both still live in prod:

- **`clear_contact_pii` had the identical NOT NULL abort.** `contacts.contact_channel`
  and `contact_handle` are `TEXT NOT NULL DEFAULT ''` and it set them to NULL,
  so contact erasure cleared nothing, `name` included. §6.2's fix covered
  `attendees` and `developer_profiles` but missed `contacts`. Reproduced
  directly against local D1: `NOT NULL constraint failed: contacts.contact_channel`.
- **`clear_developer_pii` never erased the social-linking columns.** Migration
  `0025_social_linking` added `telegram_handle`, `telegram_id` and the
  `*_verified` / `*_verified_at` flags; the erasure statement predates it, so a
  Telegram handle survived a completed PDPA erasure. Now nulled/zeroed.

`d1_responses_deleted` also reported `1` on success rather than the number of
rows deleted; it now uses the count `delete_developer_responses` returns.

Runtime-verified on local D1 (`wrangler dev --local`): happy path →
`completed`, `failures: []`, contacts row `[DELETED]`/`''`/`''` and the
developer row with every social column cleared; `BEFORE UPDATE ... RAISE(ABORT)`
triggers on `contacts` → `partial ['clear_contact_pii']`; triggers on all three
tables → `failed ['clear_attendee_pii','clear_contact_pii','clear_developer_pii']`.

### 6.3 The cron handler panics on a warm isolate

`lib.rs` `fetch` guards `tracing_wasm::set_as_global_default()` behind
`LOG_INITIALIZED: OnceLock`. The `#[event(scheduled)]` handler called it
unguarded. `scheduled` and `fetch` share an isolate, so on any isolate that had
already served a request the cron panicked immediately with
`SetGlobalDefaultError("a global default trace dispatcher has already been
set")` — the daily 03:00 UTC cleanup never ran there.

Fixed in `c933e98` with the same `OnceLock` guard. Verified: fetch first, then
`/cdn-cgi/handler/scheduled` → 200 and `cleanup: daily pass complete`.
