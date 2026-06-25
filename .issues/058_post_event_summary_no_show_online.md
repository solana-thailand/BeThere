# 058 — Post-Event Summary No-Show Miscounts Online Attendees

## Status
- **Summary no-show bug**: ✅ Fixed (commit `4e6b4f0`, migration 0021 applied to prod)
- **`participation_type` data hygiene**: 🟡 Open (follow-up — see bottom)

## Summary
The post-event summary (Plan 008 Phase 1) computed `no_show = registered − checked_in`
uniformly across all event formats. For **online** events this is wrong: there is no
physical check-in, virtual check-in (quest completion) is opt-in, and joining the call
isn't recorded. So online registrants who simply attended via link were all counted as
no-shows.

### Concrete prod example — `solana-in-latent-space-part-1`
- 25 approved attendees, all `participation_type = "Online"`, 0 check-ins.
- **Old**: `no_show = 25 − 0 = 25` (100% no-show rate) — clearly wrong.
- **New**: `no_show = 0 − 0 = 0` (no in-person attendees → no in-person no-shows).

## Fix (commit `4e6b4f0`)
`no_show_count` is now computed **only across the in-person slice**:
`no_show = in_person_registered − in_person_checked_in`.

Two new funnel fields expose the basis so the UI can show a correct rate and distinguish
online-only events:
- `in_person_registered_count`
- `in_person_checked_in_count`

The in-person predicate mirrors `Attendee::is_in_person()` (domain crate): empty /
unrecognized values default to in-person (legacy events), and substring matching handles
the sheet's inconsistent casing (`In-Person`, `in person`, `IN_PERSON`, ...). See
`worker/src/db/dashboard.rs::IN_PERSON_PREDICATE`.

### Frontend
The "No-show" tile is now "No-show (in-person)":
- In-person / hybrid: shows the count with rate `X% of N in-person`.
- Online-only (`in_person_registered == 0`): shows `—` with sub `online`.
- Footnote: "Online registrants are excluded from no-show — their attendance isn't
  recorded via check-in."

### Files changed
| File | Change |
|------|--------|
| `domain/src/models/event_summary.rs` | +2 funnel fields (`in_person_*_count`) |
| `worker/migrations/0021_event_summaries_in_person_breakdown.sql` | New — 2 additive columns |
| `worker/src/db/dashboard.rs` | `IN_PERSON_PREDICATE` const + 2 count helpers |
| `worker/src/db/event_summaries.rs` | `compute_snapshot` uses in-person slice; upsert/row_to_summary handle new cols |
| `frontend-leptos/src/api/event.rs` | +2 funnel fields |
| `frontend-leptos/src/pages/event_summary.rs` | Relabel + correct rate denominator + online note |

### Prod actions taken
- Backed up D1 → `/tmp/bethere-backup-pre-0021-20260625.sql`
- Applied migration 0021 (2 columns, additive)
- Cleared 2 existing buggy frozen rows (`no_show_count=25` each) so they recompute on next GET
- Verified against prod data: online event → `no_show=0`; hybrid event → correct in-person slice

### Deployed + verified ✅
- Worker deployed `2026-06-25T04:38:58Z` (after commit `4e6b4f0`); migration 0021 applied to prod `bethere-db`.
- Re-freeze confirmed in prod: a GET after deploy auto-froze `solana-in-latent-space-part-1-copy`
  (online, 25 registered) with `no_show_count = 0`, `in_person_registered_count = 0` — old logic
  would have written 25. Frozen at `2026-06-25T04:54:57Z`.
- Git: `develop` at `4e6b4f0`. `main` still at `c2a1309` (does not include Phase 1 or the fix).

## 🟡 Follow-up: `participation_type` data hygiene
While fixing this, discovered `attendees.participation_type` has **inconsistent values**
in prod (no canonical form — values come straight from the Google Sheet):

| value | count |
|-------|-------|
| `""` (empty) | 21 |
| `"In-Person"` | 16 |
| `"Online"` | 109 |
| `"in_person"` | 66 |
| `"online"` | 53 |
| `"test"` | 1 |

The domain crate had **no `ParticipationType` enum** — it was stored/compared as raw
strings. `Attendee::is_in_person()` used substring matching, and the worker SQL
`IN_PERSON_PREDICATE` (`worker/src/db/dashboard.rs`) re-implements the same rules for
DB counts (SQL can't call the Rust fn, so this duplication is unavoidable but
documented). This affects more than summaries: online capacity gating, hybrid track
routing, and any feature keyed on participation type are all fragile.

**Recommended fix (separate project):**
1. ✅ **Done (`fa5b1c8`)** — Add a `ParticipationType` enum to the domain crate
   (canonical snake_case `in_person`/`online`/`other`) with `parse()` handling all
   known prod variants; `Attendee::is_in_person()` now delegates to it (behavior
   preserved; verified by `is_in_person` + new `participation_type` unit tests).
2. ⬜ Normalize at the source: canonicalize when writing from the Google Sheet sync and
   the registration path.
3. ⬜ One-time D1 migration to backfill existing rows to canonical values.
4. ⬜ Replace remaining substring-matching read sites with the typed enum (the SQL
   `IN_PERSON_PREDICATE` stays, but can be simplified once rows are canonical).

This is **not** blocking the summary fix — `IN_PERSON_PREDICATE` handles all known
variants defensively — but it's a latent bug across the platform.
