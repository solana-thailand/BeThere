# 075 — The Worker's `AttendeeDeposit` decoder still gates on the pre-`version` length

Status: **fixed locally** — length gate corrected, schema-version check added in
both Rust decoders, and the stale-layout mis-decode reproduced before and after.
Closing it fully needs a real devnet run (same gate as
[074](074_attendee_deposit_decode_offsets_stale.md)).

Found by the follow-up audit [074](074_attendee_deposit_decode_offsets_stale.md)
called for: after fixing the e2e script, check whether any *Rust* consumer
carries the same drift from `f246e8e feat(escrow): add version field + padding`.

## Scope of the audit

Every non-program consumer of the two escrow account types was checked:

| type | Rust decoders of raw account bytes |
|---|---|
| `EventEscrow` (192 bytes, disc 1) | **none** — every reference outside `bethere-escrow` is PDA derivation or a base58 address field |
| `AttendeeDeposit` (96 bytes, disc 2) | `worker/src/solana_escrow/wire.rs` and `flow-harness/src/chain.rs` |

So the `EventEscrow` half of the concern raised in 074 is clean: no code reads
its bytes, and its 36-byte padding growth cannot have shifted anything. The
indexer's `read_u64_le(data_bytes, 9)`
(`worker/src/escrow_indexer/mod.rs:104`) reads *instruction* data, not account
data, and is unaffected by the account layout.

## The defect

`decode_attendee_deposit` in `worker/src/solana_escrow/wire.rs` read the
**correct** post-`version` offsets — but gated on the **pre-`version`** length:

```rust
if data.len() < 84 || data[0] != 2 {
    return None;
}
```

84 is exactly the size of the old layout
(`1 + 32 + 32 + 8 + 8 + 1 + 1 + 1`). The doc comment two lines above already
said 96. So a stale-layout account passed the gate and was decoded with v1
offsets, one byte off in the other direction from 074:

| field | truth in an 84-byte record | v1 offsets read |
|---|---|---|
| `amount` (old `[65..73]`) | `15_000_000` | `[66..74]` — garbage |
| `refunded` (old `[82]`) | `false` | `[83]` = `bump` (non-zero) → **`true`** |

There was also no `version` check, so a future v2 account — which the program
itself rejects with `DepositVersionMismatch` — would have been decoded with v1
offsets and returned plausible junk.

## Blast radius — fail-closed, but silent

The only consumer is `verify_attendee_deposit_onchain`, the audit-F1 check that
binds deposit verification to a real on-chain deposit. A mis-decode yields a
garbage `amount` (fails `amount == expected_amount`) *and* `refunded == true`
(fails `!refunded`), so the function returns `Ok(false)`.

**This is the safe direction** — it denies verification rather than granting a
free ticket, so unlike 074 there is no false-green and no money at risk. The
real cost is diagnosability: a layout drift would quietly turn every deposit
verification into "unverified" with nothing in the log stream to say why.

## The fix

`worker/src/solana_escrow/wire.rs`:

- Named constants `ATTENDEE_DEPOSIT_DISCRIMINATOR` / `_VERSION` / `_LEN`,
  matching the ones `flow-harness/src/chain.rs` already had.
- Gate is now `len < 96 || disc != 2 || version != 1` — layout drift fails
  loudly instead of plausibly. Mirrors the program's own `validate_version`
  (`bethere-escrow/src/state.rs:92`), which rejects any version but 1, so the
  worker and the program stay in lockstep.
- The `None` arm at the call site now emits a `tracing::warn!` carrying
  **shape only** — `len`, `discriminator`, `version`. No addresses, no
  identifiers; clears the Issue 070 guard (`worker/tests/log_pii_guard.rs`).

`flow-harness/src/chain.rs`: the decoder already reported `version` in its view
but nothing validated it and no caller read it (`flows/deposit.rs:476`). It now
rejects an unknown version with the same reasoning.

## Verification

Both directions, by execution:

- `rejects_legacy_84_byte_layout` builds a real pre-`version` record and asserts
  the decoder rejects it — **and** asserts that the old offsets really did read
  garbage for `amount` and a non-zero byte for `refunded`, so the test guards a
  reproduced failure, not a hypothetical one.
- `rejects_unknown_version` covers `version = 2` and `version = 0`.
- Reverted the gate to `< 84` and re-ran: **both new tests fail**
  (`assertion failed: decode_attendee_deposit(&legacy).is_none()`). Restored,
  both pass. Same reverse-patch check done for the flow-harness version test.
- Gates: `cargo clippy --all-targets -D warnings` clean on the worker and on the
  harness · `cargo test -p event-checkin-worker` 437 passed / 0 failed / 1
  ignored doctest (wire module 4 → 6 tests) · flow-harness 135 passed / 0 failed.

## Remaining

- **Devnet run** — all verification used synthetic account bytes. Those prove
  the gate matches the program source and the flow-harness decoder, *not* that
  they match what the deployed program writes. Same gate as 074.
- If the escrow program ever ships a v2 account, both decoders must be updated
  before the program upgrade lands, or deposit verification denies everything.
  That is deliberate: denying is the safe direction, and the new `warn!` says so
  in the log stream.

## Related

- [074](074_attendee_deposit_decode_offsets_stale.md) — the same drift in
  `scripts/e2e_devnet_test.sh`, where it *was* a false-green.
- [070](070_worker_log_pii_redaction.md) — the log-field constraint the new
  `warn!` is written against.
