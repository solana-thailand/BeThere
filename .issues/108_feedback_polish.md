# 108 — The continue button did nothing, and the page fetched 15 MB of posters

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, owner's screenshot of the live page plus a report
**Severity:** medium — one hard bug, the rest is the page 206 people will open

## The bug: "ให้ความเห็นงานอื่นต่อ" did nothing

> กดแล้วมันไม่เกิดอะไรขึ้น ต้อง refresh เอา ข้อคำถามถึงจะกลับมาขึ้น

It was `<a href="/feedback">` **on `/feedback`**. The router matches the same
route, nothing remounts, and the loader never runs again. A manual refresh was
the only way through.

It is a button that reloads now. Navigation was never the right tool: the whole
point of continuing is to re-read `answered` from the server, and only a fetch
can do that.

## The posters were the page's real weight

`Event poste` appearing as text in two rows was the give-away — that is an
`alt` attribute rendering while its image loads. The images were fine; they are
just enormous:

| poster | bytes |
|---|---|
| RTM #2 | 2,841,561 |
| RTM #5 | 3,003,945 |
| RTM #3 | 1,598,331 |

Eleven rows of that is well over ten megabytes, fetched at full resolution to be
displayed at **44px**, much of it on phones.

- `loading="lazy"` and `decoding="async"`, so nine off-screen thumbnails stop
  competing with the two on screen.
- `alt=""`. The thumbnail is decorative — the event name is beside it — so the
  alt text was both redundant and the thing that rendered as garbage.
- A background on the slot, so eleven rows do not reflow as each PNG lands.

**Not fixed: they are still full-resolution downloads.** A resized variant needs
an image pipeline (Cloudflare Images or a thumbnail written at upload time), and
that is a piece of infrastructure, not a CSS change. Recorded rather than
hidden — lazy loading makes the page usable, it does not make it small.

## Alignment, affordance, reach

- **Ragged left edge.** Six events have no poster, and the element was simply
  omitted, so their titles started at a different x from everyone else's. The
  slot is now reserved and dashed — "nothing here" rather than a failed image.
- **The closing options read as empty text inputs.** Full-width bordered boxes
  with the label inside is exactly the shape of a text field. A `○`/`●` marker
  makes them pickable at a glance without reintroducing a radio.
- **The submit button sat below all eleven rows.** Anyone who answered the first
  block and wanted to stop had to scroll past everything they had chosen not to
  do. It is sticky now, in reach at the moment they decide to stop.
- **The header was inside the reading-width container**, so its links wrapped
  mid-phrase — "How it works" broke across two lines. Moved outside, on
  `/discover` too, which had the same mistake and had simply not been noticed
  because a signed-out nav has fewer items.

## Related

- `.issues/107` — the answered state this builds on.
- `.issues/103` — the ordering and collapse.
