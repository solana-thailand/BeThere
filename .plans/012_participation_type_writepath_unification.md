# Plan 012 — participation_type Write-Path Unification (Issue #059 Tier B, step 3.2)

> ✅ **IMPLEMENTED** — all 5 write paths unified, 12 new tests, clippy clean.
> No data backfill in this step — that's 3.3, approval-gated.

## Goal

## Decisions (applied as recommended defaults)
1. **`walkin` stays distinct** in D1 — it's a status sentinel queried literally in
   `db/attendees.rs` and `claim/mint.rs`. Do NOT map it to `in_person`.
2. **Sheet = display-case, D1 = canonical snake_case.** Organizers read the Sheet.
3. **API is liberal-in-what-you-accept**: manual override accepts both
   `In-Person`/`in_person` and normalizes via `ParticipationType::parse()`.
   Frontend keeps sending display-case for now (backward-compatible); can be
   updated to canonical later as low-risk cleanup.

## Scope of changes (5 write paths)

### Path 1 — Registration (`register.rs::resolve_participation_type`)
- **Current**: returns `"In-Person"`/`"Online"` (display-case), used for BOTH D1 + Sheet.
- **Change**: return canonical `ParticipationType::as_str()` (`in_person`/`online`).
  - D1 upsert: canonical ✅ (already wants canonical)
  - Sheet append (2 call sites: bg_sync + sync fallback): convert canonical → display
  - `build_next_step`, `is_online_participation`, `enforce_capacity`, logging: all use
    `ParticipationType::parse()` so canonical works ✅
  - `write_developer_data` (survey response): canonical is fine ✅

### Path 2 — Manual override (`attendee.rs::update_participation_type`)
- **Current**: `ALLOWED_PARTICIPATION_TYPES = ["In-Person","Online"]`; stores
  display-case in both Sheet and D1.
- **Change**: 
  - Accept any value, normalize via `ParticipationType::parse()`.
  - Reject if `Other` (don't allow setting `walkin`/`test`/junk via this endpoint).
  - D1: store canonical `as_str()`.
  - Sheet: write display `display()`.
  - Remove `ALLOWED_PARTICIPATION_TYPES` const (replaced by enum validation).

### Path 3 — Sheet→D1 sync (`events/sync.rs::normalize_participation_type`)
- **Current**: custom matcher, close to enum but diverges (misses `in_person`
  underscore variant, doesn't handle `walkin` explicitly).
- **Change**: delegate to `ParticipationType::parse(s).as_str()`, but preserve
  `walkin`/`test` pass-through (they're not participation modes).

### Path 4 — Deposit deadline auto-switch / reclaim (`deposit/usdc/mod.rs`, `deposit/thb/...`)
- **Current**: writes `"Online"`/`"In-Person"` (display-case) to SHEET ONLY.
  D1 updated on next sync (normalized).
- **Change**: **NO CHANGE.** These only write to the Sheet (display-case is correct
  for Sheet). D1 gets canonicalized on the next sync. This is existing behavior.

### Path 5 — Walk-ins (`claim/mint.rs`, `db/attendees.rs::try_insert_walkin`)
- **Current**: `"walkin"` in D1, `"In-Person"` in Sheet.
- **Change**: **NO CHANGE.** Already canonical. Sentinel preserved.

## Domain crate change
- Add `ParticipationType::display()` → `"In-Person"`/`"Online"`/`"Other"`.

## Tests to add
1. **Domain**: `display()` returns expected strings.
2. **register.rs**: `resolve_participation_type` returns canonical for all 3 formats.
   (Existing test asserts display-case — update it.)
3. **attendee.rs**: manual override normalizes display-case AND canonical inputs,
   rejects `walkin`/`test`/junk. (Extract the validation logic into a testable fn.)
4. **events/sync.rs**: `normalize_participation_type` delegates to enum, preserves `walkin`.

## Verification
- `cargo test -p event-checkin-domain`
- `cargo test -p worker` (lib tests)
- `cargo clippy --workspace`
- `cargo check --workspace`

## Out of scope (deferred)
- D1 backfill (3.3) — approval-gated.
- SQL predicate simplification (3.4) — after backfill.
- Frontend sending canonical tokens — low-risk cleanup, separate.
- Frontend `utils::is_in_person` / `get_participation_badge` — WASM substring
  matchers keep working; refactor to label-derived later.
