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

---

## 7. Phase 3 (2026-09-04) — the audit was incomplete; a live injection remained

Phases 1 and 2 audited by reading the `db` layer and grepping for
`format!("SELECT …")`. That grep only matches a `format!` whose SQL literal
starts on the same line, and most of this tree writes multi-line SQL with
backslash continuations. It found six interpolation sites. A lexer that walks
every string literal finds **fifty-eight**.

### 7.1 The one that was not safe

`db::developers::upsert_developer_field` took `field_name: &str` and
interpolated it as a **column identifier**:

```sql
INSERT INTO developer_profiles (email, {field_name}, first_seen_at, …)
VALUES (?1, ?2, …)
ON CONFLICT (email) DO UPDATE SET {field_name} = excluded.{field_name}, …
```

`handlers::register::contact` calls it in a loop over `data.profile_fields`,
which is assembled in `register::signup` (and `register::post_event`) from
`body.profile_fields` — a `HashMap<String, String>` deserialized straight from
the **public, unauthenticated registration body**. The key was never validated
against anything; the attacker chose the identifier.

This is not a theoretical shape. The statement is `prepare().run()`, so stacked
statements are out, but everything inside one statement was reachable: closing
the column list early and supplying the rest of the `VALUES` tuple, or extending
the `DO UPDATE SET` to write columns the request has no business setting —
`total_events`, `consent_outreach`, or (since migration `0025`)
`github_verified` / `telegram_verified`, the flags the social-link flow only
sets after a provider actually verified the account.

It also failed silently in the honest case: every caller logs the error as
non-fatal, so a rejected statement produced one `warn!` and nothing else.

**Fixed.** `field_name` is now resolved through `resolve_profile_column`, which
returns `Option<&'static str>` drawn from `UPSERTABLE_PROFILE_COLUMNS`. Only the
returned `&'static str` is in scope at the `format!`, so the untrusted string
cannot reach the SQL even by accident. The allowlist is the fourteen
profile-owned columns; `email` (bound), `wallet_address` (owned by
`upsert_developer_wallet`), the `telegram_id` / `*_verified` / `*_verified_at`
social columns and the six bookkeeping columns are all excluded, each with its
reason recorded on the const.

**No behaviour change for legitimate traffic.** An organizer's custom form key
was never a `developer_profiles` column, so it already produced a D1
"no such column" error that was logged and dropped; it now produces a Rust error
that is logged and dropped. Custom answers were and are stored in
`registration_responses` by the sibling batch insert, with bound values.

### 7.2 The other fifty-seven are safe, and now they are safe *by type*

Every remaining interpolation is one of two shapes:

- **an identifier or predicate fragment**, which SQLite cannot parameterise —
  `get_event_raw`'s `column`, `count_by_status`'s `column`,
  `dashboard::count_attendees_by_predicate`'s `predicate`, the two
  `event_summaries` predicate helpers, `contacts`'s generated `?N` list;
- **an integer or bool**, interpolated only because `worker::d1::D1Type::Integer`
  is `i32`-only while these values are `u64` / `u32` — the `EventConfig` columns
  in `events.rs`, the summary counters, `jwt_blacklist`'s `expires_at`,
  `LIMIT`/`OFFSET`.

Two of the identifier helpers took `&str` and relied on a comment asserting that
callers only pass literals. Both are now `&'static str`, so the property the
comment claimed is the one the compiler checks:
`db::events::get_event_raw` and `db::attendees::reads::count_by_status`.

### 7.3 The regression gate

`worker/tests/sql_interpolation_guard.rs` lexes every `.rs` file under
`worker/src` (a hand-rolled lexer, not a grep — it has to tell a string literal
from a doc comment, and it has to follow `\`-continued multi-line SQL), keeps the
literals that begin with a SQL keyword, and fails on any `{…}` placeholder that
is not on `ALLOWED_INTERPOLATIONS`. Each allowlist row names the *type* that
makes its site safe; a second test rejects any row whose stated reason does not
cite one, so "callers only pass constants" cannot be the justification. A third
check fails on stale rows, which would otherwise silently pre-authorize whatever
lands at that path next.

Mutation-tested: adding
`format!("SELECT * FROM jwt_blacklist WHERE token_hash = '{token_hash}'")` to
`db/jwt_blacklist.rs` fails the guard and names the file and the placeholder.

`db::developers` also gained three unit tests: the allowlist is checked against
the `developer_profiles` DDL parsed out of `worker/migrations/*.sql` (a typo'd
column would otherwise silently drop a registrant's answer), hostile field names
are asserted not to resolve, and the list is asserted sorted and duplicate-free.

**Not fixed, noted:** `db::attendees::management::set_marketing_consent`
interpolates a `bool`, which renders as SQLite's `TRUE`/`FALSE` keywords rather
than `1`/`0`. Correct on D1's SQLite version and not injectable, so it was left
alone rather than widening this change.

Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, 15 workspace test binaries green, and
`cargo build -p event-checkin-worker --target wasm32-unknown-unknown --release`.

> **Still unpushed, so still live in production.** Like §6, this fix only exists
> on `feature/sql_binding_phase2`.

### 7.4 The guard had an escape hatch of its own (2026-09-04)

§7.3's scanner only recognised a literal as SQL when it opened with a
*statement* keyword (`SELECT `, `INSERT `, …, plus `WHERE `). Two allowlist rows
— `db/campaigns/crud.rs` and `db/contacts.rs` — sanction a `{where_clause}`
placeholder whose value is a `String` assembled elsewhere in the same file. So a
clause fragment written as

```rust
where_clause.push_str(&format!(" AND email = '{email}'"));
```

was invisible to the guard (it opens with `AND`, not a statement keyword) and
had a pre-approved path straight into a live query. The guard would have passed.
The module docstring asserted this shape "does not exist in the tree today",
which was wrong — the `where_clause` builders are exactly that shape.

`SQL_PREFIXES` now covers clause fragments as well: `AND`, `OR`, `SET`,
`VALUES`, `FROM`, `JOIN`, `GROUP BY`, `ORDER BY`, `HAVING`, `LIMIT`, `OFFSET`.

Matching had to become **case-sensitive** to absorb them. Every one of those
words is also ordinary English, and the tree logs `"set algo name: {e:?}"`,
`"delete attendee request"`, `"update campaign"` — a case-insensitive match
reports all of them as SQL. This tree writes SQL keywords in upper case
throughout (all 206 recognised literals), so case is the available
discriminator.

The cost is that a lower-case query would now slip past. That is accepted and
documented rather than papered over: an attempt to pin the convention with a
test failed because deciding whether a lower-case `"set …"` is SQL or prose is
the very question at issue — the test is circular and was dropped. Instead
`SQL_LITERAL_FLOOR = 80` fails the guard if the recognised-literal count
collapses, which is what both real failure modes look like (the lexer breaking,
or the tree drifting off the upper-case convention wholesale). Actual count is
206, so the floor has wide headroom and ordinary query deletions will not trip
it.

Mutation-tested: appending `format!(" AND c.email = '{email}'")` to
`db/contacts.rs` — the file whose `{where_clause}` row would have laundered it —
now fails the guard and names the file and the placeholder. Reverted after.

No allowlist row went stale in the switch, so the tightened matching lost no
coverage of the 58 known sites.

Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, 15 workspace test binaries green. The wasm build was not re-run
— the change is confined to `worker/tests/`, which is not in that target.

## 8. A fourth NOT NULL abort — `finalize_claim_lock` (2026-09-04)

Same class as §6.2, found by auditing that class mechanically rather than by
eye. `claim_locks.expires_at` is `TEXT NOT NULL` in **both** schemas —
`migrations/0001_initial.sql:15` and the DO's `event_do/schema.rs:17` — and
**both** finalize copies set it to `NULL`:

- `db/claim_locks.rs:63` (D1 path)
- `durable_objects/event_do/claim_lock.rs:83` (DO path)

The intent was "finalization removes the 5-minute TTL". SQLite instead aborts
the whole `UPDATE`, so `asset_id`, `signature` and `claimed_at` were never
written either. **`claim_locks` has never recorded a single completed mint.**

### Why it was user-visible, not just an audit gap

`claim::lock::finalize_claim_lock` called the D1 write with `?` *before* the KV
dual-write. The abort therefore short-circuited the function and the KV record
was never upgraded, so:

- the KV value stayed the **acquire** payload — `{lock_id, wallet, started_at}`,
  with no `asset_id` and no `signature`;
- it kept the **300-second acquire TTL** instead of `CLAIM_LOCK_FINALIZE_TTL_SECS`
  (90 days), so five minutes after a claim the key was gone entirely.

`claim/mint/lookup.rs:182` and `handlers/attendee/read.rs:55,278` read exactly
that record to show a claimed attendee their `signature` / `asset_id` / explorer
link. So **every attendee who claimed an NFT lost their proof link**, blank
immediately and with `wallet` also gone after five minutes.

Both call sites (`claim/mint/execute.rs:458`, `claim/mint/walkin.rs:126`) log the
failure at `warn!` and continue, which is why nothing ever surfaced.

Not a double-mint vector: the primary guard is `attendee.claimed_at.is_some()`
(`execute.rs:272`), and on the D1 path `INSERT … ON CONFLICT DO NOTHING` still
refuses a re-acquire. The DO path degrades to the wrong *message* only —
`handle_acquire_claim_lock` reads `claimed_at` to distinguish "already
completed" from "already being processed".

### Fix

- **`claim::finalized_expires_at()`** (new, in `claim/lock.rs` beside
  `CLAIM_LOCK_FINALIZE_TTL_SECS`) returns `now + 90 days` as RFC3339 — one
  definition shared by the D1 and DO copies, so the two cannot drift again.
  Finalization moves `expires_at` out to the retention horizon rather than
  nulling it: it satisfies the NOT NULL contract honestly, needs no migration
  (a SQLite `NOT NULL` drop is a table rebuild, and `CREATE TABLE IF NOT EXISTS`
  would never re-apply it to already-created DOs), keeps
  `idx_claim_locks_expires` meaningful for any future sweeper, and matches the
  KV record's own 90-day TTL. `claimed_at IS NOT NULL` stays the marker for
  "finalized", which is what the DO already reads.
- **Write ordering.** Both paths now hold the DO/D1 error and return it *after*
  the KV record is written. A failed durable write must not also cost the
  attendee their proof link.

### Guard — `worker/tests/not_null_column_guard.rs` (3 tests)

Pins the class, not the line. Parses every `CREATE TABLE` / `ALTER TABLE … ADD
COLUMN` in `migrations/*.sql` **and** in the DO's `schema.rs` into a
table → NOT NULL column map, then scans every `UPDATE … SET` under `worker/src`
for an assignment of `NULL` to one of those columns. Reading the code cannot
catch this bug — the statement and the schema are in different files and
different languages — so the test reads both.

Mutation-tested: restoring `expires_at = NULL` in both copies fails the guard,
naming each file, table and column. Only `UPDATE … SET` is covered; a
column-list `INSERT` omitting a NOT NULL column without a default is the same
class and is not.

The self-test (Layer 2) earned its place immediately — it caught that the first
parser was broken and that the real scan had been passing **vacuously**. Two
defects it surfaced: `find_ci` returned byte offsets while the readers used char
offsets (the migrations contain em dashes, so the two diverge), and SQL/Rust
line comments were parsed as code.

### Not done

- **No backfill.** Every `claim_locks` row from a completed mint still has
  `asset_id`/`signature`/`claimed_at` NULL. The data is recoverable — the
  attendee rows carry `claimed_at`, and the mint signature is on chain — but a
  migration would have to join against Solana, so it is an owner call.
- **`release_claim_lock` has the same ordering shape**: its D1 `DELETE` uses `?`
  before the KV delete, so a D1 failure leaves the KV lock in place. Latent, not
  live — that `DELETE` has no NOT NULL exposure — and left alone deliberately.
  The DO branch of the same function already warns-and-continues instead.
