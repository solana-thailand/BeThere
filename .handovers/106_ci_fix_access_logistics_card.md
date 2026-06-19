# Handover 106 — design-lint CI Repair + Access & Logistics Ticket Card

> **Branch**: `feature/r2_lightbox_admin` (7 commits ahead of `develop`, pushed to `origin`)
> **Status**: ✅ **Pushed** + **PR #15 open** against `develop`, `mergeStateStatus: CLEAN` / `mergeable: MERGEABLE`, both CI checks green. **NOT deployed** — prod-affecting, awaiting operator go-ahead + D1 backup.
> **Commits**: `a59733b` (CI fix) + `e0d1620` (feature) added on top of handover #105's 5 commits
> **Predecessor**: handover #105 (R2 fix + participation override + ImageLightbox)
> **Created**: 2026-06-19

---

## 1. What Happened

Two distinct pieces of work, both landing on the same `feature/r2_lightbox_admin` branch as #105:

1. **CI repair** — PR #15 was found blocked in an `UNSTABLE` state by a pre-existing
   `design-lint` workflow bug. Diagnosed across two iterations and fixed (commit `a59733b`).
   This was the **first time `design-lint` ever passed** since the workflow was added in `e514218`
   (5/5 prior runs failed — drift detection had been silently masked on every PR touching
   `style.css`).

2. **Product gap closure** — the attendee confirmation calendar/email promises
   "building access guide, ID exchange procedure, and transportation details on your Ticket page,"
   but no such feature existed. Researched options, chose **Option A** (reuse `community_links`
   with `platform: "guide"` tag — zero schema change), and built end-to-end (commit `e0d1620`).

Neither change touches production. Both compile clean (`cargo check` + `cargo clippy` on
`event-checkin-frontend`).

---

## 2. Root Cause / Analysis

### 2.1 design-lint workflow bug (commit `a59733b`) — 🟠 CI blocker, every style-touching PR

`grep -oP` was used in **six** places across `.github/workflows/design-lint.yml` to extract design
tokens from `style.css` and from the brief markdown. Two of the six relied on
**variable-length lookbehind assertions**, which GNU grep `-P` (PCRE2) rejects outright:

- CSS lines: `(?<=--bg-primary:\s*)` — the `\s*` is variable-length
- Brief lines: `(?<=--bg-primary.*\|\s)` — the `.*` is variable-length

Both raise: `grep: lookbehind assertion is not fixed length` (exit code 2). This is a hard error,
not a "no match," so the **entire** drift comparison step was skipped on every run.

**Fix:** replaced both lookbehinds with semantically-equivalent **`\K` reset patterns** (which
match variable-width content before the reset point). Added `--` option terminator on the CSS
patterns since they now start with `--bg-...` (prevents grep from parsing the token as a flag).

**Validation:** `rg -P` against the real files yields the same three hex values the originals
intended: `--bg-primary: #13131b`, `--bg-secondary: #1a1a24`, `--bg-card: #1e1e2a`. No actual
design drift existed.

> Tooling notes:
> - macOS BSD `grep` lacks `-P` entirely — local validation required `rg -P` (PCRE2).
> - `rg -P` reproduces the **same** PCRE2 error (`length of lookbehind assertion is not limited`),
>   confirming it's a fundamental regex limitation, not a libpcre quirk.
> - Zed's format-on-save re-applies cosmetic YAML quote-style changes (single → double quotes) on
>   every `edit_file` save. Workaround: revert via `sed` in terminal (external writes bypass
>   editor formatting).

### 2.2 Access & Logistics card (commit `e0d1620`) — 🟡 product gap, no prod-impact on its own

The calendar/email promise covers **four** items; only one (QR code) existed on the ticket page:

| Item | Status before | Status after |
|---|---|---|
| Check-in QR code | ✅ `QrSection` | ✅ unchanged |
| Building access guide | ❌ | ✅ via guide link |
| ID exchange procedure | ❌ | ✅ via guide link |
| Transportation details | ❌ | ✅ via guide link |

**No logistics/access fields existed in the data model.** The closest were `EventConfig.location`
(single string), `EventConfig.description` (markdown, never rendered as markdown), and
`CommunityLink { platform, url, label }` — the only organizer-configurable links mechanism, already
wired end-to-end (model → API → admin form → ticket + public event render).

**Key insight:** `CommunityLink.platform` is a free-form string with no validation. Adding
`"guide"` as a category tag required **zero schema change** — just (a) a new admin form option,
(b) a render path on the in-person ticket, and (c) a filter to keep guide links out of the social
section everywhere else.

**Why Option A (over a dedicated table or markdown rendering):**
- Lowest risk for an event happening soon — no D1 migration, no markdown dep, no XSS surface.
- Reuses the entire `community_links` pipeline that organizers already know.
- Filter-based partition (`platform == "guide"`) is reversible and additive.

---

## 3. Changes Made

- Diagnosed the `design-lint` failure as a regex bug (variable-length lookbehind), not design drift.
- Fixed all affected patterns with `\K` reset + `--` terminator.
- Researched three options for the Access & Logistics gap (dedicated table / markdown rendering /
  reuse `community_links`), picked Option A.
- Built the feature across 6 files (183 insertions, 4 deletions).
- Validated both with `cargo check` + `cargo clippy` on `event-checkin-frontend` (EXIT 0 each).
- Both commits pushed to `feature/r2_lightbox_admin`. PR #15 title/body updated to reflect added scope.
- No deploy.

---

## 4. Files Modified (by commit)

**`a59733b` fix(ci): replace invalid variable-length lookbehinds in design-lint**
- `.github/workflows/design-lint.yml` (2 lookbehinds → `\K` resets; `--` terminator on CSS patterns)

**`e0d1620` feat(ticket): Access & Logistics card for in-person attendees**
- `frontend-leptos/src/pages/ticket/in_person/access_logistics.rs` (new — renders card from
  `platform == "guide"` links)
- `frontend-leptos/src/pages/ticket/in_person/mod.rs` (module declaration)
- `frontend-leptos/src/pages/ticket/in_person_view.rs` (partition `community_links` into
  guide/social; render card after QR; pass filtered social links to community section)
- `frontend-leptos/src/components/community_links.rs` (filter `platform == "guide"` out globally —
  defense-in-depth: ticket, online ticket, public event page)
- `frontend-leptos/src/pages/admin/event_form.rs` (+1 platform option
  `<option value="guide">Guide (logistics)</option>`; updated section hint)
- `frontend-leptos/style.css` (+71 lines card styling; extends `ticket-action-card--info` base)

---

## 5. Reflections

### What went well
- First-iteration misdiagnosis caught on the second CI run (initial fix only repaired the brief.md
  lookbehind; the CSS extraction lines had the **identical** bug). Caught before any broken state
  lingered — re-amended and force-pushed.
- Zero schema change was the right call. The same `CommunityLink` struct supports both social and
  guide content purely via the `platform` tag, which means no migration risk for an imminent event.
- Validation done **before** commit (`cargo check` + `cargo clippy` both EXIT 0).
- PR #15 was deliberately **bundled** rather than split: `style.css` is touched by both the
  ImageLightbox work (#105) and this card, so clean branch separation is error-prone. PR title/body
  updated to disclose the added scope.

### What was struggled with
- The `design-lint` bug is **pre-existing** (lives in `develop` via `e514218`) and has been silently
  failing for months. Any future PR that touches `style.css` would have been blocked the same way
  until this fix lands. Worth cherry-picking to `main` if `main` is on a similar workflow.
- Zed format-on-save fighting cosmetic YAML quote-style was annoying. Worked around with terminal
  `sed` rather than fighting the editor.

### What was solved
- Surfaced that **prod is currently unguarded** by `design-lint`'s drift detection (the check has
  never passed, so it's been a no-op signal). The fix turns it back on for real.
- Closed a real product gap (3 of 4 promised items) with one commit and no DB risk.

---

## 6. Remain Work

### Immediate (operator action — all prod-touching or interactive)
- [ ] **D1 backup before any deploy**:
      `npx wrangler d1 export bethere-db --output backup-pre-demo.sql`
- [ ] **Decide PR strategy**: merge PR #15 as one (now 7 commits: 4 features + CI fix + 2 docs), OR
      cherry-pick `a32215d` (R2 fix) to a hotfix for fast prod turnaround. Commands in #105 §7.
- [ ] **Deploy + verify R2 fix** (P0, highest-impact): poster loads on `/e/{slug}` (R2 read) +
      THB slip upload succeeds (R2 write).
- [ ] **Verify participation override**: flip one attendee In-Person ⇄ Online; confirm Sheet col I +
      D1 + `ParticipationTypeChanged` audit entry.
- [ ] **Verify lightbox + background** on `/e/{slug}`: open/close poster, slip, QR; fade animation
      + no overflow + latent-space bg.
- [ ] **Verify Access & Logistics card** (new this session):
  1. As organizer, add 3 community links with platform "Guide (logistics)" and labels
     "Building Access Guide" / "ID Exchange Procedure" / "Transportation" pointing to real doc URLs.
  2. As an in-person attendee, open the ticket → confirm "Access & Logistics" card appears after QR
     with all three links.
  3. Confirm guide links do NOT appear under "Join the Community".
  4. Confirm online attendees do NOT see the card.
- [ ] **P0.2 CPU-time measurement** (still genuinely undone): `npx wrangler tail` in an interactive
      terminal, exercise the 4 hot paths, read `cpuTime` per invocation (>7ms = flag,
      >10ms = already failing in prod).

### Optional follow-ups (non-blocking, surfaced during this session)
- [ ] **Separate PR for Access & Logistics** (if desired for clean revert/review history):
      cherry-pick `e0d1620` onto a fresh `feature/access_logistics` branch off `develop`. Content
      merge should resolve cleanly since CSS regions don't overlap.
- [ ] **Raise community-links cap** from 5 if organizers need more slots (3 social + 3 guide would
      already exceed the current cap).
- [ ] **Dynamic label placeholder** for guide links (currently "optional"; falls back to
      "View Guide" if blank).
- [ ] **Cherry-pick `a59733b`** (design-lint fix) to `main` if `main`'s workflow is also broken.

### Carried over (from handover #105, unrelated)
- [ ] KV write usage re-check: `python3 scripts/diag_kv_usage.py --days 3`
- [ ] Handover #104's orphan-toast visual verify on audience export.
- [ ] Confirm prod worker is at `main` HEAD (so merged KV fix `ebd0e97` is live).

---

## 7. How to Dev/Test

### Checkout + build the branch
```bash
git fetch origin
git checkout feature/r2_lightbox_admin
cargo clippy -p worker --quiet                       # EXIT 0 expected
cd frontend-leptos && cargo check --target wasm32-unknown-unknown -p event-checkin-frontend
cd .. && rg -P "(?<=--bg-primary:\\s*)#[0-9a-fA-F]+" frontend-leptos/style.css   # CI regex sanity
```

### Reproduce the design-lint fix locally
```bash
# Before fix (would fail):
rg -P "(?<=--bg-primary:\s*)#[0-9a-fA-F]+" frontend-leptos/style.css
#   → rg: regex query error: look-behind assertion is not fixed length

# After fix (passes):
rg -P -- "--bg-primary:\s*\K#[0-9a-fA-F]+" frontend-leptos/style.css
#   → #13131b
```

### Test the Access & Logistics card locally
1. Run the worker locally + open admin event form.
2. Add 1-3 community links with platform "Guide (logistics)".
3. Open the in-person ticket view → "Access & Logistics" card should render after the QR section.
4. Open the online ticket view → no card should render.
5. Open `/e/{slug}` → guide links should NOT appear in the public community section.

### Clippy gate (use the verified command)
```bash
cargo clippy -p worker --quiet
# NOT --all-targets — fails with wasm_bindgen_test errors in worker-0.8.1 dep test code (#104)
```

---

## 8. Issues Ref

- Predecessor: handover #105 (R2 fix + participation override + ImageLightbox)
- Related (R2 binding): handover #080
- Related (design-system drift): handover #063 (drift fix that `design-lint` was supposed to guard)
- Branch: `feature/r2_lightbox_admin` (7 commits: `a32215d → e0d1620`, pushed to `origin`)
- **PR**: #15 — `feature/r2_lightbox_admin` → `develop` (open, `CLEAN`/`MERGEABLE`, both checks green)
- **Prod URL**: https://bethere.solana-thailand.workers.dev
- Remote: `git@github.com:solana-thailand/BeThere.git`
- Production state: **unchanged** — branch pushed + PR open, nothing deployed yet.

---

## 9. Commit Plan

Commits on `feature/r2_lightbox_admin` (in order, oldest first):

1. `a32215d fix(worker): bypass worker 0.8.x R2 get/put null-serialization bugs` *(#105)*
2. `016be49 feat(attendee): manual participation_type override + admin sheet link` *(#105)*
3. `7d3c07f feat(frontend): shared ImageLightbox + latent-space event background` *(#105)*
4. `e94f6ec docs(plan): add #010 free-tier optimization consolidated view` *(#105)*
5. `53e279c docs: add handover #105 + correct stale KV-fix status in plan #010` *(#105)*
6. `a59733b fix(ci): replace invalid variable-length lookbehinds in design-lint` *(#106)*
7. `e0d1620 feat(ticket): Access & Logistics card for in-person attendees` *(#106)*

**Status**:
- ✅ Pushed: `git push -u origin feature/r2_lightbox_admin`
- ✅ PR #15: `feature/r2_lightbox_admin` → `develop` (open, both CI checks green)
- ✅ Validated: worker clippy EXIT 0, frontend wasm check EXIT 0
- ⏳ Operator: D1 backup → decide PR strategy → deploy → verify (R2 / participation / lightbox /
  Access & Logistics card)
- ⏳ Operator: P0.2 `wrangler tail` CPU-time measurement (interactive, can't be automated here)
