# Gmail sender setup (plan 045 R4.12)

The subscribe email ("a new event is open") goes out from
**bethere.sol@gmail.com** through the Gmail API. Owner's choice 2026-10-08:
the free path first, and a dedicated project account rather than a personal
one. Why not Cloudflare Email Sending or Brevo/Resend: `.plans/045` R4.12.

What this gives: about 500 recipients a day (consumer Gmail), mail that is
really sent by Gmail (SPF, DKIM and DMARC pass), no domain, no cost.

The grant is `gmail.send` only: whoever holds the token can send as the
account, but cannot read, list or delete its mail.

## Owner steps (once, about 15 minutes)

Do all of them signed in as **bethere.sol@gmail.com**.

1. **Secure the account.** Turn on 2-Step Verification
   (myaccount.google.com → Security). Anyone with this mailbox can send as
   BeThere.
2. **Create a Cloud project.** console.cloud.google.com → New project →
   `bethere-mail`. Keep it separate from the project behind the site's
   Google login, so this send-mail grant never affects the login app's
   consent screen or verification.
3. **Enable the API.** APIs & Services → Library → *Gmail API* → Enable.
4. **Consent screen** (Google Auth Platform):
   - Branding: app name `BeThere mail`, support email bethere.sol@gmail.com.
   - Audience: *External*. Then **Publish app** (status *In production*).
     While the app is in *Testing*, refresh tokens expire after 7 days and
     the mail would stop silently a week later.
   - Data access: add the scope `https://www.googleapis.com/auth/gmail.send`.
   - The app stays unverified. That's fine: only the owner ever consents, and
     the "Google hasn't verified this app" screen is expected (Advanced →
     Go to BeThere mail).
5. **OAuth client.** Clients → Create client → *Desktop app* → name
   `bethere-mail-cli` → Download JSON. Save it as
   `~/.bethere-gmail-client.json`, then `chmod 600 ~/.bethere-gmail-client.json`.
6. **Consent once:**

   ```sh
   python3 scripts/gmail_refresh_token.py ~/.bethere-gmail-client.json
   ```

   The browser opens. Sign in as bethere.sol@gmail.com and allow. The script
   writes `~/.bethere-gmail-refresh-token` (mode 600) and never prints it.
7. Tell the agent the two files are ready. Don't paste their contents into
   chat.

**Status 2026-10-08:** owner steps 1–6 done. Branding verified and
published, app *In production*. Data-access verification (demo video, scope
justification) is not needed: one consenting account, well under the
100-user cap for unverified apps. Don't press "Fix the issue" for it.

## Agent steps (at R4.12 build time, with the owner's go per environment)

- Worker secrets, staging first: `GMAIL_CLIENT_ID`, `GMAIL_CLIENT_SECRET`,
  `GMAIL_REFRESH_TOKEN` (`npx wrangler secret put … --env staging`, piped from
  the files, never echoed).
- Per send: exchange the refresh token at `oauth2.googleapis.com/token` for a
  one-hour access token (cache it in the isolate), then
  `POST gmail/v1/users/me/messages/send` with the RFC 2822 message
  base64url-encoded. Every message carries `List-Unsubscribe` and
  `List-Unsubscribe-Post: List-Unsubscribe=One-Click`, plus the one-click
  token link in the body.
- Budget: stay under 400 recipients a day (headroom under Gmail's ~500), one
  message per recipient (no BCC lists), spread over the notifications outbox.
  More than that means a domain and Brevo (`.plans/045` R4.12 option 3).

## What is built (.plans/045 R4.12)

- D1 (migration 0060): `subscribers` (email, locale, consent time, one-click
  token), `announced_events` (when each public event was first seen open)
  and `event_announcements` (one row per event per subscriber, written before
  the send). The migration marks what is open at apply time as seen, so
  turning it on mails nobody.
- `POST /api/subscribe` (Turnstile, auth rate limit, same answer for new and
  known addresses), `POST /api/unsubscribe/{token}` (no sign-in; the
  `List-Unsubscribe` target) and the page `/unsubscribe/{token}` (a button,
  nothing on load, because scanners follow links). PDPA erasure deletes both
  tables' rows for the address.
- The one hourly cron `17 * * * *` (`src/schedule.rs`; the 03:17 UTC tick
  runs the daily jobs instead): `src/subscribers/announce.rs` marks newly
  open public events, then mails subscribers who asked before the event
  opened, 16 a run (368 a day). A 401/403/429 frees the claim for the next
  run; any other refusal, a 5xx or a network error keeps it (no blind
  replay). Without the three secrets it stores subscribers and sends nothing
  (`skipped = "no gmail secrets"` in the log).
- `MAIL_FROM = "bethere.sol@gmail.com"` is a var in `wrangler.toml`; the
  secrets are the three above.

## Revoking

- To stop sending at once: myaccount.google.com → Security → Third-party
  access → BeThere mail → Remove access. That kills the refresh token; the
  worker's sends then fail with `invalid_grant`.
- To rotate: delete the old client in the Cloud console, then redo steps 5–6.
