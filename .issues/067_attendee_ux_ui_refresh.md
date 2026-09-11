# 067 — Attendee UX/UI refresh

**Status:** Open  
**Priority:** P1  
**Created:** 2026-09-11

## Goal

Make the path from event discovery to registration, deposit, ticket, quiz, and
claim easy to understand on a phone, especially where money or an external
wallet action is involved. Visual changes must preserve accessibility, fast
loading, reduced-motion behavior, and the event configuration as product truth.

## Design direction

The useful parts of Figma's 2026 trend review are vibrant but semantic color,
strong typography, restrained motion, progress feedback, and sustainable Web
design. BeThere should avoid experimental navigation, heavy 3D, maximalism,
neumorphism, and novelty interactions in transactional flows.

## Delivery order

- [x] Remove fixed `0.01 SOL` and ambiguous `SOL/USDC` landing claims.
- [ ] Define shared semantic states for pending, action required, confirmed,
      blocked, failed, and refundable; do not encode meaning by color alone.
      The landing registration cards now reuse the shared semantic badge
      primitives without inline hex colors; extend their vocabulary across
      flows.
- [ ] Add one attendee journey indicator shared by registration, deposit,
      ticket, quiz, and claim, driven by server state.
- [ ] Show exact currency, network, amount, refund condition, and fee ownership
      immediately before every wallet signature.
- [ ] Give pending external operations an idempotent retry action and explain
      whether the attendee can safely close the page.
- [ ] Add reduced-motion support for progress and success transitions.
- [ ] Validate 320 px mobile layouts, keyboard order, focus visibility, screen
      reader labels, and WCAG AA contrast.
- [ ] Measure cold/warm LCP, INP, CLS, initial WASM/JS size, hero image bytes,
      and public-event API latency before adding decorative assets.

## Acceptance journeys

1. New attendee: slug page → registration → required deposit → ticket.
2. Returning attendee: landing “My registrations” → resume exact next step.
3. Checked-in attendee: ticket → quiz/adventure when required → NFT claim.
4. Failure recovery: rejected/pending deposit, wallet rejection, confirmation
   timeout, mint pending, and retry without duplicated payment or mint.
5. Refund: pre-event blocked state, checked-in refund, no-show deadline, and
   held-as-credit result with the next action visible.

## Evidence required

- Browser E2E for each acceptance journey and its principal failure state.
- Axe/accessibility checks plus manual keyboard and mobile verification.
- Core Web Vitals trace for landing and one representative public slug page.
- Screenshots in the PR at 320 px and desktop widths for changed surfaces.

## Reference

- Figma, “Top Web Design Trends for 2026”:
  <https://www.figma.com/resource-library/web-design-trends/>
