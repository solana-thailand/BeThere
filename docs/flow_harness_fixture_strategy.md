# Flow-harness fixture strategy

## Purpose

The staging harness uses real Worker endpoints, SIWS, and Devnet transactions.
It must never obtain a green production gate by mixing incompatible lifecycle
states in one event or by overwriting initialized escrow metadata.

## Current verified seams

- `auth`: automatic SIWS challenge, signature verification, authenticated
  `/api/auth/me`, and an unauthenticated probe pass on staging.
- `deposit`: the harness follows the same two requests as a wallet:
  `POST /api/deposit/usdc` returns a Solana Pay callback, then
  `GET /api/deposit/usdc/tx` returns the unsigned transaction. A real Devnet
  run reached transaction confirmation; its final assertion correctly rejected
  the expired fixture instead of reporting a false pass.

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
2. Create a new active deposit fixture when the current one expires; assign a
   new worker event ID and on-chain event ID, then initialize it from the
   organizer wallet in the Admin Escrow screen.
3. Use `cargo run -- --flow deposit` or `--flow auth` for diagnosis. Focused
   runs write `summary.json` but cannot update `.last-green`.
4. Only the full suite may refresh `.last-green`. It remains blocked until the
   fixture matrix is fully provisioned; no bypass converts partial coverage
   into a production approval.

## Next implementation

Replace the single mutable seed with named, non-overlapping fixture records and
an explicit freshness check. The fixture bootstrap must report the new event ID
and required organizer initialization transaction, while preserving all
existing initialized rows.
