# 009 — Taps, typing and screens for four flows, phone viewport, local fixture
Status: valid
Rung: taps to finish, fields shown, page height and the primary button's distance from the top, in screens of a 390×844 phone viewport (.plans/049 item 1)
Session: event-checkin-e8, 1792904528
Commit: b447e7d9 (develop; local fixture worker built from this tree)
Gate: frontend `cargo test --locked` exit 0 on this tree (pull 299 CI green)
Lanes: single
Load: local fixture worker (wrangler dev --local, e2e seed + plan 047/049 probe seeds), one headless Chrome, `isMobile`, deviceScaleFactor 2; signed in as the fixture super admin with a name and Telegram contact on file

## Method

Headless Chrome (puppeteer-core) opens each page, waits for render, and reads
`scrollHeight / 844` (screens), the top of the primary button / 844, and the
visible inputs, selects, textareas and buttons inside `main`. Taps are the
clicks the script needs to finish the flow; typing is counted separately.
Probes: `scratch/p047/probe_ux_measure.mjs` and `probe_ux_measure2.mjs`
(not committed).

## Result

| Flow | Taps | Typed | Page (screens) | Primary at (screens) | Fields shown |
|---|---|---|---|---|---|
| F1 free event, one click (eligible) | 1 (+1 hero jump) | 0 | 3.8 | 1.8 ("Agree and register"); hero "Reserve Your Spot" at 0.89 | 0 |
| F2 deposit event, form (prefilled) | 2: consent, Reserve (+1 hero jump) | 0 | 4.7 | 2.8 ("Reserve My Spot") | 7 |
| F2 next: ticket with the deposit due | — | — | 2.0 | — | 0 |
| F3 door check-in, typed code | 3: Enter manually, Look Up, Confirm Check-In | 6 chars | 1.0 | 0.9 | 1 |
| F3 door check-in, camera | 1: Confirm Check-In after the scan | 0 | 1.0 | — | 0 |
| F4 organizer creates an event | 1 to open + fill + 1 to save | many | **9.3** | **9.2** ("Create Event" at the very bottom) | **38** |

All flows completed with 0 page errors. F1 and F2 landed on `/ticket/…`.

## Reading

- Attendee flows are short: 1–3 taps, nothing to type when the profile is on file.
  The cost is scrolling: the registration card sits 1.8–2.8 screens down, and the
  hero button that jumps to it is at 0.89, on the fold's edge.
- The door is 1 tap per person with the camera (a confirm after each scan).
- The organizer create form is the outlier: 38 fields over 9.3 screens, with the
  only save button at the end.
