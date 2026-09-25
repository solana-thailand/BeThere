# 148: D1 binds of amounts, slots and block times wrap at 2^31

**Status:** deployed 2026-09-24 (prod version `36eae0db`, git `897aa07`). No prod value is known
to have wrapped. The prod query below would confirm that, and it is still owed.
**Found by:** session `event-checkin-64`, from the "noted, not filed" list
(`onchain_events.rs:139`).
**Severity:** low-medium. It is latent at today's deposit sizes. When it
fires, a money column is silently stored negative.

## What happens

`worker` 0.8's `D1Type::Integer` holds an `i32`. Eleven binds in `worker/src/db/`
cast wider values with `as i32`, which wraps without an error:

| Site | Value | Wraps above |
|---|---|---|
| `onchain_events.rs` insert | `slot: u64`, `amount: Option<u64>` (USDC base units) | slot 2^31; amount 2,147.48 USDC |
| `onchain_events.rs` insert | `block_time: i64` | 2038-01-19 |
| `deposit_statuses.rs` insert/update | `amount: u64` (USDC base units or THB) | 2,147.48 USDC |
| `attendees/deposit.rs`, `attendees/writes.rs` | `deposit_amount_usdc: i64` | 2,147.48 USDC |
| `credit_ledger.rs` record/try_spend | `delta`, `amount: i64` | 2,147.48 USDC of credit |
| `thb_deposits.rs` insert/update | `amount_thb: u64` | 2.1 B THB (theoretical) |

Example: a 2,500 USDC deposit (`2_500_000_000`) was bound as `-1_794_967_296`.

`credit_coverage::apply` already refused amounts above `i32::MAX` with an
explicit error, so it was not affected.

## Fix

`worker/src/db/d1_int.rs` provides `int_bind(field, i64)` and
`uint_bind(field, u64)`:
- They return `D1Type::Real`. `worker` 0.8 converts `Integer(i)` to
  `JsValue::from_f64(i as f64)`, so the JS value is byte-identical for every
  in-range integer, and exact up to 2^53 - 1.
- A value past ±(2^53 - 1) returns an `IntBindError` that names the column. It
  is never rounded.

All 11 sites use these helpers. The remaining `as i32` binds are small counters
(limits, `row_index`, quiz attempts, adventure stats, `deposit_order`) and bools.

## Tests

`worker/tests/d1_int_bind_exactness.rs` has 4 tests:
- the i32 boundary, the 2^53 boundary, and refusal past it;
- a source guard over `src/db/**`: a `D1Type::Integer(... as i32)` whose line
  names amount, delta, slot, block_time, usdc or thb fails.

A/B: putting the `thb_deposits.rs` cast back turned the guard red on both lines.

## Owed (owner-gated)

- A prod read to confirm nothing has wrapped yet:
  `SELECT COUNT(*) FROM onchain_events WHERE amount < 0 OR slot < 0;` plus the
  same check for `deposit_statuses.amount` and `attendees.deposit_amount_usdc`.
