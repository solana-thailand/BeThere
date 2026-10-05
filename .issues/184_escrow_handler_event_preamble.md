# 184: Escrow tx handlers repeat the event-load preamble six times

**Status:** parked. Reopen trigger: the 8 Oct take is done and the freeze is
lifted. Code smell, not a live bug (reasoning below). Filed by session
`event-checkin-a6`, 2026-10-01.

## What

`worker/src/handlers/deposit/escrow/handlers.rs` has six handlers that each
open with the same block:

- `refund_and_close_tx_handler`
- `mark_checked_in_tx_handler`
- `deactivate_event_tx_handler`
- `close_event_tx_handler`
- `claim_forfeited_tx_handler`
- `close_deposit_tx_handler`

The block is:

1. `state.events_kv` or 500 "EVENTS KV not configured";
2. `event_store::get_event_config(kv, &body.event_id)` (KV only) or 404;
3. `deposit_enabled` or 400;
4. in two of them, a non-empty `escrow_address` or 400.

`init_escrow_tx_handler` differs. It resolves KV → D1
(`resolve_event_or_fallback`, `964f9bdd`) because an event can exist only in
D1 (a staging SQL seed, or a KV put that failed at create; `create.rs` only
logs that failure).

## Why it is not a live bug

The six handlers only do useful work once the event has an `escrow_address`.
That address reaches the event through `update_event`
(`event_store/write/update.rs`), which loads the event with the D1 fallback and
then calls `save_event_config(kv, …)?`. The KV write is fatal there, so any
event with an escrow address is in KV. A D1-only event reaching these handlers
has no escrow yet, and the handlers would refuse it anyway.

This rests on reading the code. It is not probed: a D1-only event after init
on staging would confirm it.

## Fix, when unparked

- Add one helper next to the handlers, e.g.
  `load_deposit_event(state, event_id) -> Result<EventConfig, WorkerError>`,
  built on `event_store::get_event_config_with_fallback`. It applies the
  `deposit_enabled` check, with an `EscrowRequired` flag for the
  initialized-escrow check.
- Swap the six preambles for it. Keep the error texts byte-identical:
  `scripts/e2e_devnet_test.sh:710` greps for "escrow not initialized".
- The same KV-only pattern also appears in `status.rs` (3×),
  `thb/handlers/slip_upload.rs` and `thb/handlers/hold_credit.rs`. Decide
  whether they share the helper.
- Gate: `cargo test -p event-checkin-worker` (whole crate; see the filtered-run trap), plus
  the devnet e2e (`scripts/e2e_devnet_test.sh`) on staging, because these are the
  money paths.

## Why parked now

These handlers move real deposits. RTM #6 is 4 Oct and the freeze is
6–8 Oct. A pure refactor of the refund/check-in/close paths buys nothing
before then and risks the take.
