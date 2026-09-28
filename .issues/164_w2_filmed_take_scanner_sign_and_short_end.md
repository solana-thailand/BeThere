# 164: The W2 filmed take has two unrehearsed steps: the scanner's wallet sign and a short event end

**Status:** open (2026-09-28). Found by session `event-checkin-cf` during the W2 devnet rehearsal (`.plans/033` §6). Both gaps must be closed before the final demo take on Thu 8 Oct.

## What the rehearsal covered

`scripts/e2e_devnet_test.sh --skip-setup --non-interactive` passed twice on staging (`a3d4e7a9`) and devnet. Each run did init, deposit, webhook verify, `mark_checked_in`, then refund with the PDA closed. The script signs `mark_checked_in` itself, through `POST /api/escrow/mark-checked-in`. The scanner calls the same builder (`frontend-leptos/src/api/deposit.rs` `mark_checked_in`), so the transaction content is rehearsed. The browser part is not.

## Gap 1: the scanner's on-chain check-in has never been run in a browser

W2 films "scan at the door → `mark_checked_in`". That path is `docs/devnet_e2e_walkthrough.md` §15 (Flow 10). The steps are: off-chain check-in, "Mark Checked In On-Chain", connect Phantom with the **organizer** wallet (`9Bz7…`, devnet), then sign. No issue, handover or plan records a run of it. `.handovers/107` carried "designate a staff scanner with Phantom + devnet SOL" as an open risk and never closed it. It needs a human-approved wallet popup, so a headless probe can't cover it.

**Owner step:** import the organizer devnet key into Phantom on the device that will film, then walk Flow 10 once on staging.

## Gap 2: no fixture makes an event end minutes after check-in

`refund` needs `clock >= event_end` (`.issues/129`). The e2e script sets `event_end` 120 s after creation and does not register an attendee through the UI, so it has no ticket QR to scan. `bethere-mcp/examples/demo_fixture.rs` does make a scannable event, but it hardcodes `event_end_ms = start + 3 h`. The filmed take needs an event with a real attendee and QR that ends about 10 minutes after the scan.

**Fix (no owner needed):** let `demo_fixture` read the event end from an environment variable, e.g. `BETHERE_DEMO_END_MIN`, keeping the default at 3 h. This touches the fixture only, not the USDC deposit flow.

## Not a gap

- `solana transfer` and `spl-token transfer` now work against devnet (both finalized, 28 Sep). The 27 Sep "error sending request" did not reproduce.
- Badge minting can't be filmed on staging (no Crossmint secret). W2 already plans to film the badge on prod, where it is live, so it is a separate clip.
