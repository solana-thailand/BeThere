# 106 — Signing in with a wallet dead-ended the survey

**Status:** fixed 2026-09-14, **not deployed**
**Raised by:** DevRel, reviewing the sign-in flow the survey link leads into
**Severity:** medium — it sits directly on the path 206 people are about to walk

## What

The survey link goes to `/feedback`. Signed out, that bounces to
`/login?next=/feedback`, which offers **Google and Solana Wallet side by side,
equally prominent**. Only one of them can answer the survey: the questions are
recorded against the email the person registered the event with, and
`my_feedback_events` requires `claims.email_verified`.

Traced rather than assumed. A wallet session gets 403, and `api_get_json`
returns that as an `Err`, which the page rendered as:

```
ส่งไม่สำเร็จ
Sign in with Google again to give feedback.
```

Wrong three ways: **nothing was submitted**, the instruction is in English on an
otherwise Thai page, and **there is no button** — the person has to work out for
themselves that they must sign out and start again.

## The fix, on both sides of the door

**After.** A `NeedsGoogle` state of its own, in Thai, with a button straight to
`/login?next=/feedback`. It says why — the survey is tied to the email the event
was registered with — rather than just what to do.

**Before, which is the better place.** `/login` now knows when `next` is the
survey and says so before the choice is made:

- the subtitle becomes *"ใช้ Google ด้วยอีเมลที่คุณลงทะเบียนงานไว้ —
  แบบสอบถามผูกกับอีเมลนั้น"*
- a line under the wallet button: *"กระเป๋าเงินใช้ตอบแบบสอบถามไม่ได้
  เพราะแบบสอบถามผูกกับอีเมลที่ลงทะเบียนงานไว้"*

Naming the consequence next to the button costs a sentence. The dead end it
prevents costs a respondent.

## Why not simply hide the wallet button

It was the obvious move and it is wrong. A wallet session is a real session for
the rest of the platform — tickets, deposits, claims — and someone arriving at
`/login` with `?next=/feedback` may be there to do something else afterwards.
Removing a working sign-in method to protect one flow trades a permanent
capability for a temporary confusion. Labelling is enough.

DevRel is also covering this from their end with a line in the survey mail, so
the same fact now appears at the two points where it matters: before the link is
clicked, and beside the button.

## Not verified

Both halves need a session to exercise — the 403 branch needs a *wallet* session
specifically, which no account here has. Compiled, linted, formatted.

## Related

- `.issues/098` — why the survey needs a verified email at all.
- `.issues/091` — the campaign this is the front door of.
