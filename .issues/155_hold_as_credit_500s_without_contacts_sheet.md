# 155: Hold as credit 500s when no contacts sheet is configured

**Status:** fixed on develop, not deployed. Found on staging on 2026-09-27 (session `event-checkin-40`) while building a fixture for the `admin_deposit.rs` split check.

## What happens

`POST /api/refund/hold/{attendee_id}` (admin "Hold as Credit") returned
`{"success":false,"error":"internal error"}` on staging for a verified cash
deposit. The attendee path (`hold_deposit_handler`) has the same code.

Both handlers resolve the org's contacts sheet and then did:

```rust
if resolved.sheet_id.is_empty() {
    return Err(AppError::Internal("contacts sheet not configured".to_string()).into());
}
```

before settling the deposit. Staging has no `CONTACTS_SHEET_ID` secret
(`wrangler secret list --env staging`); prod has one. So on staging no deposit
can be held as credit, by the admin or by the attendee.

The same handlers say, a few lines later, that the D1 `credit_ledger` is the
authoritative record and the Sheets increment is a "best-effort display
mirror" that "never fails the request". The config check contradicted that:
a missing mirror blocked a money action. The other credit mirror
(`register/signup.rs`, credit decrement) already skips when the sheet is empty.

## Impact

- **Prod:** none today. `CONTACTS_SHEET_ID` is set, and
  `resolve_contacts_sheet` falls back to it when an org has no sheet of its own.
  It would bite if that secret were ever removed.
- **Staging:** hold as credit was untestable end to end.
- **Money state:** consistent. The check ran before the settle CAS; the probe
  deposit read back `held_as_credit = 0`, and there were no ledger rows for the event.

## Fix

In `hold_credit.rs` and `hold_admin.rs`:

- The empty-sheet error now fires only when there is also no D1. Without D1
  the sheet is the only record, so it is still required.
- The Sheets increment is skipped, with a warning, when the sheet is empty.
  The ledger write before it is unchanged.

Guard: `worker/tests/hold_contacts_sheet_guard.rs` (2 tests). It fails on the
old code, which still has the unconditional check.

## Verify

After a staging deploy: create a THB event with a walk-in attendee, record a
verified slip (`/api/deposit/thb/admin-upload`), then
`POST /api/refund/hold/{attendee}` with `{"event_id": …}`. Expect 200,
`held_as_credit = 1`, and one `credit_ledger` row with `reason = 'hold'` for
the event. The fixture script is `/tmp/ux_verify/split_fixture.sh` (outside the
repo).
