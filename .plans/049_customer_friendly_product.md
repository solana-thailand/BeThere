# 049 — What makes customers like the product: five asks, sized for BeThere

**Status:** in progress. Item 4 done (pull 298, 299; on staging). Item 1 measured (`.benchmarks/009`); its fixes below need owner picks. Owner asked 2026-10-27 to
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

## Item 1 — measured 2026-10-27 (`.benchmarks/009`)

| Flow | Taps | Screens | Finding |
|---|---|---|---|
| Free event, one click | 1 (+1 hero jump) | page 3.8, button at 1.8 | short, but the card is below the fold |
| Deposit event, prefilled form | 2 (+1) | page 4.7, button at 2.8 | 7 fields shown even when all are filled |
| Door, camera | 1 per person | 1.0 | fine |
| Organizer creates an event | open + fill + save | **9.3**, save at **9.2** | **38 fields**, one save button at the end |

Proposed fixes, by value (owner picks):

- **A. Organizer quick create.** A short first step with the few fields an event
  cannot exist without (name, date and time, place, format; deposit on/off), then
  "create", then the rest in the existing edit form. Or, smaller: a sticky save bar.
  This is the biggest gap the tips point at.
- **B. Register card above the fold.** When one click applies, show the card in
  the hero (where "Reserve Your Spot" is, at 0.89 screens) so it is 1 tap, 0 scroll.
- **C. Collapse a prefilled form** to a summary line plus "edit", like the one-click
  card, for events that cannot use one click (deposit events).
- Door: no change proposed. Auto-confirm after a camera scan would save a tap per
  person but removes the chance to catch a wrong ticket; owner call if wanted.

## Fix A built (2026-10-27, owner pick A, branch `feature/049-quick-create`)

On create, the Optional and Recommended sections of the event form start
collapsed (`open=!is_create`; editing still opens everything); the three
Required ones start open. Deposit Details stays open on purpose: a new event has
the deposit on, and then the amount is required (the form refuses "Create"
without one), so hiding it would hide the field the error is about. The save row
is `position: sticky` and `.admin-content` uses `overflow-x: clip` (it used
`hidden`, which made `<main>` the sticky container, so the row never stuck).

Measured on a local fixture, same probe, 390×844:

| | before | after |
|---|---|---|
| page | 9.3 screens | **6.3** |
| fields shown | 38 | **25** |
| save button | at 9.2 screens | **stuck to the bottom of the screen** |

Not done, owner call: collapse Deposit Details too (with a summary line) only if
the amount stops being required on create, or the default becomes "deposit off".

## Landing page: less is more (2026-10-27, prototype on `feature/049-landing-minimal`, not merged)

Owner: too many things pull the eye, unclear what is the focus; asked whether
less is better for the business image. Measured on prod and a local fixture
(390 and 1440 wide):

| | current | minimal prototype |
|---|---|---|
| mobile page | 4.9 screens | 2.9 |
| desktop page | 2.9 | 1.7 |
| animated elements | 3 | 0 |
| hero parts | 9 (kicker, 2-colour headline, sub-line with 6 underlined keywords, Today/Coming box, orange quote, 2 buttons, the 90/94 card, cookie bar) | 4 (headline, one sentence, 2 buttons, the 90/94 card) |

Cut: the kicker, the Today/Coming box, the quote, the keyword underlines, both
"SO FAR" tapes, the second payers line (73/77) and the devnet door on the home
page (the other pages keep it); the globe moved to `/events`.

**Retraction.** An earlier version of this note called the "0 events · ฿0" strip
the most serious problem. It was a screenshot taken during the count-up
animation: after scrolling it reads 14 events, 141 on-site, 111 through the door,
385 online, ฿48,000. 90/94 (RTM #1–#6 with hand records) and 73/77 (recorded by
BeThere only) are different counts and each is labelled. No numbers bug; the case
for the cut is clutter, not a defect.

**Checked, not a bug:** the group-photo strip showed empty frames in a screenshot.
All 8 photo URLs answer 200 `image/jpeg`; they are `loading="lazy"` and 8 of 8
load once the strip is scrolled through.

## Fix B (register card in the hero): not done, on purpose

Measured: the hero's "Reserve Your Spot" is at 0.89 screens and already jumps to
the card (`#reserve`), so the one-click path is 2 taps and 0 manual scrolling.
Moving the card into the hero would save one tap on free events only, at the
cost of restructuring an 889-line page and its visual baselines. Revisit if a
click-through measurement shows people stop at the hero button.

## Minor finding: `HEAD /media/*` is 404

The landing film works for viewers (GET, range, 206, 3.8 MB `video/mp4`, EN and
TH, captions and posters). `media::serve` is gated on `Method::GET` in
`worker/src/lib.rs`, so `HEAD` falls through to the 404 page; a link checker or
crawler that probes with HEAD would report the film as broken. Left alone: the
path took work to get right (iOS Safari ranges, edge cache) and nothing real
breaks. Fix if a checker ever complains: answer HEAD with the same headers and no
body.

## Item 3: clips (draft for the owner, nothing rendered)

The existing "Why a deposit · 1 min" film is a pitch about the problem (no-shows
and food waste). These two are how-tos, so they do not duplicate it. 60–90 s
each, EN and TH captions like the existing film, screens from a seeded fixture
(no real attendee data), built with the fframes/ffilms pipeline.

**Attendee, "Show up, get it back" (~75 s)**
1. Open an event from `/events`; the Reserve button (5 s).
2. Sign in once; the card says "You'll register as …"; one tap on "Agree and
   register" (10 s). Say that a name on file is why it is one tap.
3. The ticket: pay the deposit by PromptPay, upload the slip (15 s).
4. At the door: show the QR, the staff phone scans it (10 s).
5. After the event: the refund card, "Returned on <date>" and the receipt
   (15 s). Say the refund window in the words the site already uses.
6. End card: the badge, and where to find the FAQ (10 s).

**Organizer, "Create an event in 3 minutes" (~85 s)**
1. `/admin` → Create Event: only Basic Info, Schedule and Google Sheets are open
   (15 s). Name, date, place; the deposit amount, which is required.
2. Save; the public link and the share button (10 s).
3. Door day: the scanner on a phone, a scan, a walk-in (20 s).
4. After: Deposits & Refunds, refund with the slip; credit holders and the
   payout (20 s).
5. End card: FAQ and organizer page (10 s).

Owner to decide: tone (plain or the Night Edition look), voice-over or captions
only, whether the organizer clip may show the Google Sheet step (it needs a
service account), and the order. Rendering waits for that.

## Order

1. Item 4 (this branch).
2. Item 1: measurements first, recorded under `.benchmarks/`.
3. Item 5: design doc, owner decision, then build.
4. Item 3: clips.
5. Item 2: parked; reopen trigger: an outside organizer gets stuck in onboarding.
