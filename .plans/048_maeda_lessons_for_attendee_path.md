# 048 — Lessons from John Maeda's talk, applied to the attendee path

**Status:** proposal, nothing built. Source: "John Maeda's Simple Recipe for
Better Products in the Age of AI" (Convex, YouTube `w6BJ6lXxB0I`, 30 min,
read from auto-captions, so a few words are garbled). Owner asked 2026-10-22 to
learn from it and adapt. The mapping below is our inference, not his claims.

## What he said (short)

1. Good design = familiar + novel (MAYA, most advanced yet acceptable). Too
   advanced is a defect.
2. A product needs good decisions and good engineering. Craft is a thin layer
   on top, not everywhere.
3. One shared frame for a team: conversion → onboarding (first success, fast)
   → retention → expansion.
4. Agentic era: the user states a goal and the system gets there. Design moves
   from the gulf of execution (help me do it) to the gulf of evaluation (make
   me sure it was done right). If the interface goes away, the new interface is
   probably built for agents.

## Mapping to BeThere

| Idea | Where it bites here | Candidate |
|---|---|---|
| Gulf of evaluation | The money worry is "did my deposit land and will I get it back", see `.issues/194` | A per-ticket deposit status line: held / refunded / kept as credit, with the date. **Audit first**: `credit_chip.rs` and `credit_refund_card.rs` already cover credit; find what a THB deposit shows today before adding anything |
| Onboarding = clicks to first success | Plan 047 Phase 2 | Count clicks from the event page to a ticket; the live form still asks for participation type, deposit and custom fields |
| Retention | After "Agree and watch" there is no next step | On a watch-back ticket, offer the next open event and other decks by the same speaker |
| MAYA | Night Edition and new features | Test: if it needs explaining, it is too advanced |
| Built for agents | `.plans/045` SG2 (x402) | Read-only status API an agent can poll. Needs a consent and security decision first; do not build before that |

## Order

1. Audit what the ticket shows for a THB deposit today (read the code, then
   open the page; do not trust this table).
2. If there is a gap, add the status line behind the existing ticket layout
   without moving live tickets (the visual baselines pin them).
3. Measure clicks for plan 047 Phase 2 before designing one-click for live
   events.
4. Retention and agent API stay ideas until the owner picks one.
