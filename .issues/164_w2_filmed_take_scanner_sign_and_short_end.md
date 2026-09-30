# 164: The W2 filmed take has two unrehearsed steps: the scanner's wallet sign and a short event end

**Status:** open (2026-09-29): gap 2 fixed on develop (`0da1be79`, 2026-09-29) and rehearsed on staging; gap 1 (the owner's Phantom walk-through) is still open. Found by session `event-checkin-cf` during the W2 devnet rehearsal (`.plans/033` §6). Both gaps must be closed before the final demo take on Thu 8 Oct.

## What the rehearsal covered

`scripts/e2e_devnet_test.sh --skip-setup --non-interactive` passed twice on staging (`a3d4e7a9`) and devnet. Each run did init, deposit, webhook verify, `mark_checked_in`, then refund with the PDA closed. The script signs `mark_checked_in` itself, through `POST /api/escrow/mark-checked-in`. The scanner calls the same builder (`frontend-leptos/src/api/deposit.rs` `mark_checked_in`), so the transaction content is rehearsed. The browser part is not.

## Gap 1: the scanner's on-chain check-in has never been run in a browser

W2 films "scan at the door → `mark_checked_in`". That path is `docs/devnet_e2e_walkthrough.md` §15 (Flow 10). The steps are: off-chain check-in, "Mark Checked In On-Chain", connect Phantom with the **organizer** wallet (`9Bz7…`, devnet), then sign. No issue, handover or plan records a run of it. `.handovers/107` carried "designate a staff scanner with Phantom + devnet SOL" as an open risk and never closed it. It needs a human-approved wallet popup, so a headless probe can't cover it.

**Owner step:** import the organizer devnet key into Phantom on the device that will film, then walk Flow 10 once on staging.

## Gap 2: no fixture makes an event end minutes after check-in

`refund` needs `clock >= event_end` (`.issues/129`). The e2e script sets `event_end` 120 s after creation and does not register an attendee through the UI, so it has no ticket QR to scan. `bethere-mcp/examples/demo_fixture.rs` does make a scannable event, but it hardcodes `event_end_ms = start + 3 h`. The filmed take needs an event with a real attendee and QR that ends about 10 minutes after the scan.

**Fix (no owner needed):** let `demo_fixture` read the event end from an environment variable, e.g. `BETHERE_DEMO_END_MIN`, keeping the default at 3 h. This touches the fixture only, not the USDC deposit flow.

**Done (2026-09-29, session `event-checkin-83`, uncommitted):**
`demo_fixture` reads `BETHERE_DEMO_END_MIN=<n>` (n >= 2). The event ends n
minutes from now and starts one minute earlier; without the variable it
keeps 48 h out for 3 h. Registration closes at the start (in the docs; see
below) and `mark_checked_in` needs `clock <= event_end`, so the filmed take
has to register, pay and scan in the first n − 1 minutes. The fixture and
`bethere-mcp/README.md` are the only files changed. `bethere-mcp` gates were
all clean: `cargo fmt --check`, `cargo clippy --locked --all-targets -D warnings`
exit 0, and 11 tests passed.

**Staging rehearsal (`a3d4e7a9`, devnet, 2026-09-28 23:55 to 00:03 +07):**
the driver was `/tmp/gap2/run.py` and the log is `/tmp/gap2/run.log`, both
outside the repo. With `BETHERE_DEMO_END_MIN=8`, event
`agent-demo-meetup-1790614521` read back start `1790614941057` and end
`1790615001057` (60 s apart, 474 s out); escrow
`HEpXFSgESL928XMatLYhPFtpxe8Feq5AURf1L1ekHZwT`. In order:

| Step | Result |
|---|---|
| `register` via `bethere-mcp` (agent wallet `54GK…`) | 200, attendee `01a0e8f1-88dd-79c1-8d76-ca1092c32931` |
| `pay_deposit` | `3ccs7MB5…` finalized, `verified: true` |
| staff scan `POST /api/checkin/{id}` | 200; a repeat scan returns 400 "already checked in" |
| `mark_checked_in`, organizer signs | `4PcZ4Hyb…` finalized |
| `refund` before the end (simulated) | rejected on chain: `Custom(1)` = `RefundNotYetAllowed` |
| `refund` 20 s after the end, agent signs | `5PL4weDa…` finalized |

So the window works as the take needs it to: the scan and the on-chain
check-in land before the end, and the refund opens only after it.

Two things to know before filming:
- **Registration did not close at the start.** A second `register` 11 s after
  the start returned 200. That is a product finding, not a fixture bug: see
  `.issues/165`. It does not block the take.
- **`ticket_status` does not show an escrow refund.** After the on-chain
  refund, `deposit_info.refunded` was still `false`. The attendee signs the
  refund straight to the chain and the worker never records it, so show the
  refund with the explorer link or the wallet balance, not the ticket page.

It took three attempts. The earlier ones stopped in the driver before any
attendee existed: first the KV lag after activation, then Cloudflare 1010
on the `Python-urllib` User-Agent, then the event's required contact
channel. They left events `agent-demo-meetup-1790614386`, `-1790614414` and
`-1790614490` on staging, each with an initialized escrow and no deposits.
The staff-scan CPU from this run is `.benchmarks/007`.

## Not a gap

- `solana transfer` and `spl-token transfer` now work against devnet (both finalized, 28 Sep). The 27 Sep "error sending request" did not reproduce.
- Badge minting can't be filmed on staging (no Crossmint secret). W2 already plans to film the badge on prod, where it is live, so it is a separate clip.

## Gap 1 rehearsal fixture (2026-09-30, session `event-checkin-b5`)

Staging, devnet. Built with `demo_fixture` plus `bethere-mcp` `register` and
`pay_deposit` (the first half of `/tmp/gap2/run.py`; driver
`/tmp/ec-b5-rehearsal/setup.py`, outside the repo). Check-in was left undone
for the owner's Phantom run:

- Event `agent-demo-meetup-1790757649`, active. It ends Tue 6 Oct 23:58 ICT;
  `mark_checked_in` needs `clock <= event_end`.
- Escrow `97cGnfZCWQC43AwycSWV3yPQUq9GvbAuvNxL87od7bau`, initialized. The
  scanner's on-chain prompt shows because `deposit_enabled` is true and
  `escrow_address` is set.
- Attendee `01a0f179-9b4a-7a13-82a0-cf4d2d629dfe` ("Phantom Rehearsal"). It
  paid 1 USDC from agent wallet `54GK…` (verified), and D1 `checked_in_at` is
  NULL.
- Ticket (QR):
  `https://bethere-staging.solana-thailand.workers.dev/ticket/01a0f179-9b4a-7a13-82a0-cf4d2d629dfe?event_id=agent-demo-meetup-1790757649`
- The organizer to sign in Phantom (devnet) is `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN`.
