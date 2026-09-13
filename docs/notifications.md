# Attendee notifications

> **Budget constraint (2026-09-09): $0; no paid services or upgrades.**
> The current Cloudflare Email Sending transport must remain disabled: arbitrary
> recipients require Workers Paid. Its included email quota is not a free-plan
> allowance. Before activation, replace it with a verified free option with hard
> sending limits and no automatic paid overages. The production path is currently
> the D1-backed in-app inbox described below.
> Existing activation instructions below are conditional and do not authorize payment.
> Source: https://developers.cloudflare.com/email-service/platform/pricing/


BeThere creates registration confirmations, event reminders, deposit confirmations,
and rejected-slip notices for **new self-registrations with proven email ownership**.
Wallet sessions (including linked-email sessions), legacy sessions without the
verification claim, imports, walk-ins, and historical registrations are not enrolled.
Fresh Google sessions carry a signed `email_verified` attestation only after the
provider response passes verification. Legacy users can sign in again to obtain
this attestation for future registrations. These are event service messages, not marketing.

## Setup

1. Apply migration `0030_notifications.sql` **before deploying the worker**. The
   registration write now batches the attendee and notification enrollment in D1;
   a missing migration correctly fails the registration instead of losing its jobs.
2. Deploy the worker and frontend together. Due messages appear in the signed-in
   attendee's landing-page inbox without a scheduled job or external service.

### Optional future email transport

1. Onboard a sender domain that you own and verify its DNS setup. A `workers.dev`
   hostname cannot be used because its DNS zone is owned by Cloudflare.
2. In `worker/wrangler.toml`, uncomment `[[send_email]]` / `name = "EMAIL"`, set
   `NOTIFICATION_FROM` to an address on that domain, and set
   `NOTIFICATIONS_ENABLED = "1"` when ready to send. `SERVER_URL` must be the public
   HTTPS origin. Staging needs its own explicit settings and a restricted recipient
   binding; never point local tests at production recipients.
3. Add a dispatch schedule suitable for the chosen provider. The checked-in config
   keeps only the existing daily cleanup and reconciliation cron.
4. Test with an explicitly authorized address you control, then inspect the event's
   **Notifications** panel and Cloudflare Email logs. A provider `messageId` means
   acceptance, not confirmed inbox delivery. No real messages are sent by tests.

Sending is disabled in the checked-in config. New registrations still create jobs
while disabled. Review queued registrations before enabling; registration and
payment messages are cancelled once the event ends. There is no historical backfill.
A missing binding or invalid sender stops dispatch before jobs are claimed.

Cloudflare references:
[domain setup](https://developers.cloudflare.com/email-service/get-started/send-emails/),
[Workers binding API](https://developers.cloudflare.com/email-service/api/send-emails/workers-api/).
The installed workers-rs version lacks the structured email builder wrapper, so
`notifications/transport.rs` calls the documented native binding through a small
JS bridge. Provider payloads include both text and escaped HTML; no SMTP or REST
API token is needed.

## Delivery and duplicate behavior

- Registration and outbox enrollment are committed in one D1 batch. A trigger
  creates one confirmation and one reminder; replayed enrollments are no-ops.
- Reminders become due 24 hours before the event, or immediately for registrations
  made within that window. Unknown times wait for the organizer to set a time.
  Changes to event dates reschedule pending reminders. A time change detected
  after claiming defers the reminder without consuming a retry attempt; unrelated
  event saves preserve backoff. Already accepted reminders
  are not sent again. Date-change notifications are outside this implementation.
- Deposit triggers journal actual verified/rejected transitions using the payment
  attempt's `deposited_at` value. Identical writes do not create another message;
  a newly uploaded slip can receive a new rejection notice.
- Each job is atomically claimed before transport. The dispatcher rereads the
  event, attendee and deposit, suppresses obsolete payment notices, and skips
  reminders for checked-in attendees. URLs point to authenticated ticket/deposit
  pages, with event time in UTC and an Add to calendar link when dates are known.
- Known rate/quota rejections retry with 5/10/20/40-minute backoff, at most five
  attempts. Known validation, sender and recipient failures require organizer
  retry after their cause has been fixed. Pre-send preparation failures also use
  bounded retries. Delivery history is paginated in groups of 50.
- **There is no exactly-once delivery guarantee across the database/provider
  boundary.** Cloudflare's documented binding has no idempotency key. Unknown
  transport errors and interrupted sends are marked `uncertain`; they are never
  automatically retried and the UI does not offer resend for them. Inspect the
  provider logs before any manual recovery. A crash after provider acceptance but
  before persisting the result therefore cannot cause an automatic duplicate.
- Accepted messages keep the provider ID for diagnosis. The outbox stores record
  references and error codes, not recipient addresses or rendered bodies. Listing
  resolves current attendee details behind the event organizer authorization gate.
  Attendee deletion, email erasure/change and event deletion remove enrollment and
  delivery records. Ordinary delivery history otherwise follows event retention.

## Organizer endpoints

- `GET /api/events/{id}/notifications?before={cursor}` — newest 50 messages;
  `next_before` is the cursor for older results.
- `POST /api/events/{id}/notifications/retry` with `{ "notification_id": 123 }` —
  requeue a known `failed` message. Requires Organizer or SuperAdmin for that event;
  rejects pending, sending, accepted, uncertain and cancelled messages.

## Attendee inbox endpoints

- `GET /api/my-notifications?before={cursor}` returns due notifications and the
  unread count, newest first in pages of 50.
- `POST /api/my-notifications/{id}/read` marks one owned notification as read.
- `POST /api/my-notifications/read-all` marks all due owned notifications as read.

All three require a JWT with `email_verified=true` and scope every D1 query to the
attendee row matching that email. Read state never changes email delivery state.

## Verification

The UI is covered by `e2e/notifications.spec.ts`. Against a locally served current
frontend, run `BASE_URL=http://127.0.0.1:3001 npm run test:e2e -- notifications.spec.ts`
from `worker/`. The tests intercept API requests and block external network calls;
they do not require a real worker, identity provider, or email service. They cover
retry, pagination, stale responses on close/reopen, and mobile table sizing.


```sh
python3 -m unittest discover -s worker/tests/notifications -v
cargo test -p event-checkin-worker
cargo check -p event-checkin-worker --target wasm32-unknown-unknown
cargo check --manifest-path frontend-leptos/Cargo.toml --target wasm32-unknown-unknown
```

The CI workflow runs the Python SQL suite alongside the workspace Rust tests.
SQLite tests execute all production migrations and the exact dispatcher SQL,
including concurrent claims, rollback, replay, event-scoped retries, date changes,
and erasure. Live Cloudflare binding delivery remains an activation check.
