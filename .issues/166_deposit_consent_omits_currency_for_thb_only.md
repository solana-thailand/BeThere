# 166: The deposit consent omits the currency for THB-only events

**Status:** fixed on develop (2026-09-29, session `event-checkin-31`). Staging page check still to do. Found by session `event-checkin-f6` while checking `.issues/161` on staging `53de706d`.

## What happens

On a THB-only deposit event (no USDC amount) whose escrow is not closed, the
registration consent reads:

> I agree to the Privacy Policy and authorize the 500 commitment deposit (returned after the event).

The attendee is authorizing a payment, and the text does not say in what
currency. Seen on staging `/e/slipdemo-1790494035` (headless Chrome, 390 px,
`/tmp/ec-f6/consent_161.mjs`).

## Cause

`frontend-leptos/src/pages/public_event/page.rs` builds `deposit_label`.
Its last arm, reached when `deposit_amount_usdc == 0` and escrow is open, is
`thb.to_string()`. The escrow-closed arm already renders `"{thb} Baht"`.

## Fix

Render `"{thb} Baht"` in that arm too (owner rule 2026-09-28: lead with
THB). Add a test that every `deposit_label` arm with a non-zero amount names
a currency. Then open the page on staging for a THB-only event.

## Resolution

- The label moved into `deposit_consent_label` in
  `frontend-leptos/src/utils/money.rs`, a pure function. The THB-only, escrow-open
  arm now renders `"{thb} Baht"`, and the USDC arms now say `USDC`.
- `frontend-leptos/tests/deposit_consent_names_currency.rs` covers every arm
  (4 tests; the floor was ratcheted).
- Still to do: open a THB-only event page on staging after the next staging deploy.
