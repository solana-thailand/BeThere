# 143: OAuth `state` is an open redirect and is not bound to the browser (login CSRF)

**Status:**
- **Part A: deployed 2026-09-24 (prod version `36eae0db`, git `897aa07`).**
- **Part B: open.**

**Found by:** the ISO 27001 gap assessment (`docs/iso27001_gap_assessment.md`).
**Severity:** medium. The JWT is set only as an HttpOnly cookie and never
appears in the redirect URL, so this is phishing and session confusion, not
token theft.

## Part A: open redirects (fixed)

1. **Worker.** `GET /api/auth/url?redirect=<anything>` put the value into the
   Google OAuth `state`. `handlers/auth.rs::auth_callback` then did
   `Redirect::to(state)` verbatim. A crafted, genuine Google sign-in link
   therefore landed the victim on any site after a real login. A `state` with
   CR/LF would also have made `Redirect::to` build an invalid header value.
2. **Frontend.** On wallet sign-in, `pages/login.rs` ran
   `location.href = ?next=` without validation. `next=javascript:…` would run
   script in this origin, which can read the localStorage token. The
   already-signed-in redirect and the Google `state` came from the same
   parameter.

**Fix:** `event_checkin_domain::validation::safe_redirect_path` accepts only a
same-origin path. It rejects:
- absolute URLs and other schemes;
- `//host`;
- any backslash;
- control characters.

The worker applies it at the redirect sink. The login page filters `next`
once, which covers all three sinks. Tests are in
`domain/tests/safe_redirect_path.rs`.

In the same pass (verified, fixed):
- the SIWS nonce was `now ^ const`, which made the whole challenge message
  predictable; it is now 128 bits from `crypto.getRandomValues` (`crypto::random_hex`);
- the webhook bearer compares (`handlers/escrow_index.rs`,
  `deposit/usdc/handlers/webhook.rs`) used `==`; they now use the shared
  `crypto::constant_time_eq`, which JWT verification also uses.

## Part B: login CSRF (open)

`state` carries a redirect path, not a nonce bound to the browser, and the
callback accepts any `code`. An attacker can start a Google login for their
own account, stop before the callback, and get the victim's browser to load
`/api/auth/callback?code=<attacker's code>`. The victim is then signed in as
the attacker, and whatever they do next (registrations, a slip upload with
bank details) lands in the attacker's account.

**Fix sketch:**
1. `/api/auth/url` mints a random nonce and sets it in a short-lived
   `HttpOnly; SameSite=Lax; Path=/api/auth` cookie.
2. It sends `state = <nonce>.<path>`, HMAC-signed the way
   `handlers/email_link` already signs its state.
3. The callback requires the cookie to match `state` and clears the cookie.

Deploy caveat: logins started before the deploy fail once. Keep a short grace
period, or ship it after RTM#6 (2026-09-27). Do not change the auth flow in the
days before the event.
