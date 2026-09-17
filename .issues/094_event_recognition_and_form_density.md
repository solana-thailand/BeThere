# 094 — You cannot rate an event you do not recognise

**Status:** partially fixed 2026-09-13 — poster plumbing and the feedback form
are done; the landing-page observations below are proposed, not applied
**Found:** 2026-09-13, UX review of `/feedback` and the landing page
**Severity:** medium — it decides whether the 45-response campaign returns 45
answers or 15

## The ask, and why it is the right instinct

*"หน้า feedback ใส่รูป poster ของงานนั้นๆ แล้ว link ไปงานนั้นเลยก็ได้นะ
ทำยังไงก็ได้ให้เขาจำได้ว่าอันนี้คืองานไหนที่ user เคยไป"*

DevRel reached the same conclusion independently, from having run the survey by
email (`solana-thailand-devrel-helper`, `reports/phase-1/survey-emails.md`):

> Recordings in the body, not just the form. Four months is long enough to
> forget which session was which. A person who cannot place the event either
> abandons the form or answers about the wrong one.

Two people arriving at the same requirement from different directions is the
strongest signal this repo gets. The version shipped in `.issues/091` printed
the event **name and nothing else** — for a series where every event is called
"Solana x AI Builders: The Road to Mainnet #N".

## The poster was never reaching the card either

Chasing the poster for `/feedback` turned up the same gap on the most visible
surface the site has.

`GET /api/public/events` — the landing page's Upcoming Events feed — does not
return `poster_url`. The *past*-events payload, forty lines further down the
same file, always has. Two near-identical `json!` builders, one field apart.

So the landing card fell through to `nft_image_url`, which for every event so
far is the generic `/api/badge-hd.svg`. Verified against production:

```json
{"name":"Solana x AI Builders: The Road to Mainnet #6 (Bangkok)",
 "nft_image_url":"https://bethere.solana-thailand.workers.dev/api/badge-hd.svg"}
```

No `poster_url` key at all — while RTM #6 has a real poster sitting in R2. The
one upcoming event on the front page, fourteen days out, was showing a gradient
placeholder.

`event_hero` already documents the correct rule and says why:

> 3-tier fallback: marketing `poster_url` → `nft_image_url` → Ticket icon …
> matching the past-events listing card and **avoiding a cross-surface
> inconsistency**.

The landing card was the surface that inconsistency was hiding on.

## What changed

**Backend.** `poster_url` added to the upcoming-events payload, with a comment
naming the divergence so the next person does not re-create it.

**Landing card.** Poster first, badge second — the order `event_hero` and
`past_events` already use.

**Feedback page.** Each block now opens with poster, date and venue, wrapped in
a link to `/e/{slug}` for anyone who needs more than a picture. Fetched per
block from `GET /api/public/event/{slug}` rather than widened into the inbox
payload, because that payload comes from a SQL view and widening it costs a
migration for an N of 1–3 cached public GETs. The fetches are fired **after**
the form renders: blocking the questions on N image lookups would trade the
thing that makes people answer for the thing that helps them answer accurately.

## Density: four taps per event, not a wall

The first version rendered four `<fieldset>`s of radio buttons per event. Three
events is 36 radios stacked vertically — the shape that makes a two-minute form
look like a ten-minute one.

| | before | after |
|---|---|---|
| per dimension | fieldset + legend + 3 stacked radios | one row: label left, 3-button segmented scale right |
| per event | 4 fieldsets + always-visible textarea | 4 rows + collapsed "＋ เพิ่มข้อเสนอแนะ" |
| taps to finish one event | 4, after scrolling past a textarea each time | **4** |

The comment box starts collapsed. A textarea per event that most people leave
blank is exactly what makes a short form look long, and the answer is still one
tap away for the people who do have something to say.

Tap targets are 44px minimum — most of these answers arrive on a phone. Below
480px the label and scale stack rather than shrink, because three buttons and a
Thai label cannot share a 430px line without the buttons dropping under 44px.

Styles live in `styles/style-21-feedback.css`, following the `style-NN-*`
convention the `_headers` immutable-cache rule keys off.

## Landing page — observed, not changed

Reviewed at 430px on production. One fix is in this change; the rest are
recorded rather than applied, because they are product calls and not defects.

1. **~~Event card shows a placeholder~~** — fixed above.
2. **Two role switchers, one axis.** The hero has a `For Attendees / For
   Organizers` pill toggle; "How it works" has `I am an Attendee / I am an
   Organizer / I am Event Staff` tabs. A reader who picks a side at the top has
   to pick it again 800px later, and the two do not agree on how many roles
   exist. One of them should drive both.
3. **The third stat tile is orphaned.** `100%` and `Instant` share a row, `< 1s`
   sits alone under them. Three tiles in a two-column grid always strands one —
   either a third column at that width, or a full-width third tile, reads as
   deliberate.
4. **Two competing hero CTAs.** `Find Events ↓` and `Create an Event →` carry
   equal weight on a page whose own toggle defaults to Attendees.
5. **Nothing for a returning attendee.** Logged out, the page is entirely a
   pitch. The 30 people being asked for feedback are returning users, and the
   route they need (`/feedback`) is reachable only from the notification bell
   after signing in.

## Not verified

`/feedback` is auth-gated and staging has no public upcoming events, so **the
rendered page has not been seen** — only compiled, linted and tested. The
landing-card fix is verifiable in production and is checked there. Someone with
a session should open `/feedback` before this is treated as done.

## Related

- `.issues/091` — the campaign this serves.
- `.issues/079` — the id/slug drift visible in the same payload: RTM #6's `id`
  is still `...mainnet-5-bangkok-copy`.
