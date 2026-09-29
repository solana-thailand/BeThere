# Plan 039: frontend-first pass (handoff "FINAL v3", F1–F5)

**Created:** 2026-09-29, session `event-checkin-3e`.
**Trigger:** the owner's frontend-first handoff of 2026-09-29 evening.
**Hard deadline:** the demo take on Thu 8 Oct (`.issues/164`). From 6 Oct,
do not destabilize demo-facing flows (event, deposit, ticket, claim, scanner).
**Rules of the package:** no push, no deploy (the owner does both); stage
named files; verify every frontend change at 390×844.
`.issues/167` part B is parked until this package lands.

## Already done before this plan (verified against the tree, not redone)

- D1 fast-dev profile + `serve.sh`, D2 no `CARGO_BUILD_JOBS=1`, D3 sccache
  (`.plans/038` DevEx, `.issues/168`).
- F3 inline ticket, meta rows, TH/EN toggle (`.plans/038` P2-a/b/c).
- F4 FAQ, discover poster cards, fluid type scale (`.plans/038` P3-a/b/d).

## F1 — landing + Your Events + emoji purge

- [x] **F1-a** (`cae4733e`): the landing account card is gone; the topbar
  chip carries identity and sign-out. "Verified" moved next to the email on
  `/profile`.
- [x] **F1-b** (`cae4733e`): one row per registration, reusing /discover's
  `DateChip` + `dv-row`: event date, name, status text, chevron; the row links
  to the ticket. A full-width button only for deposit / claim / quest.
  Grouped Upcoming (n) / Past (n); past newest first, 3 shown + "Show n more".
  - The cards already used the event date, not the registration timestamp;
    that part of the brief was stale.
  - Checked on an 8-registration local fixture (not committed): no email per
    row, rows span 425–1215 px at 390×844, "Show 1 more" reveals the 8th.
- [x] **F1-c** (`b211d206`): emoji removed from every catalog and attendee
  page (icons or CSS instead); gate `frontend-leptos/tests/emoji_audit.rs`.
  A planted emoji in a locale and in a source file each fail it. Adventure's
  sprite art is exempt; staff pages are out of scope.
- [x] **F1-d** (`cae4733e`): the organizer block is one line, "Host your
  event →". Organizers go to `/admin`. Everyone else gets a disclosure over the
  waitlist: `ProtectedRoute` checks sign-in, not role, so `/admin` would be a
  dead end for a would-be organizer. (Deviation from the brief, on purpose.)
- [x] **F1-e** (`a54a44ae`): not reproduced — Chromium scrolled the document
  on all 9 attendee pages checked. Hardened anyway: sideways clipping on html
  only (`hidden` then `clip`), none on body, so body can never become an inner
  scroller in WebKit.
- Snapshots: darwin refreshed locally (51/51). The 8 Linux baselines F1
  changed are deleted (`b4f9f82a`); the first CI run after the owner pushes
  will be red on them by design — commit the `visual-baselines` artifact.
- Size: attendee first load 1,212,047 B br4 (+401 B vs baseline), staff
  shell 1,990,631 B (+2,397 B). Both gates green.

## Remaining

- [ ] **F2** art pass, ticket + claim first (display + Thai display + mono
  fonts, CHECKED IN stamp, perforation, `Nº` number, paper grain, claim
  reveal, Thai copy voice). Land before 6 Oct only if clean.
- [ ] **F4** generative SVG posters from the event slug (no-poster hero, OG
  image, discover thumbnail).
- [ ] **F5** LINE in-app browser login on staging — owner phone test.
- [ ] Owner: push; then commit the Linux baselines CI writes.
