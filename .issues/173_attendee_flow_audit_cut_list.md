# 173: Attendee flow audit: landing → event → register → deposit → ticket → claim

**Status:** fixed on develop (2026-09-29, session `event-checkin-3e`): C3, C4, C5, C7 and C8 approved by the owner and applied; C1/C2 were fixed earlier (session `event-checkin-ba`), C6 went to P2-b.

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

## Applied (2026-09-29, owner approved C3–C5, C7, C8)

- **C3** deposit: with one method the card drops its name and amount (the
  hero says it once); the welcome/sign-out card and the `/api/auth/me` fetch
  that only fed it are gone.
- **C4** event: the 4-step grid and the "= FREE" card are cut; the refund
  checklist stays. The sticky bar hides while `#reserve` is on screen (scroll
  listener, removed on cleanup) and comes back above it.
- **C5** event: the "👤 email / Sign out" card is cut. **Correction to the
  premise:** attendee pages have no header account chip, so the card was the
  only switch-account exit. It moved next to the locked email in the reserve
  form ("Sign out", then reload the event page), the one place the email is
  shown.
- **C7** ticket: one derived state. An unpaid deposit is its own hero
  (`DepositDue`, "Deposit Required") with the pay card as the only CTA, no QR
  placeholder, no second "Ready" card and no "present this ticket" footer.
  The pending/ready descriptions moved into the hero subtitle.
- **C8** claim: before check-in (in-person, no check-in time) the stepper is
  hidden and the card reads "Check in at the door to unlock your badge.",
  followed by the badge preview. After check-in the status card no longer has
  a separate preview card beside it.
- Checked at 390×844 in headless Chrome against the local e2e worker: sticky
  hides at the form and returns at the top, one amount on the deposit page,
  no overflow. Playwright 45 passed; the 6 failures are the intended visual
  changes (event, ticket, claim × 2 viewports). First load −2,969 B br4.
