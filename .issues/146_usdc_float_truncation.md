# 146: USDC decimal → atomic conversions truncate, so "2.01" becomes 2,009,999

**Status:** deployed 2026-09-24 (prod version `36eae0db`, git `897aa07`). The organizer form was not
driven in a browser; the conversion is covered by `domain/tests/golden_vectors.rs`
(every cent from 0 to 1,000 USDC through both paths). The prod count below is owed.
**Found by:** session `event-checkin-2f`, while scoping the rounding golden
vectors of `.plans/030` §3.
**Severity:** low-medium. No funds are lost, but the on-chain deposit amount,
the price shown to attendees and indexed deposit/refund amounts can be one
atomic unit short, and the display then drops a whole cent.

## What happens

Every place that turns a decimal USDC amount into atomic units (1 USDC =
1,000,000) does `(x * 1_000_000.0) as u64`. `as u64` truncates toward zero, and
most decimal fractions are not exact in f64, so the product often lands just
below the integer.

| Site | Input | Effect |
|---|---|---|
| `frontend-leptos/src/pages/event_form.rs:688` (create) | organizer types `2.01` | stored `deposit_amount_usdc = 2_009_999`; escrow `create_event` is built with it |
| `frontend-leptos/src/pages/event_form.rs:934` (edit) | form re-shows `{:.6}` then re-parses | 369,679 of the 20,000,001 atomic values ≤ 20 USDC drift down by 1 on a plain re-save |
| `frontend-leptos/src/pages/escrow_init/panel.rs:241` | same form value | same as create |
| `worker/src/escrow_indexer/webhook.rs:235` | Helius `tokenAmount` (f64 UI amount) | deposit/refund/rollover amount indexed 1 short; rollover writes it into the target event's `DepositStatus.amount` |

The display then makes it visible: both `format_usdc` helpers
(`frontend-leptos/src/utils/money.rs:20`, `domain/src/pr_pack.rs:91`) floor to
cents, so 2,009,999 renders as **2.00**, not 2.01.

Counted with Python's float, which is IEEE-754 like Rust's: 1,196 of the
100,001 two-decimal amounts from 0.00 to 1,000.00 truncate wrongly (the first
is 2.01). `round()` in place of `as u64` is exact for every atomic value up to
the 1,000 USDC cap, sampled every 997 units.

## Fix

- `domain::money` holds the only conversions:
  - `parse_usdc_atomic(&str) -> Option<u64>` parses the decimal string exactly
    (integer arithmetic, at most 6 fraction digits, no float);
  - `usdc_ui_to_atomic(f64) -> Option<u64>` rounds to nearest for the Helius
    UI amount and rejects NaN, infinities and negatives.
- The three frontend sites and the indexer call them. Invalid form input still
  becomes 0, as before; only the valid-input result changes.
- Golden vectors in `domain/tests/fixtures/golden_vectors.json` pin both
  (`.plans/030` §3).

## Not in scope (noted)

- Two `format_usdc` copies (domain `pr_pack` and frontend `utils/money`)
  floor to cents. Correct once the stored value is right.
- `worker/src/db/onchain_events.rs:139` binds `amount as i32`. That is safe
  only while the 1,000 USDC cap (1e9 < i32::MAX) holds. `slot as i32` on the
  same insert passes i32::MAX near slot 2.1e9.

## Checks still owed

- A read-only prod count of events with `deposit_amount_usdc % 10000 != 0`,
  run by the owner: `SELECT id, deposit_amount_usdc FROM events WHERE
  deposit_amount_usdc % 10000 != 0;`. Rows that are off by 1 are organizer
  typos turned into odd values. If the event has no escrow yet, fix it with
  `PUT /api/events/{id}`, not `d1 execute` (the KV-first read path hides a
  direct write). If an escrow exists, its on-chain `deposit_amount` is fixed
  at `create_event`; leave the row alone so D1 and the chain agree.
