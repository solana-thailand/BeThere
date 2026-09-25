# 100 — `/discover` rendered with no way back and no way in

**Status:** fixed 2026-09-14, deployed (in prod tag `deploy/production/20260925T032335Z`; `issue_ledger.py` 2026-09-25 found every linked commit there)
**Found:** 2026-09-14, looking at the deployed page rather than its status code
**Severity:** medium — it blocks the page becoming the post-login default,
which is the reason it was built

## What

Opened on production, `/discover` renders exactly its three sections and
nothing else: no wordmark, no sign-in, no menu, no link home. The browser back
button is the only navigation.

That is survivable for a page you reach deliberately. It is disqualifying for
the thing `.issues/096` proposes making it — the first screen after login.

**There is no shared site header to have forgotten.** The landing page builds
its own inline in `pages/landing/page.rs`; nothing else in the tree exposes one.
So every page that is not the landing page either builds chrome or has none, and
this one had none.

## The second half: a signed-out visitor could not tell what they were missing

After `.issues/099`, a logged-out visitor correctly sees the public list and no
personal sections. Correct, and confusing: the subtitle promises
*"งานที่คุณลงทะเบียนไว้"* and then nothing appears, with no explanation and no
sign-in affordance anywhere on the page.

## The fix, deliberately small

- A `dv-nav` row: the BeThere wordmark linking `/`, and — only when the visitor
  is signed out — a `เข้าสู่ระบบ` button carrying `next=/discover`.
- A one-line hint under the header, also signed-out only, saying what signing in
  adds.

`signed_in` is set from the `/my-registrations` response actually returning 200,
not from a token in local storage — the cookie is the session, and a stale token
would promise sections that never arrive.

## Not done

**A shared header component.** Extracting the landing page's inline chrome is
the right fix and is not this. It touches the most-viewed page in the app, and
doing it at the end of a long session to save a wordmark would be a bad trade.
Recorded as the real blocker for `/discover` becoming the default.

## Related

- `.issues/096` — the page and the default-after-login question.
- `.issues/099` — the redirect that hid this for a day.
