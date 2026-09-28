# 036 — Relentless simplification (standing rule + candidate list)

**Status:** open. Created 2026-09-28 (owner asked, session `event-checkin-af`).
**Rule:** every change leaves the system simpler than it found it. Big cuts wait
until after the 12 Oct submission; small ones happen between tasks.

## Why

Most real defects found on 2026-09-28 had one root: a rule written in several
places that drifted.

| Defect | Copies |
|---|---|
| `.issues/157` walk-ins counted as online | 8 counting sites that disagreed |
| Deposit promise out of date after D1 | 6 surfaces (4 in code) |
| Walk-ins double-counted, bank data in 3 places (`.issues/160`) | 2 stores for attendees: D1 and Google Sheets |
| `.issues/161` consent bundling | 1 checkbox standing in for 4 consents |
| `.issues/141` a gate nobody could pass | every prod deploy used `--force` |

## How (the three guardrails)

1. **One home + a guard test that can fail.** The pattern that worked:
   `worker/tests/capacity_count_single_home.rs`,
   `frontend-leptos/tests/deposit_promise_has_one_home.rs`,
   `frontend-leptos/tests/date_formatting_has_one_home.rs`.
2. **A simplification is verified like any change:** a failing-then-passing
   test, staging, and opening the page for frontend work.
3. **Know why before you delete** (Chesterton's fence): read `git log` for the
   thing first, and write the reason in the commit.

## Candidates (measured 2026-09-28)

| # | Candidate | Measure now | Target | When |
|---|---|---|---|---|
| S1 | Attendees: D1 is the only store; the Sheet becomes a one-way export, never read back | 2 sources (`sheets::get_attendees_inner` falls back to Sheets on an empty D1) | 1 | after 12 Oct (L) |
| S2 | Hide the USDC rail in the UI while escrow is devnet-only and off for every event | `show_usdc` / `escrow_closed` branches on the event page, registration and landing | 0 USDC branches on THB-only events | after 4 Oct (S–M) |
| S3 | `notifications::dispatch` (email sending): wire it or delete it | defined in `worker/src/notifications/mod.rs:71`, 0 callers; sending off (`NOTIFICATIONS_ENABLED = "0"`) | 0 dead entry points | after 12 Oct, owner call (email is a product decision) |
| S4 | `slip_vision.rs` (Anthropic vision), dormant under the 0-THB running-cost rule | off in prod and staging (no `ANTHROPIC_API_KEY`) | keep dormant with a test, or delete | owner call; not before the submission (the AI story names it) |
| S5 | `scripts/make_pitch_deck.py`, legacy deck copy that contradicts D1/D4/D5 | stale | deleted, or aligned | before 12 Oct only if the submission uses it; else after |
| S6 | `worker/scripts/preflight.sh` + flow-harness as a gate | no longer gates (`37bc4764`) | keep as a manual tool, or delete if unused by 31 Oct | after 12 Oct |
| S7 | Consent: 4 signals behind 1 checkbox | 1 checkbox → 4 consents | 1 checkbox per consent | after 4 Oct (`.issues/161`) |
| S8 | Unmerged branches waiting on one date | 5 (W3, W4, W5, log key, CSP) | 0 on 5 Oct; then branches live ≤ ~1 day | 5 Oct |
| S9 | ~~Issue ledger heuristics for legacy (≤144) free-text statuses~~ | — | **Declined 2026-09-28:** that parsing found a real stale claim the same day (#121 said "awaiting deploy" while in every prod tag). Removing a check that just caught something is not simplification. Keep it; stop only if it produces noise. | — |
| S10 | Plan 028 bullets of 300+ words | — | **Done 2026-09-28:** a status-at-a-glance table on top; the detail stays below as the log. | — |

Remote branches were 9 after `git fetch --prune` (an earlier "~70" count
included stale refs), so branch cleanup is not a candidate.

## Metrics to keep at their targets

- `--force` on prod deploys: **0** (the staging-parity gate can pass).
- Sources of truth per entity: **1**.
- Rules with one home and a guard: add one each time a duplicate is found.

## Log

- 2026-09-28: `.issues/141` gate replaced by staging parity (`37bc4764`);
  deposit promise moved to `utils::deposit_copy` (`17df354e`); W3 made the
  capacity count single-home (branch `feature/028-w3-track-counts`).
