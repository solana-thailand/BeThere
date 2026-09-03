# 020 — SQL parameter binding across `worker/src/db`

Status: **Phase 1 done and staging-verified** (on `develop`).
**Phase 2 done but compile-verified only** (on `feature/sql_binding_phase2`) —
see §4.4 before merging.

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

### 4.4 What Phase 2 still needs

`cargo clippy -D warnings` clean and 212 lib tests pass, but **that only
proves it compiles**. Every real defect in this workstream was found by
running the code, not reading it. Before this branch merges it needs the same
staging treatment Phase 1 got: deploy `bethere-staging`, then exercise
campaign CRUD + set-events, check-in auto-progress, campaign stats, the
profile update endpoint, marketing-consent toggle, logout (JWT blacklist),
summary freeze, and the post-event-registration toggle.

The highest-risk conversions to check first are `list_campaigns` (dynamic
`WHERE`), `upsert_summary` (three placeholders across a 19-column INSERT),
and `update_my_profile` (12 placeholders).

### 4.5 Still open: no email validation

No email validator exists anywhere in `worker/src` or `domain/src` (searched
`validate_email`, `is_valid_email`, `contains('@')` — no hits). Binding makes
this harmless for SQL, but "emails are well-formed" remains an unenforced
assumption that other code may lean on. Worth an explicit validator at the
auth boundary, tracked separately.

## 5. Out of scope, noticed while here

`GET /api/events/{id}` returns **500** for a missing event, not 404
(`"internal error: failed to read event: event '…' not found"`). Pre-existing,
unrelated to this work, not fixed.
