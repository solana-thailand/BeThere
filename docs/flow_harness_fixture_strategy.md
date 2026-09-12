# Flow-harness fixture strategy

## Purpose

The staging harness uses real Worker endpoints, SIWS, and Devnet transactions.
It must never obtain a green production gate by mixing incompatible lifecycle
states in one event or by overwriting initialized escrow metadata.

## Current verified seams

- `auth`: automatic SIWS challenge, signature verification, authenticated
  `/api/auth/me`, and an unauthenticated probe pass on staging.
- `deposit`: a dedicated staging attendee completed a real Devnet USDC deposit.
  The Worker persisted the verified D1 record, and the harness confirms the
  matching `AttendeeDeposit` PDA. The named-fixture status request always sends
  `event_id`, so it cannot silently read another active staging event.
- `confirmation`: an opt-in focused probe creates a fresh SIWS session, calls
  the authenticated `/api/deposit/usdc/confirm` route for an already verified
  fixture, and verifies the same Devnet PDA. It passed on 2026-09-12.

## Why one event cannot cover every case

An active event is required for a fresh deposit and the pre-event refund
negative case. Checked-in refunds require a past event end. No-show deadline
tests require a past refund deadline. An escrow event cannot be re-created
with a past end time, and a submitted deposit/refund changes its on-chain PDA.
These are mutually exclusive states for a repeatable Devnet fixture.

## Fixture matrix

| Scenario | Environment | Required state | Gate role |
| --- | --- | --- | --- |
| Auth and active deposit smoke | Staging + Devnet | Fresh active event, initialized escrow, funded dedicated attendee | Required live smoke |
| Pre-end refund reject | Program test / local validator | Clock before event end | Required deterministic contract test |
| Checked-in post-end refund | Program test / local validator | Clock after end, attendee marked on-chain | Required deterministic contract test |
| No-show deadline reject | Program test / local validator | Clock after deadline, attendee not checked in | Required deterministic contract test |
| NFT claim | Staging after a separate claim fixture is provisioned | Valid claim token and checked-in attendee | Optional until NFT provider is configured |

## Operating rules

1. Never run `seed-staging.sh` against an event whose `escrow_status` is
   `initialized`, `deactivated`, or `closed`.
2. Create a new active deposit fixture when the current one expires:
   `bash worker/scripts/seed-staging.sh --event-id flow-deposit-YYYYMMDD`.
   The harness derives the matching on-chain event ID from the event ID; then
   initialize the event from the organizer wallet in Manage Events → Edit →
   Escrow Management.
   The seed script refuses initialized, deactivated, or closed escrow rows.
   Create a new event ID instead of attempting to reuse them.
3. Use `cargo run -- --flow deposit` or `--flow auth` for diagnosis. Focused
   runs write `summary.json` but cannot update `.last-green`.
   To include the authenticated confirmation route for a verified fixture, set
   `FLOW_HARNESS_VERIFY_CONFIRMED_DEPOSIT=1`. This remains read-only and must
   not be used as evidence for an unverified or newly sent transfer.
4. Only the full suite may refresh `.last-green`. It remains blocked until the
   fixture matrix is fully provisioned; no bypass converts partial coverage
   into a production approval.

## Remaining coverage

Named, non-overlapping seed records and an explicit freshness boundary are now
implemented. Before a production escrow release, provision separate fixtures
for the refund and NFT-claim matrix, run the complete suite green, and retain
the resulting full-suite sentinel for the production preflight gate.
