# 179: The two bank-info Sheet writers disagree on `account_name`, and one logged all three raw

**Status:** parked (filed 2026-09-30 by session `event-checkin-eb`). The raw-value log is fixed on develop. Reopen the `account_name` half when the owner decides whether the account-holder name belongs in the Sheet.

## What was found

A THB slip upload (`handlers/deposit/thb/handlers/slip_upload.rs:365-395`,
`slip_admin_upload.rs:328-360`) mirrors the refund bank details into the
event's Google Sheet. It uses one of two writers:

| Path | When it runs | Columns written |
|---|---|---|
| `sheets::bg_sync::write_bank_info` | `state.worker_ctx` is set, which is every fetch request (`lib.rs:165`) | `BankAccount`, `BankName` |
| `sheets::write::write_bank_info` | no `worker_ctx` (tests / fallback) | `BankAccount`, `BankName`, `AccountName` |

The bg writer has never taken an `account_name` argument, so in prod the
Sheet's `account_name` column is never filled. D1 still has the value
(`thb_deposits.account_name`). This is the same drift as
`duplicated-state-transition-paths`: two entry points for one write, and they
no longer write the same thing.

The blocking writer also logged all three values raw
(`bank_account_val = ?bank_account`, `bank_name_val`, `account_name_val`),
added in `32944684` as "bank column debug logging". `log_pii_guard` missed it:
its field list had `.bank_account_number` but not a local `bank_account`
logged under a `*_val` name. The path is unreachable in prod today (see the
table), so no prod log is known to hold these values.

## Fixed on develop

- The three `*_val` fields are gone. The column-letter fields stay.
- `worker/tests/log_pii_guard.rs` now forbids the fields `bank_account`,
  `bank_account_val`, `bank_account_number`, `account_name`,
  `account_name_val`, and the expressions `?bank_account`, `%bank_account`,
  `?account_name`, `%account_name`. Before the fix, both scans failed on
  `sheets/write/deposit.rs` and on nothing else. After it, they pass.

## Owner decision (the parked half)

Should the organizer's Sheet carry the account-holder name?

- **Yes:** add `account_name: Option<String>` to `bg_sync::write_bank_info`
  and pass `body.account_name.clone()` from both handlers. It is a 3-line
  change. Update `docs/pdpa_ropa.md` if the Sheet's field list is written
  down there.
- **No (data minimisation):** drop `AccountName` from the blocking writer so
  the two paths agree. Refunds read the name from D1.

Either way, the two writers should end up with the same column set.
