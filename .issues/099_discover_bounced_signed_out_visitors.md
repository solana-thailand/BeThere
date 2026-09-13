# 099 — `/discover` bounced every signed-out visitor to the login page

**Status:** fixed 2026-09-14, **not deployed**
**Found:** 2026-09-14, opening the deployed page instead of trusting the compile
**Severity:** high — the page exists to be the first thing people see, and it
was unreachable for anyone not already signed in

## What

`/discover` returned HTTP 200, and then the app replaced it:

```
location → /login?next=/discover
.dv-page present → false
```

`.issues/096` states the intended behaviour explicitly:

> **Signed out is a normal state**, not an error. `/my-registrations` failing
> just means the two personal sections do not render; the public list still does.

The page was written to that rule. The API layer was not. Every helper in
`api/mod.rs` — six call sites — ends a 401 with
`redirect_to_login_expired()`, which navigates away. So `api_get_json` never
returns the `Err` the page was waiting to ignore; the visitor is gone before the
match arm runs.

## Why it was not caught

Three checks passed and none of them could see it: `cargo check`, `cargo clippy
-D warnings`, and a `curl` for the status code. The route **does** serve 200 —
the redirect happens in WASM, after the document loads. The only check that
could catch it is opening the page, which is what found it.

It also cannot fail in CI as written: Playwright would have to visit
`/discover` signed out and assert the URL did not change.

## The fix

The codebase already had the answer. The landing page's own registrations list
uses the **low-level** `crate::api::fetch::get` and tests `status() == 200`,
precisely because it renders for signed-out visitors too. `/discover` now does
the same. That path sends the session cookie (default `same-origin` credentials)
and does not redirect, which is why the landing list works signed in — visible
in the organizer's screenshot.

The comment at the call site now records that the API layer's contract is the
opposite of what the page wants, rather than asserting the page's own rule as if
it were shared.

## The lesson, which is the same one as this morning

`.issues/095` ended with: *when a rule appears once, look for the second copy
before fixing the first.* This is its mirror — **when a page states a rule the
platform has to honour, check that the platform honours it.** I wrote "signed
out is a normal state" as a comment and never verified the six functions that
disagree.

## Related

- `.issues/096` — the page, and the rule it asserted.
