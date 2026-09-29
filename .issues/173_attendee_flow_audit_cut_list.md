# 173: Attendee flow audit: landing → event → register → deposit → ticket → claim

**Status:** in progress (2026-09-29, session `event-checkin-ba`). Two bug-level cuts are fixed on develop (C1, C2). The rest are proposals that need the owner's approval (the brief says "approved cuts implemented"). This is P1-2 of `.plans/038`, and it absorbs LM-2 (ticket banners) and LM-3 (claim trim).

## Method

- **Tools:** headless Chrome at 390×844, full-page captures, against
  `wrangler dev --local` with the P0-3 fixture (`e2e/fixtures/seed.sql`)
  plus one unregistered probe event. The probe script is
  `/tmp/ec-ba/flow_probe.mjs`.
- **Identity:** the dev-token identity `e2e-admin@example.com`, registered
  for the fixture event with a THB deposit pending.
- **Not opened:**
  - slip upload and pending review;
  - the USDC flows;
  - refund;
  - quiz;
  - mint success.

  Those need live payment or mint state (also listed as open in
  `.plans/037`).

## Walk

| Step | Page | Height | What the attendee does | Taps |
|---|---|---|---|---|
| 1 | `/` | 3,211 px | Taps an event card (both cards fit the first screen since P1-1) | 1 |
| 2 | `/e/{slug}` (not registered) | 2,917 px | Taps "Reserve Your Spot →" (the top and sticky buttons jump to the form) | 1 |
| 3 | form, same page | — | Fills name, contact channel and handle; ticks consent; taps "Reserve My Spot" | 1 + fields |
| 4 | `/deposit/{id}` | 888 px | Taps "Pay with PromptPay →" on the only card | 1 |
| 5 | slip upload | — | not opened | — |
| 6 | `/ticket/{id}` | 1,244 px | Reads the status; the QR appears once the deposit is verified | 0 |
| 7 | `/claim/{token}` | 1,348 px | After check-in, claims the badge | 1 |

A registered visitor who opens `/e/{slug}` again goes straight to the
deposit page. That skip is right; keep it.

## Cut list

| # | Where | Finding | Proposal | State |
|---|---|---|---|---|
| C1 | claim | The countdown read `STARTS IN 28901h 24m 46s`; the event page said `1211d 5h`. Claim had its own hours-only formatter. | Reuse `public_event::types::format_countdown` (days, EN/TH). | **fixed** |
| C2 | deposit | A "RECOMMENDED" badge (CSS `::before`) on the **only** payment option. | Mark it only when there is a choice. | **fixed** |
| C3 | deposit | With one method: the brand eyebrow, a "Welcome, … / Sign out" card (the header has it), the event card, the amount three times (`฿500`, `THB`, `฿500 THB`), then the button. | With one method, show the amount once and the pay button, and drop the welcome card. | proposed |
| C4 | event | The deposit is explained three times in one card: a checklist, a 4-step "Reserve / Show up / Scan / 100% back" grid, and a "= FREE" card. There are three "Reserve" buttons (top, sticky, submit). | Keep the checklist (it carries the promises from `deposit_copy`). Cut the grid and the FREE card. Keep top + submit; drop the sticky bar once the form is on screen. | proposed |
| C5 | event | An in-page "👤 email / Sign out" card duplicates the header account chip. | Cut it. | proposed |
| C6 | event | Capacity is a full card with one number ("40 in-person spots remaining"). | Make it a meta row: P2-b ("Capacity", "เต็มแล้ว" when full). | → P2-b |
| C7 | ticket (LM-2) | With the deposit pending, the page shows three states at once: the "Ready for Check-In" hero, "Your ticket is being prepared / QR will appear once your deposit is verified", and a "Deposit Required: 500 THB — Pay Deposit Now" callout followed by a second "Ready for Check-In" callout. | One status banner derived from one state: pending deposit → "Pay deposit" (one CTA); verified → QR + "Ready for check-in". | proposed |
| C8 | claim (LM-3) | Before check-in, the step bar shows **VERIFIED ✓ → CLAIM** while the card says "Not yet checked in". There are also a "Proof of Attendance" card, a "Pending / NFT Badge Coming Soon" card and a "bookmark this page" hint. | Before check-in, show one line ("Check in at the door to unlock your badge") and hide the step bar. Merge the two NFT cards. | proposed |

## Tap count

The happy path (landing → paid) is **4 taps plus form fields**, and it has no
dead-end screens. The cuts above remove repetition, not steps. The only step
that could go is C3's chooser when there is one method (4 → 3 taps).
