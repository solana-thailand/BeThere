# Plan 038: GOAT hardening + simplification package

**Created:** 2026-09-29, session `event-checkin-ba`.
**Trigger:** the "GOAT hardening + simplification" handoff, which follows
Wave 1 (`.plans/037`).
**Rule of the package:** cut first, then add.

**Standing constraints:**
- No push, no deploy, no secrets from agent sessions.
- Turnstile widgets and secrets are created by the owner.
- Every frontend change is checked in a browser at 390×844.
- The frontend first-load budget is binding: see `.issues/169`. Every "add"
  item below must report its br4 delta.

## DevEx

- [x] **D1:** `b3a00edc`. Added `[profile.fast-dev]`, an explicit rust link
  (`data-cargo-profile-dev`) and `frontend-leptos/serve.sh`. A view edit went
  from 120.6 s to 10.0 s; hot reload takes 9 s. The release dist is
  byte-identical.
- [x] **D2:** `0e1c6fae`. Removed `CARGO_BUILD_JOBS=1`. A cold build went from
  73.9 s to 35.9 s, peak RSS from 2.8 to 2.4 GB, and the wasm is
  byte-identical.
- [x] **D3:** `.issues/168`. sccache works when the target-dir path is the
  same (135/135 hits) and cannot hit across paths. The comment is fixed. The
  global `incremental=false` trade-off is the owner's call.
- [ ] **D4:** cargo-heal in the fix loop. Use it as needed; it is not a
  deliverable. The memory `cargo-heal-not-adopted` records a 2026-09-13
  evaluation.
- [x] **D5:** loop discipline, followed throughout.

## P0

- [x] **P0-1:** `8513ea26`. `utils::poll_policy`: 2.5 s in the event window,
  15 s outside it, `?poll_ms=` to override, 0 requests while hidden.
  Checked in a browser.
- [x] **P0-2:** Turnstile (`.issues/170`). Inert until the owner sets both
  keys; +5,209 B br4. Scope decision:
  - `/api/waitlist`, which is anonymous and appends to a sheet;
  - `/api/public/register`, which is JWT-gated but still scriptable;
  - **not** staff walk-in (staff-authed, and a door challenge is friction).

  It is enforced only when `TURNSTILE_SECRET_KEY` is set.
- [x] **P0-3:** `e2e/visual.spec.ts` + `e2e/a11y.spec.ts` over 7 pages ×
  390/1440, a fixture (`e2e/fixtures/seed.sql`), and CI wiring (DEV_MODE with
  a synthetic admin, `--update-snapshots=missing`, and a `visual-baselines`
  artifact).
  - Locally: 42/42 green, three runs.
  - The axe self-test proves the gate fails on injected violations.
  - **Open:** commit the Linux baselines from the first CI run's artifact.
    This machine has no container runtime, so it cannot render Linux
    baselines.
  - Found and fixed along the way: `.issues/172` (the `/api/auth/me` per-IP
    limit) and the muted-text contrast.
  - Owner decision: `.issues/171` (brand CTA fills).

## P1: cut

- [ ] **P1-1:** landing rebuild, one screen = one decision.
- [ ] **P1-2:** attendee flow audit (fold in LM-2 / LM-3).

## P2: add

- [ ] **P2-a:** inline ticket expand with SVG QR (QR only; the short-code
  column gets its own issue).
- [ ] **P2-b:** event page meta rows.
- [ ] **P2-c:** TH/EN toggle. Wave 1 already ships `LanguageSwitch` on
  attendee routes. Verify placement and the 390 px overflow only.
- [ ] **P2-d:** cookie/privacy notice.
- [ ] **P2-e:** `consent_marketing` self-serve.
- [ ] **P2-f:** sample-event button, and the version + commit in the footer.

## P3

- [ ] **P3-a:** FAQ.
- [ ] **P3-b:** discover cards.
- [ ] **P3-c:** jsQR lazy-load + Early Hints.
- [ ] **P3-d:** fluid type scale.
- [ ] **P3-e:** LINE in-app browser auth. Needs staging and a phone, so it is
  owner-run.

## Found along the way

- `.issues/169`: `size_budget_guards` is red on develop, because the
  baseline is past the warn line (from Wave 1 `e1161fdd`).
- At 390 px, the `/dashboard/live` header has a large empty gap above the
  controls. This predates this plan.
- The ticket page with a pending deposit shows "Ready for Check-In", then
  "Deposit Required", then "Verifying", all at once. This is input for P1-2
  (LM-2).
