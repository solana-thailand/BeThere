# Plan 040: event page attendance, follow-ups after the first row

**Created:** 2026-10-03, session `event-checkin-0b`.
**Trigger:** the owner asked that `/e/{slug}` say how many people have
registered, not only how many seats are left. The first row shipped on
`develop` (the "Attendance" row, `frontend-leptos/src/pages/public_event/attendance.rs`).
**Gate:** the owner lifted it on 2026-10-03 for the two follow-ups below,
which shipped before the 6 Oct freeze.

## Done

- [x] One "Attendance" row replaces the capacity row: count per track, a fill
  bar and seats left on a capped track, "Full" at the cap; an uncapped track
  shows its count from 10 people; after the event only the counts remain.
  Verified in a local browser, EN and TH, capped / full / hybrid at 390 px.
- [x] Counting checked: `attendee_counts_by_participation.sql` counts every
  attendee row of the event with no status filter, and the same tally gates
  the cap. So "3 / 40" always equals "37 left"; a rejected attendee still
  holds a seat. Changing that is a seat-policy decision, not a display one.

- [x] **Checked-in count** (2026-10-03, `event-checkin-0b`, owner OK'd it
  before the take). `GET /api/public/event/{slug}` now sends
  `checked_in_count`: `null` before the event starts or when the D1 read
  fails, a count after. It is a separate query (`attendee_checked_in_count.sql`),
  run only once the event has started, so the capacity tally that gates
  registration is untouched and the pre-event page pays nothing. The row leads
  with it when it is above zero; zero or unknown falls back to registered, so a
  sheet-only event never reads "0 checked in". The field is inserted into the
  response map, not into `json!`, which is at the macro recursion limit.
- [x] **One-line caption under the reserve button** (same day). Same model as
  the row, so the two cannot disagree. Phones only (<=640px, the sticky-CTA
  breakpoint): beside the button on wide screens the row is already visible.
  Verified at 390x844 in a local browser, EN and TH; CI e2e 51/51 with the new
  Linux baselines (`b16a4470`).

## Open (updated)

- [x] **Uncapped threshold (10) is my call, not the owner's.** Change
  `MIN_PUBLIC_UNCAPPED_COUNT` if the owner wants small numbers shown.
  **Blocked (2026-10-06, `event-checkin-42`):** owner preference, and any change moves the `/e/{slug}` attendance row (event flow, 6–8 Oct freeze); 10 stays until the owner says otherwise.
  **Still waiting (2026-10-08, `event-checkin-19`):** the owner's preference; the freeze half has lapsed.
  **Checked (2026-10-08, `event-checkin-fe`):** `MIN_PUBLIC_UNCAPPED_COUNT` is 10 on every branch. **Owner must:** say "keep 10" (then this closes) or name the new number.
  **Closed (2026-10-08, `event-checkin-8a`, owner go "do what was recommended"):** keep 10. The reason in `attendance.rs` still holds: an uncapped "3 registered" reads as an empty room, and a capped track always shows its count against the cap. No code change; reopen if an organizer asks for smaller counts.

## Deploys

- Staging 2026-10-05: `58f4f9cc` (git `c4a33b25`). Opened on a disposable event:
  attendance row "1 / 20", caption under the button, a checked-in attendee
  lands on the ticket with the claim card and the Resources card.
- Prod 2026-10-05 (owner go): release merge `da6da377`, version `69b5cf19`,
  D1 migration 0057 applied first, backup taken. Prod write smoke did not run
  (needs `SMOKE_TOKEN`; `dev-token` gets 401); reads and asset types pass.
