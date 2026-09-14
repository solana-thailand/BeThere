# 112 — Nothing on the page helped someone who could not remember the event

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, asked directly whether the page should point at the ticket
**Severity:** medium — "I cannot remember this one" is the main way a
retrospective survey gets abandoned

## The question

> ถ้าคนที่เข้าไปทำ feedback แล้วจำไม่ได้ อยากจะเข้าไปดูหน้า ticket ของตัวเอง
> ต้องบอกเค้าไหม

Yes — and checked before answering, because linking to a page that does not
actually aid recall is worse than linking to nothing.

## The ticket page is the right destination, verified

`pages/ticket/` carries `event_context.rs` (image, tagline, venue, sessions
link), `timeline.rs`, `access_logistics.rs`, and — the one that matters —
`video_section.rs`, the recording.

**All twelve open events have a `video_url`.** Not most: all of them. So the
link works for every session anyone can be asked about, which is what makes it
worth putting on the page rather than in a footnote.

DevRel reached this conclusion first and solved it the only way they could from
outside the product, by pasting recordings into the covering mail:

> ทบทวนความจำก่อนตอบ — ทุกครั้งมีวิดีโอเต็มให้ดูย้อนหลัง
>
> people do not remember an event by its title four months later, they remember
> the room and the slide

The platform already holds all of that per person. It simply never offered it at
the moment the question is asked.

## Why this is only possible now

`.issues/108` **removed** a link from the block header, deliberately:

> Nothing on this form is persisted until submit, so a navigation away from a
> half-filled block throws the answers away — the header was one tap from
> losing someone's work.

That was right then. `.issues/111` shipped drafts the following day, and the
constraint went with it: answers now survive leaving the page. The same feature
that made the form safe to abandon is what makes it safe to offer a detour.

Two days apart, and the second issue only reads as an improvement because the
first was recorded with its reason.

## Where it goes, and why not elsewhere

**Inside the open block, above the questions.** That is where the reader is
standing when they get stuck — not in the collapsed list, where it would be
eleven more lines of chrome on a page whose whole shape is about not looking
like work.

**A new tab.** Drafts save the answers but not which block is open, so a
same-tab detour would still cost the reader their place and their scroll
position. `rel="noopener"` with it.

**Worded as the problem, not the destination.** *"จำงานนี้ไม่ได้? เปิดตั๋วของคุณ
เพื่อดูวิดีโอย้อนหลังและรายละเอียดงาน"* — someone who is not stuck skips it
without reading; someone who is recognises their own situation in the first two
words. "ดูตั๋ว" would have made them work out whether it was for them.

`my_feedback_events` returns `attendee_id` for this. The view already joined it.

## Related

- `.issues/108` — removed the link that this restores, for a reason that has
  since been fixed.
- `.issues/111` — the drafts that removed it.
- `.issues/094` — the posters, the other half of the recall problem.
