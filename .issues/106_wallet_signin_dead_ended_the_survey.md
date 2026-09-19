# 106 — Signing in with a wallet dead-ended the survey

**Status:** fixed 2026-09-14, **verified 2026-09-19**, still **not deployed**
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

## Verified (2026-09-19)

Both halves, in a browser against a local `workerd` (the offline harness in
`wrangler dev --local`), because a Leptos page that compiles and lints can still
render nothing.

The 403 branch never needed a *wallet* in particular — it is gated on
`claims.email_verified`, so an HS256 token signed with the local `JWT_SECRET`
carrying `email_verified: false` reaches exactly the same handler branch. That
is what a wallet session is, from this endpoint's point of view.

- `GET /api/my-feedback-events` with that session → **403**
  `Sign in with Google again to give feedback.`
- `/feedback` with the same session in the cookie renders the `NeedsGoogle`
  state, in Thai — *"ต้องเข้าสู่ระบบด้วย Google"*, the reason
  (*"แบบสอบถามผูกกับอีเมลที่คุณใช้ลงทะเบียนงาน…"*) — and a button whose href is
  `/login?next=/feedback`. No "ส่งไม่สำเร็จ", no English, no dead end.
- `/login?next=/feedback` shows the Thai subtitle and the line under the wallet
  button. `/login` with no `next` shows neither — the A/B rules out "the copy is
  always on", which a one-sided check would not have.

What is still unverified is the *wallet sign-in itself* (the button's own flow),
which needs a real wallet extension. Nothing on this issue's path depends on it.

## Related

- `.issues/098` — why the survey needs a verified email at all.
- `.issues/091` — the campaign this is the front door of.
