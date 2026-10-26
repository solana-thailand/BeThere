# 049 — What makes customers like the product: five asks, sized for BeThere

**Status:** in progress. Item 4 first (no owner decision needed). Owner asked 2026-10-27 to
assess a post listing five tips and adapt them ("go").

Two kinds of customer, and the tips land differently: **attendees** (phone, a few visits a year)
and **organizers** (use it for real, may pay).

## The five, checked against the code on 2026-10-27

| # | Tip | Where BeThere is | Plan |
|---|---|---|---|
| 1 | Uncluttered, little scrolling, few taps | One click and profile prefill shipped (plan 047, pull 286/296); never *measured* | Measure clicks and screen-heights for 4 flows (register, deposit, door check-in, organizer creates an event) with the headless probes; fix the worst |
| 2 | A guided tour with voice and motion | None | Last. An attendee who needs a tour is a screen to fix (item 1). Organizer onboarding only, and only once outside organizers get stuck; a tour library costs wasm near the staff 2 MiB ceiling |
| 3 | Short explainer clips | Event recordings only | Two 60–90 s clips with the fframes/ffilms pipeline: attendee (register → deposit → check in → money back) and organizer (create an event in 3 minutes) |
| 4 | Docs for AI, as Markdown | `/llms.txt` exists (`worker/src/crawl.rs`, guarded by `tests/crawl_routes.rs`), but every page it links renders only with wasm, so a crawler reads nothing behind the links | Serve Markdown pages built from text the site already says (FAQ, organizer page), link them from `llms.txt` |
| 5 | API, webhooks, tokens with limited scope | Inbound webhook only (Helius escrow indexer); no outbound webhooks; no third-party keys | Design together with `.issues/195`: one per-organization scoped API key (the smoke key is one scope), and outbound webhooks fed by the existing notification outbox. Design first; auth change needs the owner |

## Order

1. Item 4 (this branch).
2. Item 1: measurements first, recorded under `.benchmarks/`.
3. Item 5: design doc, owner decision, then build.
4. Item 3: clips.
5. Item 2: parked; reopen trigger: an outside organizer gets stuck in onboarding.
