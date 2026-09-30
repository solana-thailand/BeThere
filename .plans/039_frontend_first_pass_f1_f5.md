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

F1 was pushed 2026-09-29 (owner go in session); CI green at `9de1cfa0` after
the Linux baselines were re-taken.

## F2 — the keepsake ticket (`295a2a5a`, `d093a803`, pushed)

- [x] Fonts: `--font-display` (Bricolage Grotesque → Anuphan for Thai glyphs)
  on ticket/claim headings, `--font-mono` (JetBrains Mono) for asset IDs.
  Google Fonts, already in the CSP; Anuphan confirmed loading on TH pages.
- [x] Ticket: "Checked In" stamp (scale 2→1, −8°, spring), perforated tear
  line with notches clipped into the card edge, ~3% feTurbulence grain.
- [x] Claim: the success mark flips from a `--paper` card back; confetti is
  one ~1.2 s burst, skipped under `prefers-reduced-motion`.
- [x] Thai voice: 16 strings on landing/event/ticket/claim, copy only; the
  deposit promises (`utils::deposit_copy`) untouched.
- [ ] `Nº 00123` ticket number: waits for the short-code column
  (`.issues/178`); the brief forbids deriving it from IDs.
  **Blocked (2026-09-29, `event-checkin-1a`):** `.issues/178` is design-only (owner decision), and the column is a `worker/` migration (peer `event-checkin-16`).
  **Owner question (2026-09-30, `event-checkin-aa`; the peer clause is
  stale):** "do you approve `.issues/178` as written: a random 6-character
  per-event `display_code` column, a staff-only lookup, and a backfill
  migration, landing after the 8 Oct take?"
- Verified at 390×844 EN + TH; a11y allowlist still empty. CI after push:
  only the 2 re-taken ticket baselines failed, as designed.

## F4 — generative posters

- [x] `src/utils/poster.rs`: a risograph-style SVG seeded by FNV-1a of the
  slug (paper ground, 2–3 brand inks multiplied off-register, grain), as a
  `data:` URL. Tests: `tests/generative_poster.rs` (determinism, variety,
  palette-only colours, URL-safe, < 4 KB).
- [x] Event hero third tier (no poster, no badge) and /discover thumbnails
  for rows with no image of their own.
- [ ] OG image: not done. Social cards need PNG, and SVG→PNG in the Worker
  does not fit the free-plan CPU cap (`free-plan-cpu-cap-is-binding`).
  Options: a build-time/offline render per event, or a paid plan.
  **Blocked (2026-09-29, `event-checkin-1a`):** owner picks the option (paid plan is a cost call; per-event render needs a pipeline decision).
  **Owner question (2026-09-30, `event-checkin-aa`):** "for OG images, a paid
  Workers plan (render SVG→PNG in the Worker), or a PNG rendered per event
  offline and uploaded to R2 on save?"

## Remaining

- [x] **F5** LINE in-app browser login on staging — owner phone test.
  **Done (2026-09-30, owner, reported to `event-checkin-b5`):** the owner has logged in on a phone and it worked. That also closes `.plans/038` P3-e.
  **Blocked (2026-09-29, `event-checkin-1a`):** owner, on a real phone with LINE (same gate as plan 038 P3-e).
  **Owner question:** the one in `.plans/038` P3-e; one phone run closes both.
