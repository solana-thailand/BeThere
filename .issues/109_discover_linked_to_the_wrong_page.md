# 109 — `/discover` sent ticket-holders to a page saying the event had ended

**Status:** fixed 2026-09-14; deployed to prod 2026-09-14 at `bc062fc` (`worker/scripts/.preflight-bypass.log` entry 19); still present in current prod `8a3d6d9d`.
**Found:** 2026-09-14, owner review
**Severity:** medium — it is the defect that made `/discover` unfit to be the
post-login home

## What

> กิจกรรมที่ผ่านมาแล้ว พอกดเข้าไปเป็นหน้า slug ที่บอกว่า event จบแล้ว แต่ว่าจริงๆ
> คนที่สมัครผ่านแล้วควรจะเห็นหน้าสุดท้ายที่เห็นได้ เช่น ticket หรือ claim

Every `/discover` row linked to `/e/{slug}` — the **public** event page. For a
completed event that page correctly says the event is over, which is the wrong
thing to tell someone who holds a ticket, a claimable badge, or an outstanding
deposit for it.

**The landing page has always got this right.** Its own registrations list uses
`next_step.url` from the same endpoint `/discover` was already calling, and
`/discover` simply ignored the field. One payload, two readers, one of them
discarding the useful half.

## The fix

Rows in "งานของฉัน" and "งานที่ผ่านมา" link to `next_step.url` — ticket, claim,
deposit, whichever the API says applies — falling back to `/e/{slug}` only when
there is no next step to offer.

The `<For>` key had to change with it: it was the `href`, which is no longer
unique now that two registrations can resolve to the same destination.

"เร็ว ๆ นี้" still links to `/e/{slug}`. Those are public events the reader may
not have registered for, and the public page is the right destination.

## Still open on this page

`/discover` shows the same upcoming events and the same registration list as the
signed-in landing page. Two surfaces, one set of facts. Nothing links to
`/discover` yet — deliberately, until that is resolved (`.issues/107`).

## Related

- `.issues/107` — where the owner listed this, and where the redirect was pulled.
- `.issues/096` — the page.
