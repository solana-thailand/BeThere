# Plan 034: UX overhaul along the three journeys

**Status:** open. Proposed 2026-09-27 by `event-checkin-89` at the owner's request.
**Supersedes** `docs/ux_roadmap.md` (audit dated 2025-05-10; stale).
**Timing:** Part A is small, on-camera fixes inside the plan 033 window.
Part B starts **Mon 13 Oct**, after both submissions. Plan 033 §1 still
holds: spending the window on UI polish loses.

## 0. Evidence

A read-only walkthrough on 27 Sep covered every route in
`frontend-leptos/src/lib.rs` at 390×844 and 1440×900.
- Public pages were opened on prod.
- Admin, staff and ticket pages were opened on staging with `dev-token`.
- Screenshots and metrics are in `/tmp/ux_audit/` (local only, not in git;
  re-run `probe.mjs` there to refresh).
- Baseline: no horizontal overflow on any page. Content shows in 0.2–1.7 s,
  and the only console noise is SRI-preload warnings.

Checked in code after the walkthrough (not only seen in screenshots):
- The USDC choice is a clickable `<div>`, not a button
  (`pages/deposit/choose_payment.rs:207`).
- The landing FAQ says "No-shows forfeit their deposit to the organizer" and
  "refunded on-chain" (`pages/landing/page.rs:386,404`). Both contradict the
  owner rule of 17 Sep (no forfeiture; THB is off-chain, returned as a refund
  or kept as credit).
- The scanner's event bar is not visible at 390 px
  (`/tmp/ux_audit/30b_staff_viewport_390.png`).

**Not seen:** a populated slip queue; a not-yet-checked-in ticket (full QR);
the registration and create-event forms; a real claim link; real camera
scanning. Part B starts by seeding staging with these states.

## Part A: inside the window (before RTM #6 on 4 Oct)

Only items that are on camera, a door-day risk, or an honesty problem.

| # | Fix | Why now | Where |
|---|---|---|---|
| A1 | Landing copy: drop "forfeit", say THB is off-chain and returned as a refund or credit; soften the "100%" card | A judge reads the landing page; the copy contradicts the product. Misrepresentation risk. | `pages/landing/page.rs` |
| A2 | Scanner: the active event name is always visible at the top; the picker is not covered by the video | Door-day risk at RTM #6 | `pages/scanner/page.rs` |
| A3 | Deposit: USDC becomes a real button, like the PromptPay card | On camera in the W2/W3 demo | `pages/deposit/choose_payment.rs` |
| A4 | Ticket: event name as the heading; `in_person` shown as "In-person" | On camera | `pages/ticket/in_person_view.rs` |
| ~~A5~~ | Moved to Part B (B2), 27 Sep. The upload already refuses an unknown attendee (`slip_upload.rs:152`), so no deposit lands on a bogus id. But the fix changes a public read path on the money page (`deposit/usdc/handlers/status.rs` falls back to the active event), and that is not worth doing a week before RTM #6. | — | — |

Each fix is verified by opening the page ([[verify-frontend-by-opening-it]]).

## Part B: from 13 Oct

### B1. First-time visitor (landing)
- Phone: Find Events and the next event above the fold. One "BeThere", not
  two. Stat cards in a single row.
- "Create an Event" explains who can create and routes to the waitlist, not a
  bare `/login`. "Join Waitlist" must not look disabled.
- One language policy for the whole site: `/discover` is Thai while the landing
  page is English, and `/e` mixes both.

### B2. Attendee: `/e` → register → deposit → ticket → door → badge → recap
- `/e`: show participation mode (online is exempt) and the deposit before
  sign-in, or say why sign-in comes first. Use the desktop width (it is one
  700 px column now).
- Deposit: `/deposit/{unknown id}` shows the active event's pay screen
  (`GET /api/deposit/status` resolves the event and never checks that the
  attendee exists). Return 404 when the attendee lookup says "absent", and
  fail open on a lookup error. A person on a mistyped link could transfer
  money before the upload refuses them.
- Claim: replace the raw "API error (0): not found…" with friendly text and a
  "find my ticket" path (`pages/claim/page.rs:215`).
- Recap: prod has zero past events, so the last step of the journey is empty.
  Decide what the recap shows before the first event ends.

### B3. Organizer: create → share → queue → door → summary / PR pack
- Admin events: filter or group drafts, test and archived events (22 in one
  flat list). Delete becomes destructive-styled and moves into an overflow
  menu.
- In-person tab, mobile: eight equal toolbar buttons; the row shows a chip
  where the name should be; "Record Slip" is clipped.
- Scanner: scanning is the primary action and walk-in is secondary.
- Deposits queue: an empty state with a next step; review it populated.
- Summary: mobile layout; don't show "0.00" for a figure the page says is
  not tracked.
- PR pack: Bangkok time as an option (UTC is by design, `domain/src/pr_pack.rs:12`).

### B4. Sweep
- Tap targets: every 15–20 px link (back links, ticket footer, landing footer
  and nav, scanner "Enter manually" and gear) to at least 44 px.
- Hide the public `/checkin/nfc` stub. `/dashboard` still names Helius (retired).
- `/dashboard/live`: empty gap under the header on mobile.

### B5. Gate
Turn `/tmp/ux_audit/probe.mjs` into `scripts/verify/ux_probe.mjs`:
- It fails on horizontal overflow, on tap targets under 44 px on the primary
  pages, and on console errors.
- It needs a `--self-test` against a fixture page that breaks each rule.

## Acceptance
- Part A: A1–A5 are on staging, opened at 390 px and 1440 px, and screenshots
  are attached to this plan's log.
- Part B: every journey is walked end to end on seeded staging with no dead
  end, and B5 is green in CI.

## Log

### 27 Sep: Part A (A1–A4) on staging, not prod

Staging `0a71a857` = git `052cdf3c`. Opened in headless Chrome
(`/tmp/ux_verify/v.mjs`), no page errors:
- **A1 landing (390 px):** no "forfeit" and no "recorded on Solana";
  "off-chain" and "Back When You Attend" are present. A/B: the same probe on
  prod (old code) finds "forfeit", so the probe can fail.
- **A2 scanner (390 px):** the event bar is at y=69, and
  `elementFromPoint` at its centre hits the `<select>`, not the `<video>`.
- **A3 deposit (USDC-only fixture):** "Pay with USDC →" is a button; clicking
  it opens the Choose → Connect → Pay steps.
- **A4 ticket (390 and 1440 px):** the heading is the event name; "In-Person"
  is shown, not `in_person`.

The smoke test's fixture cleanup returned 400 once; a manual archive + delete
right after returned 200. Not yet explained.

Not on prod. A1 is the one that matters before judges read the site; it
ships with the Fri 3 Oct deploy (owner go).
