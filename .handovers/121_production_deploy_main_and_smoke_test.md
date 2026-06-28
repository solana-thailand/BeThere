# Handover 121 — Production Deploy to `main` (41 commits) + API/Protocol Smoke Test

**Date:** 2026-06-27
**Branch:** `main` (merge from `develop` — no feature branch; this was a deploy session)
**Deploy range:** `c2a1309..6f4a7b3` (41 commits, Plans 008/011/013/014)
**Outcome:** ✅ DEPLOYED, LIVE-VERIFIED (6 checks), SMOKE-TESTED (3 surfaces). Plan 008 lazy-freeze path **verified-live** (1 row frozen 2026-06-25); manual `POST /summary/freeze` + browser-level UI rendering remain unexercised live.
**Test delta:** 0 (no `.rs`, no `Cargo.toml`, no migrations authored — this session shipped already-merged code to production).

---

## 1. What happened

The prior session (handover 120) closed Plan 014 Phases 5.1 + 5.2 doc-only and left 16 commits unpushed on `develop`. This session's intended scope was narrow: push those 16 commits, delete merged feature branches, done. The actual scope turned out to be a full production deploy with a critical scope correction along the way.

### Phase 1 — Push + branch cleanup (as planned)

- Pushed `develop`: `80c51fc..6f4a7b3` (16 commits published to `origin/develop`)
- Deleted 8 merged `014_*` feature branches (each verified merged via safe `-d` first)

### Phase 2 — Deploy audit caught a scope error

The prior session's summary claimed *"No schema changes; rollback is `wrangler rollback`"* — but that claim was scoped to **only the 16 unpushed commits**, not the actual deploy gap. Auditing the real `main..develop` gap revealed:

| Claim (prior summary) | Reality (`main..develop`) |
|---|---|
| 16 commits | **41 commits** (`main` last deployed 2026-06-24 at `c2a1309`) |
| Docs-only | Includes Plans 008, 011, 013, 014-Phase-1 (wire format, post-event summaries, event series, campaigns) |
| No schema changes | **2 D1 migrations**: `0020_event_summaries_post_event.sql` + `0021_event_summaries_in_person_breakdown.sql` |
| Rollback = `wrangler rollback` | Rollback after migrations leaves **orphaned tables/columns** |

**The blind deploy would have shipped correct code but misled the operator on rollback semantics.** Stopped and re-planned before proceeding.

### Phase 3 — Full production deploy

1. **D1 verification:** `wrangler d1 migrations list --remote` → *"✅ No migrations to apply!"* — D1 was already fully migrated (all 21 migrations applied, `event_summaries` table + new columns present). The schema was **ahead of the deployed worker** before this session.
2. **Frontend rebuild:** The dist was stale — built Jun 26 23:02, but `wire.rs` was modified Jun 27 08:57 (~10h newer). Rebuilt via `trunk build --release`. New bundle hash: `d334df8c0d54958b` (was `c8fd97a372588044`).
3. **Merge:** `main` was **not** a fast-forward ancestor of `develop` (main had 1 unique commit `c2a1309` — a Solana Mobile demo merge). `git merge develop` on main → clean `ort` strategy merge, no conflicts → `b432ac5`. Trees identical post-merge.
4. **Worker compile:** `cargo check --target wasm32-unknown-unknown` clean, no warnings.
5. **Push:** `main` → `origin/main` (`c2a1309..b432ac5`).
6. **`scripts/deploy.sh`:** Deployed. The script handles a known Wrangler 4.x bug (versions API error 10013) via fallback to legacy PUT API + manual asset JWT upload, and a Yarn PnP conflict by temporarily moving `~/.pnp.cjs` (trap-based restore).

### Phase 4 — Live verification (6 checks)

| Check | Result | Meaning |
|---|---|---|
| `GET /` | **200** | Worker alive, SPA serving |
| Frontend bundle hash | `d334df8c0d54958b` | **New** rebuild is live |
| `GET /{bundle}.js` | 73215 bytes, 200 | Assets uploaded correctly |
| `GET /api/events/test-event/summary` | **401** (not 404) | New Plan 008 endpoint **exists** + enforcing auth |
| `GET /api/health` | 200 | No regression on existing routes |
| `GET /api/wire-sample/level-score` | **200** | New Plan 014 wire endpoint live |

### Phase 5 — Smoke test of 3 feature surfaces

| Feature | Result | Verdict |
|---|---|---|
| **Wire format** (Plan 014 Phase 1.7) | 56-byte payload fetched live, decoded via real `domain::wire::unpack::<LevelScore>` — **BLAKE3 verified**, all fields match (`moves=7, puzzles_solved=2, time_seconds=45, stars=2`) | ✅ **Definitively verified** — round-trip clean |
| **Event series nav** (Plan 013) | `/api/public/events` 200; `/api/public/event-series/{id}` 404 *"not part of a campaign"* | ✅ **Live, no data yet** — `campaigns`=0 rows, `campaign_events`=0 rows. The 404 is correct. |
| **Post-event summary** (Plan 008) | `/api/events/{id}/summary` **401** (route exists, enforcing auth) | ✅ **Route live + freeze path verified-live (prior session)** — `event_summaries`=1 row (frozen 2026-06-25, `no_show_count=0`). The smoke test's 401 confirms the route on the new deploy; the freeze write itself was exercised live 2 days earlier (issue #058). |

### Deliverables (this session)

| Action | Result |
|---|---|
| Push `develop` → `origin/develop` | ✅ `80c51fc..6f4a7b3`, 16 commits |
| Branch cleanup | ✅ 8 merged `014_*` branches deleted |
| Frontend rebuild | ✅ Bundle `d334df8c0d54958b` (was `c8fd97a372588044`) |
| `main ← develop` merge | ✅ Clean `ort` merge → `b432ac5`, trees identical |
| Push `main` → `origin/main` | ✅ `c2a1309..b432ac5` |
| `scripts/deploy.sh` | ✅ Deployed, startup 13ms |
| This handover | ✅ `.handovers/121_production_deploy_main_and_smoke_test.md` |

---

## 2. Where is the plan / code / test

| Artifact | Path | Purpose |
|---|---|---|
| Deploy script | `scripts/deploy.sh` | Worker deploy + asset upload, handles Wrangler 4.x bug + Yarn PnP conflict |
| Wire format source | `domain/src/wire.rs` | `pack`/`unpack`/`pack_slice`/`unpack_slice` — the deployed encoder |
| Wire bench (GOAT-gate) | `domain/benches/wire_bench.rs` | Phase 1.7 arbiter — already cleared at commit `787e62d` (6.2× decode, 73.5% size) |
| Migration 0020 | `worker/migrations/0020_event_summaries_post_event.sql` | `event_summaries` table + 3 `events` cols + 1 `attendees` col + index |
| Migration 0021 | `worker/migrations/0021_event_summaries_in_person_breakdown.sql` | 2 `event_summaries` cols (in-person breakdown) |
| Smoke endpoint | `worker` route `GET /api/wire-sample/level-score` | Live wire-format proof |
| Cross-ref | `.plans/008_event_lifecycle_summary_pr_generator.md` | Plan 008 (105 tasks, ongoing) |
| Cross-ref | `.plans/011_solana_mobile_demo_day.md` | Plan 011 (in deploy range) |
| Cross-ref | `.plans/013_*` | Plan 013 (event series — live, no data) |
| Cross-ref | `.plans/014_wire_audit.md` | Phase 1.7 GOAT-gate result record |
| Cross-ref | `.handovers/120_*` | Prior session — 16 unpushed commits, doc-only closure |

**No code authored in this session.** All shipped code was already merged to `develop` in prior sessions; this session shipped it to production.

---

## 3. Audit findings (the substance)

### 3.1 The scope correction — the load-bearing finding

The prior summary's *"no schema changes, docs-only"* claim was not malicious or sloppy — it was correctly scoped to the 16 unpushed commits from handover 120's session. The error was treating that as the **deploy** scope. The actual deploy gap was `c2a1309..6f4a7b3` (41 commits), because `main` had last been deployed 3 days earlier and had accumulated Plans 008/011/013/014 in the interim.

This matters because of rollback semantics. Pre-migration, `wrangler rollback` is a clean revert. Post-migration, rollback reverts the **worker code** but leaves the new tables/columns orphaned — not data-corrupting, but operationally confusing on a future audit. The corrected scope made this explicit before deploying rather than discovering it during an incident.

### 3.2 D1 was ahead of the worker — schema drift

The most surprising finding: production D1 had **all 21 migrations applied** before this session, but the deployed worker was the pre-0020 code. This means migrations 0020/0021 were applied to production D1 in a prior session without a corresponding worker deploy. The schema existed but was unreferenced. This session brought the worker into alignment.

This is a process smell worth flagging: **applying migrations and deploying the worker should be atomic.** The current `deploy.sh` does not apply migrations (by design — migrations are applied separately via `wrangler d1 migrations apply --remote`), which allows the two to drift.

### 3.3 Wire format — definitive round-trip from production

This is the strongest verification of the session. Rather than eyeballing hex bytes, I fetched the live binary (`curl https://bethere.solana-thailand.workers.dev/api/wire-sample/level-score?fmt=bin`) and decoded it through the **real `domain::wire::unpack::<LevelScore>`** code. The function recomputes BLAKE3 over the payload and rejects tampering with `WireError::HashMismatch` — and it **passed**.

This proves the deployed worker's encoder and the domain crate's decoder agree end-to-end across:
- Magic (`BTE1`)
- Version (1)
- Struct layout (16-byte `LevelScore`: 3× `u32` + `u8` + `[u8; 3]` pad)
- BLAKE3 commitment

One mid-test puzzle: production payload was 16 bytes but the handler references `_pad`. Resolved by reading the real struct definition — `_pad: [u8; 3]` (3 bytes), not `[u32; 3]` (12 bytes). Didn't guess.

### 3.4 The 404 that is correct, and the freeze that already ran

`event-series/{id}` (404 *"not part of a campaign"*) is a "missing data" response that proves the **code is live and structured correctly** — the endpoint exists, the auth gate fires, the structured error comes back. It has no data because no organizer has grouped events into a campaign yet (`campaigns`=0, `campaign_events`=0).

`event_summaries` is **NOT** empty — it has **1 row**, frozen 2026-06-25T04:54:57Z for `solana-in-latent-space-part-1-copy` (an online event, `in_person_registered_count=0`, `no_show_count=0`). This freeze ran live 2 days before this deploy session (documented in issue #058), proving the lazy-on-first-read-after-`event_end_ms` write path works end-to-end against real D1. The freeze wrote the **fixed** value (`no_show_count=0`) — old logic would have written 25.

**Correction note:** an earlier draft of this handover (and the session summary it was based on) claimed `event_summaries`=0 rows / "freeze path untested-live." Verified directly against prod D1 during the post-deploy audit — that claim was wrong. See §8 caveat #8.

### 3.5 Verified production D1 state (post-deploy audit)

Queried directly against prod `bethere-db` during the post-deploy audit (`npx wrangler d1 execute bethere-db --remote --json`). These are **facts**, not claims inherited from any summary:

| Table | Count | Notes |
|---|---|---|
| `events` | **9** | Includes the public Road-to-Mainnet series (#1–#4 Bangkok), Solana-in-Latent-Space (Parts 1–3), `intro-to-vibing-on-solana`, `islanddao-v4-demo`, plus `solana-in-latent-space-part-1-copy` (non-public test event that owns the frozen summary) |
| `campaigns` | **0** | No campaigns created yet — Plan 013 series nav self-hides |
| `campaign_events` | **0** | — |
| `event_summaries` | **1** | Frozen 2026-06-25T04:54:57Z, `event_id=solana-in-latent-space-part-1-copy`, `no_show_count=0`, `in_person_registered_count=0` (online event, fixed logic) |

The public-API smoke test's view of "3 events" was a **filtered subset** (only published events surface in `/api/public/events`), not the production total. The production DB has 9 events; the public listing shows fewer. Both counts are correct in their own scope — the error was framing the public-API count as "events in production."

The frozen `solana-in-latent-space-part-1-copy` event still exists in the `events` table (verified — not an orphaned summary row). It is simply non-public, which is why it doesn't appear in `/api/public/events` even though its summary is frozen.

---

## 4. Reflection — struggling / solved

### Solved: did NOT trust the prior summary's scope claim

The instinct to run `git log main..develop --stat` before deploying caught a scope error that the prior summary's framing would have missed. Audit-first discipline paid off concretely: a blind deploy would have shipped correct code but misled the operator on rollback.

### Solved: did NOT fabricate a benchmark run to look diligent

In the pre-stop verification, I initially declared the wire bench N/A, then noticed the deploy range included 2 commits touching wire code (`787e62d` + `617d7dc`) and re-investigated rather than skip on assumption. The re-investigation confirmed `617d7dc` was a **doc-comment-only** change to `wire.rs` plus a test-only `alloc_count` feature — the decode logic is byte-identical to what the GOAT-gate measured. So the bench genuinely is N/A: re-running would measure unchanged code. The honest call was to skip it with justification, not manufacture a 5-minute bench run to look thorough.

### Solved (in the audit pass): caught a propagated inaccuracy about the freeze path

The initial smoke test marked the Plan 008 freeze write path "🟡 untested-live" and the first draft of this handover claimed `event_summaries`=0 rows / "no freeze has ever run in production." That claim was inherited from the session summary without independent verification. The post-deploy audit queried prod D1 directly and found **1 row**, frozen 2026-06-25T04:54:57Z — the freeze path IS tested-live (issue #058 documented it, the row confirms it). Corrected the handover (§3.4, §5 #1, §8 #4) rather than leaving the wrong claim in place. The honest lesson: "a session summary is a claim, not a fact." When a handover records prod state, verify the prod state directly.

### Solved: did NOT distinguish "live" from "tested-live" prematurely

The smoke test's 401-not-404 check was correctly framed as "route exists, enforcing auth" — that part was honest. The overclaim was extending it to "freeze write untested-live" without checking whether a prior session had already exercised the freeze. The correction (verify against D1) turned the 🟡 into a ✅ on the lazy-freeze path. The **manual** `POST /summary/freeze` variant remains genuinely unexercised live (§5 #1), and that distinction is now accurate.

### No real struggles

The session's mechanics (push, merge, deploy) were routine. The only friction was the Wrangler 4.x versions API bug, which `deploy.sh` already handles via fallback, and the brief wrangler-CLI invocation puzzles during the post-deploy audit (DB name is `bethere-db`, not `bethere`; `--json` output structure varies between calls). No debugging rabbit holes.

---

## 5. Remaining work

### Untested-live (needs human/browser)

1. **Plan 008 summary — manual `POST /summary/freeze` path** — the **lazy** freeze path (on first read after `event_end_ms`) is **verified-live** (1 row frozen 2026-06-25). The **manual** freeze endpoint (`POST`) has not been exercised live. Lower priority since the lazy path works.
2. **Browser-level UI rendering** of the 3 new surfaces (summary page, series nav, wire decoder). Curl/protocol checks confirm the data layer but not Leptos component rendering. Open the app in a browser as an organizer and watch for console errors.

### Process follow-up (not blocking, worth flagging)

3. **Migration/worker deploy atomicity** — D1 was ahead of the worker before this session (migrations applied without worker deploy). Consider whether `deploy.sh` should refuse to deploy if unapplied migrations exist, or whether migrations should be applied as part of deploy. Currently the two can drift.

### Plan 014 remaining open work (unchanged from handover 120)

4. **Phase 2.2 R3** — Substantive mirror-type merge decision (`EventFormat`/`EscrowStatus`/etc.). Design decision; needs dedicated session.
5. **Phase 4.1** — Profile a staged 200-attendee event end-to-end. **Blocked on infrastructure coordination.**
6. **Phase 4.2** — CPU-bound hot spot flamegraph (conditional on 4.1).
7. **Phase 4.4** — Document the "no SIMD" decision. **Blocked on 4.1.**
8. **Acceptance criteria #2, #3, #4** — Tied to the above Phase 2.2/4.x work.

### Cleanup (already done)

- ✅ 8 merged feature branches deleted
- ✅ Throwaway wire-decode example (`domain/examples/`) created for validation and deleted
- ✅ `~/.pnp.cjs` restored (deploy.sh trap confirmed)
- ✅ `develop` and `main` both pushed to origin

---

## 6. Issues ref

No new issues created in this session. Relevant existing refs that landed in this deploy:

- `.issues/058_post_event_summary_no_show_online.md` — Plan 008 (summary route is live)
- `.issues/059_participation_type_canonicalization.md` — Plan 013 (series endpoints live)
- `.issues/060_attendee_event_series_navigation.md` — Plan 013 (series endpoints live, no data yet)
- `.issues/057_headers_not_applied_put_api_fallback.md` — directly relevant to the `deploy.sh` PUT API fallback path exercised in this deploy

---

## 7. How to dev / test

### Verify the live deploy

```sh
# Worker alive + SPA serving
curl -sS -o /dev/null -w "%{http_code}\n" https://bethere.solana-thailand.workers.dev/

# New Plan 008 summary route exists (401, not 404)
curl -sS -o /dev/null -w "%{http_code}\n" https://bethere.solana-thailand.workers.dev/api/events/test-event/summary

# New Plan 014 wire endpoint
curl -sS -o /dev/null -w "%{http_code}\n" https://bethere.solana-thailand.workers.dev/api/wire-sample/level-score

# Confirm new frontend bundle hash is live
curl -sS https://bethere.solana-thailand.workers.dev/ | rg "d334df8c0d54958b"
```

### Re-verify wire format round-trip from production

```sh
# Fetch live binary (56 bytes)
curl -sS https://bethere.solana-thailand.workers.dev/api/wire-sample/level-score?fmt=bin -o /tmp/wire.bin
wc -c /tmp/wire.bin  # expect 56

# Decode via the real domain crate (run from repo root)
# — the throwaway example used during smoke test was deleted;
#   the round-trip is covered by unit tests in domain/src/wire.rs
cargo test -p event-checkin-domain --features wire --quiet
```

### Verify D1 schema state

```sh
# Confirm migrations applied
wrangler d1 migrations list --remote --env production
# Expected: "✅ No migrations to apply!"

# Confirm tables/columns
wrangler d1 execute bethere --remote --command "SELECT COUNT(*) FROM event_summaries"
wrangler d1 execute bethere --remote --command "SELECT COUNT(*) FROM campaigns"
wrangler d1 execute bethere --remote --command "SELECT COUNT(*) FROM campaign_events"
# Expected: 0, 0, 0 (no data yet — features live but unused)
```

### Rollback semantics (important)

```sh
# Worker code rollback — clean, reverts to prior worker version
wrangler rollback

# NOTE: This does NOT revert D1 migrations. After this deploy, rolling back
# the worker leaves event_summaries + the new events/attendees columns orphaned.
# The tables/columns persist but the rolled-back worker won't reference them.
# Not data-corrupting, but operationally confusing. Manual D1 rollback (writing
# a reversing migration) would be required for full schema revert.
```

### Test the untested-live Plan 008 freeze path (needs browser)

The summary freeze write path cannot be exercised headlessly (`DEV_MODE=0` requires real OAuth). To validate:

1. Open `https://bethere.solana-thailand.workers.dev/` in a browser
2. Sign in as organizer of "Road to Mainnet #1" (freeze-eligible — `event_end_ms` in past)
3. Navigate to the event summary page
4. Confirm the lazy freeze fires on first read (check `event_summaries` row count before/after)
5. Alternatively, hit `POST /api/events/{id}/summary/freeze` authenticated and check the row appears

### Local diagnostics (confirmed clean at session end)

```sh
cargo check --workspace --quiet          # clean, no errors
cargo clippy --workspace --all-targets --quiet  # clean, no warnings
git status --short                       # empty (clean tree)
git log --oneline -1 main                # b432ac5 (== origin/main == develop)
```

---

## 8. Honest caveats

1. **The prior summary's scope claim was wrong, and I caught it only because I audited before deploying.** This is not a criticism of the prior session — its summary was correctly scoped to its own 16 commits. The error was in treating that as the deploy scope. The lesson is audit-first: `git log main..develop --stat` before any deploy, regardless of what the summary claims.

2. **D1 was ahead of the worker before this session — schema drift.** Migrations 0020/0021 were applied to production D1 without a corresponding worker deploy in a prior session. This session brought the worker into alignment, but the process that allowed the drift is unchanged. `deploy.sh` does not apply migrations, so the two can drift again. Worth a process fix (see §5 item 3).

3. **Rollback now leaves orphaned schema.** Pre-this-deploy, `wrangler rollback` was a clean revert. Post-this-deploy, rollback reverts the worker but leaves `event_summaries` + the new `events`/`attendees` columns orphaned. Not data-corrupting, but operationally confusing on a future audit. A full schema revert would require a manual reversing migration.

4. **The Plan 008 summary freeze write path IS verified-live (corrected).** An earlier draft of this handover claimed the path was "untested-live" and `event_summaries` was empty. That was wrong — propagated from an inaccurate session summary without independent verification. The post-deploy audit queried prod D1 directly: `event_summaries` has **1 row**, frozen 2026-06-25T04:54:57Z for `solana-in-latent-space-part-1-copy` with `no_show_count=0` (the fixed value). The lazy-on-first-read freeze path works end-to-end against real D1. Only the **manual** `POST /summary/freeze` variant remains unexercised live (lower priority — see §5 item 1).

5. **Browser-level UI rendering is untested.** Curl/protocol checks confirm the data layer (routes exist, payloads round-trip), but no Leptos component rendering was validated in a real browser. The three new surfaces (summary page, series nav, wire decoder) could still have console errors or render bugs invisible to curl.

6. **The wire bench is N/A, not skipped.** The deployed wire decode logic is byte-identical to what the GOAT-gate bench measured at commit `787e62d` (6.2× decode, 73.5% size reduction). The only wire.rs change in the deploy range (`617d7dc`) was a doc-comment correction plus a test-only `alloc_count` feature. Re-running the bench would measure unchanged code — busywork, not verification. The deploy's correctness was instead proven by the live BLAKE3 round-trip smoke test, which exercises the actual deployed encoder.

7. **This session shipped no new code.** All 41 deployed commits were authored in prior sessions. This session's contribution was: scope correction, frontend rebuild, merge topology resolution, deploy, and live verification. The handover exists to record the deploy event and its operational consequences (schema drift, orphaned-on-rollback schema), not to claim code authorship.

8. **I propagated inaccuracies from the session summary and caught them during the post-deploy audit.** The session summary I trusted claimed: (a) `event_summaries`=0 rows / freeze path untested-live, (b) "3 events" in production. Both were wrong. The post-deploy audit (this handover's §3.4 + §8 #4 corrections) verified directly against prod D1: `event_summaries`=1 row (freeze ran 2026-06-25), and the events table has **9 rows** (the "3 events" was a public-API-filtered count, not the production total). The lesson: a session summary is a claim, not a fact. When a handover records prod state, verify the prod state directly — don't inherit the prior summary's claims. The wire-format round-trip and campaigns-count claims in this handover WERE independently verified during the smoke test and remain accurate.