# 020 — SQL parameter binding across `worker/src/db`

Status: **Phase 1 done** (free-text and destructive-predicate sites bound).
Phase 2 (residual id/hash predicates) open.

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

## 4. Phase 2 — residual sites (open)

~40 interpolated predicates remain, all of the same two shapes:

- **Ids** — `campaign_id`, `event_id`, `asset_id`, `signature`,
  `organization_id`, `status` (closed enum), `token_hash` (blake3 hex).
  Server-generated or shape-validated (`validate_campaign_id`).
- **Emails** — `developer_email` in `db/campaigns.rs` (5 sites),
  `db/contacts.rs`, `db/developers.rs`.

No email validator exists anywhere in `worker/src` or `domain/src`
(searched for `validate_email` / `is_valid_email` / `contains('@')` — no
hits). Emails reach these queries from the JWT claim, so today they are
bounded by the identity provider. That is an *implicit* invariant with
nothing enforcing it, and it would break the moment a login path lets a user
choose their own email string.

Phase 2 should:

1. Bind the remaining predicates — mechanical, same pattern.
2. Or, if that is deferred, add an explicit email validator at the auth
   boundary so the invariant is enforced rather than assumed.

Preference is (1): it removes the invariant entirely instead of documenting it.

## 5. Out of scope, noticed while here

`GET /api/events/{id}` returns **500** for a missing event, not 404
(`"internal error: failed to read event: event '…' not found"`). Pre-existing,
unrelated to this work, not fixed.
