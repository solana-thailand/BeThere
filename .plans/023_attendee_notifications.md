# 023 — Transactional attendee notifications

> **Budget constraint (2026-09-09): $0; no paid services or upgrades.**
> The current Cloudflare Email Sending transport must remain disabled: arbitrary
> recipients require Workers Paid. Its included email quota is not a free-plan
> allowance. Before activation, replace it with a verified free option with hard
> sending limits and no automatic paid overages. A free D1-backed in-app inbox is
> implemented and the email dispatcher has no scheduled trigger.
> Existing activation instructions below are conditional and do not authorize payment.
> Source: https://developers.cloudflare.com/email-service/platform/pricing/


Status: free in-app implementation complete locally; migration and app not deployed.

## Scope and decisions

Registration confirmation, 24-hour reminder, verified/rejected deposit notice,
calendar/ticket links, and organizer delivery history with safe retries.

The attendee landing page now lists due D1 notifications with unread state and
ticket/deposit actions. Its read APIs require a verified-email JWT and update only
rows owned by that email. No provider, sender domain, or notification cron is needed.

- D1 enrollment and registration commit atomically; triggers journal notification
  intent. No historical backfill, no marketing, and no wallet-email inference.
- A signed `Claims.email_verified` flag defaults false for old and wallet sessions.
  Google callback validates `verified_email` before issuing an attested token.
- A single conditional SQL update claims each job. Native Cloudflare Email Sending
  has no documented idempotency key, so unknown/interrupted sends become uncertain
  and are not automatically replayed. Accepted means provider acceptance only.
- Storage, scheduling/retry policy, eligibility, content, and transport have separate
  modules. Pure policy tests do not depend on Cloudflare bindings.
- Per-event history, attendee erasure, enrollment rescheduling and in-flight recovery
  have indexes. Claims remain sequential to avoid marking unsent batches uncertain
  if a worker dies. Capacity is 25 jobs per five-minute cron; verify this against
  the largest planned event before enabling.
- Pending reminder timing is checked again after claim; date/TBA changes defer
  without consuming attempts. A no-op event save does not reset retry backoff.
- API helpers receive `/events/...` paths because they already prepend `/api`;
  browser tests catch double-prefix regressions.
- UI responses are invalidated on newer requests, close and disposal. Delivery
  reads are no-store; event organizer authorization is required on list and retry.
- Attendee pages expose storage failures, serialize read mutations, page backward
  through history, and invalidate responses after disposal. List, unread, read and
  read-all share one D1 eligibility view.
- Reminder history excludes cancelled/TBA/past events and checked-in attendees.
  Ticket/deposit actions use current `deposit_statuses`, participation type, and
  URL-encoded identifiers rather than the legacy attendee deposit column alone.

## Coverage

- [x] Transaction rollback leaves neither attendee nor queued confirmation.
- [x] Duplicate enrollment/deposit writes do not duplicate jobs.
- [x] Concurrent dispatcher claims are mutually exclusive.
- [x] Rejected/reuploaded deposit attempts have distinct notice keys.
- [x] Cancelled events and erased identities do not produce queued sends.
- [x] Retry is event-scoped and rejects accepted/uncertain/pending messages.
- [x] Interrupted sends become uncertain; late completion cannot overwrite them.
- [x] Date changes, TBA, sub-second boundaries and unrelated saves are covered.
- [x] Cleanup and enrollment query plans select the intended indexes.
- [x] Text/HTML safety, encoded links and optional calendar links have unit tests.
- [x] Legacy/wallet claims default to unverified; Google verification is mandatory.
- [x] SQL behavior suite runs in CI, with strict Rust Clippy and normal regression suites.
- [x] Browser integration: organizer and attendee pagination, retry, unread state,
  API failure feedback, close/reopen while loading, and mobile layout (mocked APIs).
- [x] Attendee list/read/read-all APIs are identity-scoped and use existing D1 intent.
- [x] Free in-app UI needs no provider call or recurring dispatch cron.
- [ ] Native email binding integration, only if a no-cost sender domain/provider is later approved.

## Activation gates

1. Apply migration 0030 before the worker (it remains an unpublished migration and
   includes all hardening changes); verify backups and staging schema first.
2. Deploy worker/frontend to staging, keep email sending disabled, and verify
   registration plus organizer/attendee authorization.
3. Verify inbox creation, reminder visibility and read state with a fresh Google session.

See `docs/notifications.md` for operator instructions and `.issues/062_wallet_email_provenance.md`
for the separate identity concern discovered during review. No production migrations,
DNS changes, deploys or live emails have been performed by this implementation.

## Validation evidence

- Worker unit and integration suites: passed (238 unit tests; one ignored doctest).
- Strict Worker and frontend Clippy with `-D warnings`: passed.
- Frontend release WASM build: passed.
- SQL behavior suite: 21 passed, including identity-scoped inbox reads, indexed
  user registration lookup, consolidated dashboard metrics, and two concurrent
  SQLite connections.
- Chromium integration: 7 notification/dashboard tests passed with all APIs
  mocked; no mail or production request was sent.
