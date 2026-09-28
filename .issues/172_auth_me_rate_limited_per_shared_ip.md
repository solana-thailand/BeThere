# 172: The session check was rate-limited per IP, so a shared Wi-Fi could bounce signed-in attendees to login

**Status:** fixed on develop (2026-09-29, session `event-checkin-ba`). Not deployed. Prod impact is reasoned from the code, not observed: prod logs were not checked.

## What happened

`middleware::rate_limit` put every `/api/auth/*` path in the auth group:
20 requests per 60 s, keyed by `cf-connecting-ip` (the native
`AUTH_RATE_LIMITER` binding, or the isolate fallback). That group included
`GET /api/auth/me`. The SPA calls it on mount from:
- `ProtectedRoute`;
- the public event page;
- discover;
- the developer profile.

A 429 reads as "signed out": `ProtectedRoute` redirects to `/login`.

It was found by the P0-3 e2e suite. One local IP loading ~20 pages a minute
got `/api/auth/me` 429s, and `/admin` rendered the sign-in page. It failed
about every second run (`/tmp/ec-ba/wr-e2e.log`: dozens of `429 Too Many
Requests` on `/api/auth/me`).

## Why it matters in prod

Many people share one public IP:
- attendees on venue Wi-Fi (one NAT);
- phones on Thai mobile carriers (CGNAT).

At 20 session checks per minute per address, a check-in crowd opening their
tickets at the door can exhaust the budget within seconds. Everyone after
that is sent to the login page at the one moment the page has to work.

## Fix

`GET /api/auth/me` is exempt from the limiter. It only verifies the signed
session cookie, so there is nothing to guess; an invalid cookie is a cheap
401. Everything else under `/api/auth/` stays limited: sign-in URLs, the
OAuth callback, the wallet nonce and verify, and social linking.

Test: `rate_limit::tests::session_check_is_not_limited_but_sign_in_is` (in
the file, next to the existing tests of this private function).

**After the fix:** 3 full e2e runs of 42/42 with 0 429s.

## Follow-ups (not done)

- Check prod: `/api/auth/me` 429 volume during a past event (Workers
  analytics, or `wrangler tail`, owner-run).
- Other per-IP limits have the same shared-IP shape and may be too low for a
  venue: claim 120/min and deposit 60/min. The memory
  `free-plan-cpu-cap-is-binding` already notes the deposit limiter as "the
  wall".
