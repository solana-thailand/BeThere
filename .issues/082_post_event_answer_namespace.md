# 082 — `post.` answer namespace, and a bound on the public field map

**Status:** fixed 2026-09-13 (code only; nothing deployed) — see `.issues/090`
**Found:** 2026-09-13, answering DevRel `BETHERE-ASKS.md` item 2
**Severity:** low for the namespace, medium for the bound

## Why

DevRel are retiring two Google Forms (two forms, lists cut for 55 onsite and 96
online people, ready for weeks, **zero responses**) and moving post-event
satisfaction into BeThere. They chose the `field_key` convention we offered:
`post.satisfaction.overall`, `post.nps`, `post.would_return`, submitted through
the existing `POST /api/public/event/{slug}/register-post-event` body. They
asked for one change: stop the profile upsert warning once per answer.

Reviewing that path for first production use turned up two more things.

## What changed

### 1. `post.` is now a recognised namespace

`db/developers.rs`: `EVENT_SCOPED_FIELD_PREFIX` + `is_event_scoped_field`.

An answer in that namespace describes the *event*, not the person, so
`write_developer_data` skips the `developer_profiles` upsert for it
deliberately instead of calling, failing the `UPSERTABLE_PROFILE_COLUMNS`
allowlist, and logging. A **non**-namespaced key that fails to resolve is still
a real form-config mistake and still warns — the noise is gone, the signal is
not.

A test asserts the namespace is disjoint from every upsertable column, so a
future column named `post.*` cannot silently stop updating the profile it is
supposed to update.

### 2. `is_profile_field` stopped lying

The column records whether an answer *also* updated `developer_profiles`. It
was hardcoded `true` for every dynamic field; a `post.` answer does not update
a profile, so it now records `false`.

This matters beyond tidiness: DevRel read exactly this column from outside the
Worker to work out where the existing 2,103 rows came from. A guard test fails
if it is hardcoded again.

### 3. The public field map is bounded

`profile_fields` is a free-form `HashMap<String, String>` on **two public
endpoints** (`register` and `register-post-event`) and had no cap on the number
of entries or the length of a key or value. One request could become an
unbounded number of `registration_responses` rows.

`accepted_profile_fields` now bounds it: 40 fields, 64-char keys, 2,000-char
values; empty keys and values dropped; over-long entries **dropped, not
truncated** — a truncated answer is a corrupted answer.

The bound lives in `write_developer_data`, which both entry points funnel
through. Putting it on one caller would leave the sibling unbounded, which is
how this class of bug survives (memory `duplicated-state-transition-paths`); a
guard test fails if either consumer iterates the raw map again.

### 4. The batch insert can no longer exceed D1's parameter ceiling

Found while sizing the cap, and the sharpest of the four.
`batch_insert_registration_responses` built **one** INSERT with 6 bound
parameters per row. D1 rejects a statement over 100 bound parameters, and the
write is best-effort — so a question set of about a dozen questions would have
failed *as a whole*, logged one warning, and silently dropped every answer
including the consent rows.

It now chunks at 10 rows (60 parameters), and the relationship is asserted at
**compile time** rather than in a test:

```rust
const _: () = assert!(RESPONSE_CHUNK_ROWS * RESPONSE_BIND_COUNT <= D1_MAX_BOUND_PARAMS);
```

Widening the INSERT or raising the chunk size fails the build. Verified by
temporarily setting the chunk to 20: `error[E0080]: evaluation panicked`.

## Verification

- 6 unit tests in `contact.rs`, 2 in `developers.rs`, 3 wiring guards in
  `worker/tests/registration_field_bounds_guard.rs`.
- **Negative-controlled**: hardcoding `is_profile_field = true` fails the guard;
  raising `MAX_PROFILE_FIELDS` to 100,000 fails the range test; a 20-row chunk
  fails the build. Each passed again on revert.
- One test was **rewritten because it was self-referential** — the original cap
  test read `MAX_PROFILE_FIELDS` to assert against `MAX_PROFILE_FIELDS`, so it
  stayed green when the constant was set to 100,000. It now pins the policy
  against literals as well (memory `false-clean-probes-from-shell-aliases`).
- `cargo clippy --workspace --all-targets -- -D warnings`, worker clippy, both
  test suites, `cargo fmt --check` on workspace **and** worker, ShellCheck gate:
  all clean.

## Not done

The Leptos form still does not send `profile_fields` —
`PostEventRegisterBody` (`frontend-leptos/src/api/event/post_event.rs`) has the
three fixed profile fields and no map. The API accepts a question set today; the
UI cannot yet submit one. That is the remaining work, and it needs DevRel's
questions first.

Nothing here is deployed.

## Related

- DevRel `reports/phase-2/BETHERE-REPLY.md` item 2.
- `.issues/068_retrospective_learning_hub.md` — `participation_type =
  'retrospective'`, which this path already writes.
