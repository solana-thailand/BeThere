# Plan 040: event page attendance, follow-ups after the first row

**Created:** 2026-10-03, session `event-checkin-0b`.
**Trigger:** the owner asked that `/e/{slug}` say how many people have
registered, not only how many seats are left. The first row shipped on
`develop` (the "Attendance" row, `frontend-leptos/src/pages/public_event/attendance.rs`).
**Gate:** the demo take on 8 Oct (`.issues/164`); from 6 Oct do not touch
demo-facing flows (`.plans/039`). Both open items below wait for it.

## Done

- [x] One "Attendance" row replaces the capacity row: count per track, a fill
  bar and seats left on a capped track, "Full" at the cap; an uncapped track
  shows its count from 10 people; after the event only the counts remain.
  Verified in a local browser, EN and TH, capped / full / hybrid at 390 px.
- [x] Counting checked: `attendee_counts_by_participation.sql` counts every
  attendee row of the event with no status filter, and the same tally gates
  the cap. So "3 / 40" always equals "37 left"; a rejected attendee still
  holds a seat. Changing that is a seat-policy decision, not a display one.

## Open

- [ ] **Checked-in count while the event is live and after it.** "Registered"
  is wrong once people are in the room; the page should lead with who came.
  Reopen when: the take on 8 Oct is done. Design notes from reading the code:
  - Add `SUM(checked_in_at IS NOT NULL AND checked_in_at <> '')` to the
    existing `GROUP BY` query, not a second query: the public event endpoint
    is on the free-plan CPU cap (`free-plan-cpu-cap-is-binding`), and the
    shared query already runs per page view.
  - That query also feeds the capacity gates, so the new column must not
    change `TrackCounts`; return it beside it.
  - D1 only: an event whose attendees live only in its sheet has no check-in
    count, so the field must be `null` there and the UI must fall back to
    registered, never show 0.
- [ ] **Move a one-line count next to the "reserve" button.** The row sits
  below the poster, title, button and date card, so on a phone it is below the
  fold. Reopen when: the take is done (it changes the hero layout that was
  just QA'd in `.issues/182`).
- [ ] **Uncapped threshold (10) is my call, not the owner's.** Change
  `MIN_PUBLIC_UNCAPPED_COUNT` if the owner wants small numbers shown.
