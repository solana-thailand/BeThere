# 096 — A returning attendee lands on a pitch they have already read

**Status:** built 2026-09-13, deployed (in prod tag `deploy/production/20260925T032335Z`; `issue_ledger.py` 2026-09-25 found every linked commit there); not yet the default after login
**Found:** 2026-09-13, UX review continued from `.issues/094`
**Severity:** medium — it is the first screen every one of the 55 people in the
feedback campaign will see

## What

*"คนเข้ามาปุ้บต้องเห็นเลยว่าให้ทำอะไรบ้าง เพราะตอนนี้โดนบังด้วย hero section"*

Measured on the live landing page at 430px, in order down the page: hero, a
role toggle, three stat tiles, two CTAs, **one** event card, "How it works" with
a *second* role toggle, four FAQ entries, a host pitch, a waitlist form, footer.

That is the right page for someone who has never heard of BeThere. For everyone
else the three things they came for — when is the next event, where is my
ticket, what did I go to — are below the fold behind an explanation they have
already read. The third is not on the page at all.

## The reference, and what is actually good about it

`seatforme.com/discover`, offered as the model. Read at 430px:

- **No hero.** A title, one line of subtitle, then rows.
- **Two sections, plainly named** — `เร็วๆ นี้` and `งานที่ผ่านมา`.
- **A date chip on the left of every row**, big day over short month, **orange
  for upcoming and grey for past**.
- Row is: chip · title + time + venue · thumbnail · chevron. One tap.
- Nothing else. No explanation, no FAQ, no pitch.

The load-bearing detail is the chip's **colour**, not its content: it sorts the
page for the reader before they parse a single digit. That is what makes one
list of mixed-tense events scannable instead of a wall, and it is the piece
worth copying.

## What was built

`/discover` — `frontend-leptos/src/pages/public/discover.rs`, three sections:

| section | source | shown when |
|---|---|---|
| `เร็ว ๆ นี้` | `GET /api/public/events` | always; says so when empty |
| `งานของฉัน` | `GET /api/my-registrations`, `event_end_ms >= now` | only when non-empty |
| `งานที่ผ่านมา` | same, `event_end_ms < now` | only when non-empty |

Decisions worth recording:

- **Split on `event_end_ms`, not `event_start_ms`.** An event running right now
  belongs under the heading that says it is happening. Splitting on the start
  drops it into the past at the moment it begins.
- **Signed out is a normal state**, not an error. `/my-registrations` failing
  just means the two personal sections do not render; the public list still does.
- **An empty section renders nothing at all.** A bare `งานของฉัน` heading tells a
  first-time visitor only that they are missing out.
- **The generic badge SVG is not used as a thumbnail.** `badge-hd.svg` is the
  same image for every event, so eight identical thumbnails down a list is worse
  than eight empty slots. Poster, then a real badge image, then nothing.
- **Sort direction differs per section**: soonest first for what is coming, most
  recent first for what is done. In both the row you care about is at the top.

`GET /api/my-registrations` gained `event_end_ms`, `poster_url`, `nft_image_url`,
`location` and `time_tba` — the event row was already joined, the columns simply
were not selected.

## Not done

- **`/discover` is not yet the default after login.** Making it so is a one-line
  redirect change, but it decides what every existing user sees on their next
  visit, so it is a separate call from building the page.
- The four landing-page observations in `.issues/094` (two role switchers, the
  orphaned stat tile, two competing CTAs) are untouched. If `/discover` becomes
  the post-login default, the landing page stops having to serve two audiences
  and most of them stop mattering.
- No list/grid toggle. The reference has one; with at most a handful of rows it
  would be a control with nothing to control.
- **Not seen rendered.** `/my-registrations` needs a session, so the two personal
  sections have been compiled and linted but not looked at.

## Related

- `.issues/094` — the poster plumbing this page reuses, and the landing-page
  observations it would make moot.
- `.issues/091` — the campaign whose 55 people arrive here.
