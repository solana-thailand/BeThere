# 107 — The form forgot what you had already answered

**Status:** fixed 2026-09-14, **not deployed**
**Found:** 2026-09-14, by the organizer actually filling the form in
**Severity:** high — it is the page 206 people are about to be emailed a link to

## What happened, in the owner's words

> ลองบันทึกไป event นึง ระบบก็บอกว่าบันทึกแล้ว 1 event แล้วก็ไม่มีปุ่มไหนให้กดต่อ
> → พอพิมพ์ url กลับมาหน้า feedback เอง ก็ยังขึ้นว่า 0 จาก 11 อยู่ ทั้งๆที่ต้องเป็น
> 1 จาก 11 แล้ว และ event ที่ให้ feedback ไปแล้ว ก็ยังขึ้นมาให้สามารถกดซ้ำได้อีก

**The data was fine.** Production held four `post.*` rows for RTM #4 before any
of this was touched. The page simply had no way to know.

`feedback_eligible_events` answers *"may this person rate this session"*. The
page needed *"does this person still need to rate it"*, and nothing in the
payload distinguished them. So a returning reader saw `0 จาก 11`, was offered the
session they had already answered, and a second submission would have written a
second set of answers over the first.

## Four defects, one visit

| | |
|---|---|
| progress reset to `0 จาก 11` | no `answered` in the payload |
| answered session offered again | same |
| "บันทึกแล้ว 1 งาน" with **no button** | terminal state had no next step |
| **no way home from `/feedback` at all** | the page had no header |

## Fixes

**Migration 0040** adds `answered` to the view — an `EXISTS` over
`registration_responses` for `post.%` on that (person, event). Joined on
`lower(email)`, and there is a test for that: the submission path writes
whatever case the caller signed in with, and a case-sensitive join reports
everything unanswered. A/B confirms it — dropping `lower()` fails the test
`0 != 1`.

**Marked, not filtered.** An answered session stays in the list with a
`ตอบแล้ว` badge. A session that vanishes the moment you answer it reads as the
answer having been lost, which is the confusion this exists to remove. It also
leaves the door open to changing your mind.

Sorted `answered ASC, event_start_ms DESC`: what remains rises, what is done
sinks without disappearing. The block that opens by default is the newest
**unanswered** one rather than simply the first.

**`ตอบแล้ว` is reserved for stored answers.** A tick still marks something typed
in this session but not yet submitted — telling someone their answer was saved
when it was not is worse than saying nothing.

**Terminal states got exits.** "ขอบคุณครับ" now offers *ให้ความเห็นงานอื่นต่อ*
and *กลับหน้าหลัก*; "ไม่มีแบบสอบถามค้างอยู่" offers the latter. And the page
carries `SiteHeader`, so there is a way out at any point — it had none, on a
page people reach from an email.

## Two more from the same review

**The wordmark was a `<span>`.** On the landing page that is invisible; now that
the header is shared it was the obvious way home on every other page, and it did
nothing. It is an `<a href="/">`.

**The survey notification named one event.** `How were the sessions?` sat above
a single event name — the one whose enrolment happened to come first, out of
eleven. The survey row stands for all of them (migration 0038), so the name is
now omitted for that kind only.

## Reverted: the post-login redirect to `/discover`

Shipped earlier today, withdrawn on the owner's call, and the call is right.
`/discover` duplicates what the signed-in landing page already shows, its rows
link to the public event page rather than to the reader's ticket or claim, and
its look does not match the rest of the app. **A redirect is a promise that the
destination is better.** It is not yet, so `/` stays the default and `/discover`
remains reachable by URL while it is finished.

## Still open from the same list

- `/discover` rows should link to each registration's next step, not `/e/{slug}`
  — a past event's public page says "this event has ended" to someone who has a
  ticket for it.
- `/discover` and the signed-in landing page show the same things twice.
- No route to `/discover` from anywhere, deliberately, until the above are done.

## Related

- `.issues/103` — the ordering and collapse this corrects.
- `.issues/105` — the redirect this reverts, and the shared header it built.
