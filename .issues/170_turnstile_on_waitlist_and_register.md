# 170: Turnstile bot check on the waitlist and on self-registration

**Status:** deployed to prod `f02143d4` (2026-09-29, `deploy/production/20260929T050434Z`, session `event-checkin-1b`; staging runs the same tree as `bb8ac906`). Was: fixed on develop (2026-09-29, session `event-checkin-ba`). The check is inert until the owner creates the widget and sets both keys (see "Owner steps"). This is P0-2 of the GOAT-hardening handoff (`.plans/038`).

## Scope, and why it is not what the handoff said

The handoff asked for Turnstile on "public registration + walk-in". The code
says something different:

| Endpoint | Who can call it | Rate-limited | Gated now |
|---|---|---|---|
| `POST /api/waitlist` | **anyone**; appends to a Google Sheet | no | **yes** |
| `POST /api/public/register` | a signed-in OAuth identity (`attendee_authed` router) | no | **yes** |
| `POST /api/walkin/register` | authenticated **staff** (`protected` router) | — | **no** |

- **The waitlist** is the only anonymous write. It had no protection at all.
- **Registration** is JWT-gated, but OAuth accounts are cheap to script, and
  a scripted run can fill an event's capacity.
- **Walk-in** already needs a staff JWT. A challenge there would put friction
  at the door, the one moment speed matters, and would stop no bot the JWT
  does not already stop.

## Behaviour

- **The gate** (`worker/src/turnstile.rs`) is on only when
  `TURNSTILE_SITE_KEY` and `TURNSTILE_SECRET_KEY` are both non-empty **and**
  `DEV_MODE` is off. So deploying the code before the widget exists changes
  nothing, and the dev-token e2e scripts keep working.
- **When on, it fails closed.** A missing or rejected token returns 403
  `forbidden: human check failed or expired, please try again`. A siteverify
  outage returns 502.
- **Ordering.** The token travels in the `x-turnstile-token` header, so no
  request body type changed. The check runs after the cheap validation,
  because siteverify spends the token, and before any sheet or D1 work.
- **Logging.** Rejections are logged at info with the route and siteverify's
  error codes. The token and the IP are not logged.
- **`GET /api/public/turnstile/config`** returns `{enabled, site_key}`. It
  mirrors the gate exactly, so the widget shows if and only if the token will
  be checked.
- **Frontend** (`src/bot_check.rs`, `js/turnstile.js`):
  - The waitlist fetches the config and Cloudflare's script only on first
    focus of the form. A landing view that never touches the form makes no
    extra request and no third-party contact.
  - The register form (shown only to signed-in visitors who can register)
    starts on render.
  - One config request serves every form on the page, including concurrent
    renders.
  - Submit waits for a token. The token is reset after every attempt.
  - A rejection shows localised copy (EN + TH).
- **CSP:** `https://challenges.cloudflare.com` was added to `script-src` and
  `frame-src`, in `middleware/headers.rs` and `_headers` (the parity test
  passes).
- **Bundle:** +5,209 bytes br4 on the first load (1,970,397 → 1,975,606). The
  first load was already past the warn line (`.issues/169`).

## Verified (2026-09-29, `wrangler dev`, headless Chrome at 390×844)

Every run used Cloudflare's documented test keys. The site key was
`1x00000000000000000000AA`, which always passes.

**With `DEV_MODE` off and the always-fail secret `2x…AA`:**
- A curl with no header got 403. A curl with a dummy token got 403. Both
  wrote nothing, and the reason was logged.
- In the browser:
  - there were 0 requests to challenges.cloudflare.com before the form was
    touched;
  - after focus, the widget rendered and submit unlocked once the token
    arrived;
  - the POST carried the header;
  - the rejection copy appeared in EN and in TH;
  - there were no CSP violations.

**With `DEV_MODE` off and the always-pass secret `1x…AA`:** the request
passed the gate (200).

**With `DEV_MODE` on:**
- the config reported `enabled: false`;
- the register form rendered with no widget and an enabled submit button;
- there was one config request for two form renders.

**Tests:** `worker/tests/turnstile_gate.rs` (4) covers the gate, the token
shape, the siteverify parse, and the rejection text surviving the error
envelope and redactor.

## Test data left behind (owner, please decide)

The always-pass run meant to prove "passes the gate" without writing
anything. It used `--var GOOGLE_SHEET_ID:probe-no-such-sheet`, but the worker
reads `GOOGLE_SHEET_ID` as a secret from `.dev.vars` first, so the override
did not apply. **One row, `probe@example.com`, was appended to the waitlist
tab of the dev sheet** named in `worker/.dev.vars`. It was not removed:
deleting rows from a shared sheet is the owner's call.

## Owner steps to turn it on

1. Create a Turnstile widget (Managed mode) for the prod and staging
   hostnames.
2. `npx wrangler secret put TURNSTILE_SECRET_KEY`, then set
   `TURNSTILE_SITE_KEY` (a var or a secret) for each environment.
3. On staging with `DEV_MODE` on, the gate stays off. To exercise it there,
   either turn `DEV_MODE` off or use a Google sign-in.
