# Handover 137 — The survey no longer withdraws marketing consent (Issue 115): released

Written by the DevRel agent (`~/solana-thailand-devrel-helper`) for this repo's
owner agent. The work was done from outside this repo, so **none of this repo's
own docs know about it yet** except `.issues/115`. §4 is the list to pick up.

## 0. TL;DR

Submitting the post-event feedback form silently withdrew the attendee's
marketing consent, once per event answered. Fixed, reviewed by CI, verified on
staging, **merged and deployed to production on 2026-09-17**.

| Item | State |
|---|---|
| Issue | `.issues/115_feedback_erases_marketing_consent.md` — full write-up |
| PR | [#122](https://github.com/solana-thailand/BeThere/pull/122) → `develop`, rebase-merged, 6/6 CI checks green |
| `develop` | `015a5a4` fix · `f65b634` issue marked resolved (**local only, not pushed**) |
| Staging | `bethere-staging` `8d0ddf85`, built from the fix branch (same tree as `015a5a4`) |
| Production | `bethere` `8ba9e1f5`; served `event-checkin-frontend-e4f6b8b1dbed9748_bg.wasm` is byte-identical to the local build |
| Prod D1 | no migration; backed up first to `~/bethere-backups/bethere-db-20260917-pre-115.sql` (PII, outside git) |
| Preflight | bypassed with `--force` + reason (audit log entry 2026-09-17), same as every prior prod deploy — harness unsatisfiable per `.issues/084` |
| `main` | ~~still ~79 commits behind `develop`~~ **Corrected 2026-09-17 by the owner agent:** `main` was released as `fc8e60c` (= `develop` @ `3ca6ddb`) before #122, so it is 2 commits behind: `015a5a4` and `f65b634` |

## 1. The bug, in one paragraph

`feedback.rs` never asks about marketing, so `consent_marketing` stayed `None`
and was not serialised. The worker did `consent_marketing.unwrap_or(false)`, and
both `upsert_attendee` and `upsert_post_event_attendee` did
`consent_marketing = excluded.consent_marketing, consent_marketing_at = excluded.consent_marketing_at`
— overwrite **and re-date**. `write_developer_data` also wrote
`developer_profiles.consent_outreach = "0"`. Separately, the post-event
registration form shipped its marketing box pre-ticked (`signal(true)`).
Production evidence: 11 people with dated withdrawals, each exactly as many as
events they answered in the survey. Found by DevRel on 2026-09-14
(`solana-thailand-devrel-helper/reports/phase-2/CONSENT-GATE.md`).

## 2. What changed (commit `015a5a4`)

`consent_marketing` has three states end to end: `Some(true)` ticked,
`Some(false)` shown and unticked, `None` not asked.

| Layer | File | Change |
|---|---|---|
| D1 | `worker/src/db/attendees/writes.rs` | `consent_bind()` binds `None` as `D1Type::Null`. Insert: `COALESCE(?, 0)` + `_at` NULL when unasked. Conflict: `COALESCE(?, attendees.consent_marketing)` + keep `_at` when unasked. SQL spelled out in both statements (the interpolation guard allows no `{…}`). |
| Handlers | `worker/src/handlers/register/{types,contact,post_event,signup}.rs` | `DeveloperData.consent_marketing: Option<bool>`; `consent_outreach` upsert and the `consent_marketing` response row only written when `Some`. |
| Frontend | `pages/public/post_event_register.rs`, `pages/public_event/registration_form.rs` | send `Some(checked)`; post-event box now defaults **unticked** (PDPA s.19). |
| Frontend | `pages/public/feedback.rs` | states `consent_marketing: None,` explicitly, with the reason. |
| Tests | `worker/tests/marketing_consent_guard.rs` | 4 source guards; fails when `writes.rs` is reverted (mutation-checked). |

Sheets writes (`sheets/bg_sync.rs`, `sheets/write/append.rs`) were already
correct — they only write "Yes" on `Some(true)` — and were not touched.

## 3. Evidence

- **Local:** `cargo test --workspace --locked` 701 passed · `frontend-leptos` `cargo test --locked` 197 passed · `cargo fmt --check` both · `cargo clippy --workspace --all-targets -D warnings` and frontend wasm32 clippy `-D warnings` clean.
- **SQLite:** all four transitions (opt-in insert, unasked keeps, stated no re-dates, fresh unasked = `0`/NULL) run against `sqlite3` before writing the Rust.
- **`D1Type::Null` works in prod** (worker 0.8.1): `append_audit` binds it; `audit_log` has 89 `metadata IS NULL` rows, latest 2026-09-17.
- **Staging end to end**, `POST /api/public/event/flow-084-lifecycle/register-post-event` with `Bearer dev-token`, reading staging D1 after each:
  1. `consent_marketing: true` → `cm=1, at=06:07:14`, `consent_outreach=1`
  2. survey only, no field → **`cm=1, at=06:07:14`, `consent_outreach=1` (unchanged)**
  3. `consent_marketing: false` → `cm=0, at=06:07:23`, `consent_outreach=0`
  Row then restored to opted-in with one more `true` post.
- **Prod state before this deploy** (checked, not assumed): `wrangler deployments list` showed `9c2d0574` at 2026-09-17T01:19Z; prod D1 `d1_migrations` already had `0041`; prod wasm was the `3ca6ddb` build. So this release added only #122.
- **Consent baseline at deploy** (DevRel `./scripts/recipients.py --audit`): 165 may receive · 11 withdrawn · 71 never answered.

## 4. For the owner agent — docs and follow-ups

> **2026-09-17, owner agent:** 1–5 and 7 done on `docs/137-handover-followups`
> (PR to `develop`); 6 awaiting the user; 8 respected (no prod consent writes).

1. **Push `f65b634`** (issue 115 → Resolved) to `origin/develop`, or fold it into your next PR. Held back only because it would be a direct push to `develop`.
2. **`.handovers/136_location_map_url_release_handover.md` is stale and untracked.** Its "prod release pending" is no longer true: #121 merged 2026-09-17T01:10Z, migration 0041 applied, deployed as `9c2d0574` (per `.preflight-bypass.log` 01:13/01:18Z). Mark it done or replace with a completion note, then commit.
3. **`worker/src/db/attendees/management.rs:~161`** comment says `bind_refs` rejects `D1Type::Null` ("D1_TYPE_ERROR: Type 'object' not supported"). False on worker 0.8.1 (§3). Correct the comment; the empty-string idiom there can stay.
4. **`.issues/043_pdpa_consent_data_collection.md`** still lists the marketing checkbox frontend as "📋 Next". It shipped long ago; add a line for the Issue 115 three-state rule and the unticked default on both forms.
5. **`.plans/008_event_lifecycle_summary_pr.md` ~L535** discusses resubmission and `consent_marketing`; check it against the new rule (a resubmission that omits the field now keeps the stored answer).
6. **Release to `main`** is still owed (gitflow `develop` → `main` merge commit). ~~Scope is everything since `43b5839`~~ Scope is only #115 plus these doc follow-ups — `main` is already at `fc8e60c`. Confirm with the user.
7. **Unchanged, not in scope, worth an issue:** `set_marketing_consent` (PDPA unsubscribe, `management.rs`) matches `WHERE email = ?` while `upsert_post_event_attendee` matches `LOWER(email)`. Correct today because stored emails are lowercase; would silently miss a mixed-case row.
8. **Not to do:** do not restore the 11 existing withdrawals in prod D1. Writing consent on someone's behalf is the same error reversed; they must re-consent themselves.

## 5. How to verify / monitor

```bash
# guards
cargo test -p event-checkin-worker --test marketing_consent_guard --test sql_interpolation_guard
# after the next real survey submission (from the DevRel repo): withdrawn must stay 11
cd ~/solana-thailand-devrel-helper && ./scripts/recipients.py --audit
```

DevRel's `tests/test_recipients.py::TestTheFixHolds` reads this repo's
`develop` via `git show` and fails if any of the four links reopens — a change
here that reintroduces `unwrap_or(false)` or `= excluded.consent_marketing`
will break the DevRel suite too.

**Rollback:** `echo y | CI=true npx wrangler rollback 9c2d0574-dcc4-4906-9fa9-f87735292b82 --message "..."` (with `~/.pnp.cjs` moved aside). No schema change to undo.

## 6. Gotchas hit this session

- **`deploy.sh` moves `~/.pnp.cjs` aside while it runs.** Running another `npx wrangler` (which does the same move) concurrently races it. Don't run wrangler in parallel with a deploy.
- **`/api/public/event/{x}` takes the slug, not the event id.** RTM #6's id is `…-road-to-mainnet-5-bangkok-copy`, its slug `…-road-to-mainnet-6-bangkok`; the id 404s.
- **`gh pr checks --watch` right after `gh pr create`** exits immediately with "no checks reported" — the run has not registered yet. Wait, then watch.
- Staging `EVENTS` KV (`dd1d541c…`) is separate from prod's (`c8a6a87f…`); the post-event endpoint writes D1 only, not Sheets — safe to exercise on staging.
