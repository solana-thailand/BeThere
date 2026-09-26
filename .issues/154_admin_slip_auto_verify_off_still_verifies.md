# 154: Unchecking "Auto-verify" on the admin record-slip form still verifies

**Status:** fixed on develop, not deployed. Found by code reading on 2026-09-27 (session `event-checkin-c6`); fixed the same day (session `event-checkin-ef`). The live repro below has not been run on staging.

## What happens

The admin "Record Slip for Attendee" modal has an **Auto-verify** checkbox
(`frontend-leptos/src/pages/admin_deposit_record_slip.rs`). When it is
unchecked, the organizer expects the deposit to be recorded as *pending
review*, and the toast says "Slip recorded (pending verification)".

The request field is serialized like this:

```rust
// frontend-leptos/src/api/deposit.rs, AdminSlipUploadRequest
#[serde(skip_serializing_if = "std::ops::Not::not")]
pub auto_verify: bool,
```

So `false` is **omitted** from the JSON. The worker reads a missing field
as `true`:

```rust
// worker/src/handlers/deposit/thb/handlers/slip_admin_upload.rs
#[serde(default = "default_auto_verify")]   // fn default_auto_verify() -> bool { true }
pub auto_verify: bool,
```

The two sides only agree when the box is checked. Unchecked, the slip is
verified and a ticket QR is issued, while the toast says the opposite.
This bug has been there since `fc6245e9`, which added the endpoint.

## Why it matters

"Record but don't verify" exists for a slip the organizer isn't sure of.
That is exactly the case where verifying it by accident issues a ticket and
a refund promise for money that may not have arrived.

## Repro

On staging, as staff:
1. Open Deposits, choose "Record slip for attendee", uncheck Auto-verify, and submit.
2. `GET /api/deposit/status/{attendee_id}?event_id=…` returns `verified: true`.
   The expected result is pending.

## Fix

Always send the field: remove the `skip_serializing_if` from `auto_verify`.
Add a frontend test that serializes `auto_verify: false` and asserts the JSON
contains `"auto_verify":false`. Keep the worker default of `true` for callers
that omit the field; `default_auto_verify_is_true` pins it.

Applied: `skip_serializing_if` removed; `frontend-leptos/tests/admin_slip_request.rs` pins `"auto_verify":false` on the wire and fails when the skip is restored (mutation-checked).
