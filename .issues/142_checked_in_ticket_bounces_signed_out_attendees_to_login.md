# 142 — A checked-in ticket sends signed-out attendees to /login

**Status:** fixed on `develop` 2026-09-23 (session 4), **not deployed**: prod
still has the bug until the next owner-approved deploy. Reproduced on staging
2026-09-23; it was found during `.plans/028` (performance).
**Severity:** high for RTM#6 (2026-09-27). The ticket page is documented as "No
auth required", and every in-person attendee opens it at the door.
**Live in prod:** yes. `e346b24` (the chip) is on `main`.

## Repro (staging, 2026-09-23)

1. Open a fresh browser context (signed out) at
   `/ticket/perf-poll-fixture-1?event_id=e2e-test-event-1790173781`, an in-person attendee with
   `checked_in_at` set (a seeded fixture, deleted afterwards; any checked-in
   in-person attendee reproduces it).
2. Network: `GET /api/public/ticket/…` 200, then
   **`GET /api/deposit/credit-balance` 401**, then a document navigation to
   **`/login?next=/ticket/perf-poll-fixture-1`**.

The attendee sees a sign-in page where their ticket should be. The same thing
happens live: a polling ticket page redirected the moment the attendee was
checked in, because that is when the chip mounts.

## Cause

`frontend-leptos/src/pages/ticket/credit_chip.rs::balance_signal` calls
`api::get_credit_balance()` → `api_get_json`. Every helper in `src/api/mod.rs`
calls `redirect_to_login_expired()` on a 401. The chip's doc comment says a
failed read "degrades to `None` — render nothing", but the API layer redirects
before that code ever sees the error.

This is the **same bug class as `.issues/099`**. `pages/public/discover.rs`
(lines ~139 and ~199) documents the fix it used: call the low-level
`fetch::get`, which does not redirect, because signed-out is a normal state
there.

## Fix (small)

In `balance_signal`, fetch with the non-redirecting `fetch::get` path, or skip
the fetch when `auth::is_authenticated()` is false, and map a 401 to `None`.
Both `CreditBalanceChip` (ticket) and `CreditWallet` go through this function.
`CreditWallet` is probably only mounted on signed-in pages, but check.

Guard: a frontend test that `credit_chip.rs` does not reach
`api_get_json`/`api_get` (the repo's source-grep guard style), so it cannot
regress. Worth sweeping for other components that mount on public pages and
call a redirecting helper (`[[duplicated-state-transition-paths]]`: the fix
landed in discover and the sibling path kept the bug).

## Verify after fixing

Same repro, signed out: expect no `/login` navigation and no chip. Then signed
in with credit: expect the chip.

## Resolution (2026-09-23, session 4)

- New `api_get_json_if_signed_in<T>` in `frontend-leptos/src/api/mod.rs`
  returns `None` when there is no live token, on a 401 or other non-2xx
  status, or when the body can't be parsed. It never calls
  `redirect_to_login_expired()`.
- `get_credit_balance` and **`get_credit_refund_request_status`** both use it
  now and return `Option`. The second one was the sibling path:
  `RequestCreditRefundCard` mounts on the ticket page (`in_person_view.rs`)
  when `dep.held_as_credit`, and it redirected in the same way. For the card,
  signed out now reads as `Ready`, and pressing the button is what asks the
  attendee to sign in (the POST still redirects on 401, which is intended).
- Guard: `frontend-leptos/tests/public_ticket_page_never_redirects.rs` checks
  three things:
  - both endpoints use the helper;
  - no file in `src/pages/ticket/` calls `api_get`/`api_get_json`/
    `api_get_no_cache`/`get_me`;
  - the helper never redirects and checks `is_authenticated()`.
- Gate: wasm32 clippy with `-D warnings` is clean, and 229 frontend tests pass.
- **Still to do:** the browser re-check on staging from "Verify after fixing"
  above. It needs a checked-in fixture and a deploy of this commit to staging.
