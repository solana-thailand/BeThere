# 145: Refund-proof URL accepts any scheme and is rendered as an attendee link

**Status:** open. Found 2026-09-23 while adding magic-byte checks to slip uploads (`.plans/029` §2); not fixed.
**Found by:** session `event-checkin-df`, reading every writer that shares `maybe_upload_to_r2`.
**Severity:** medium-low. Planting a link needs a staff account, but it then runs
script in an attendee's session on click.

## What happens

- `POST` mark-refund (`worker/src/handlers/deposit/thb/handlers/refund.rs:76`)
  checks only that `refund_proof_url` is non-empty. Slips go through
  `validate_slip_url`, which accepts only `data:image/*` (now magic-byte
  checked) or `http(s)://`. The refund proof gets no check at all.
- The admin UI (`frontend-leptos/src/pages/admin_deposit.rs:922`) collects it
  as free text: "Paste refund proof URL (transfer receipt)".
- The attendee's ticket page renders it verbatim as
  `<a href=url target="_blank">` in `RefundCard`
  (`frontend-leptos/src/pages/ticket/action_cards.rs:165`). The admin
  refunded list does the same (`admin_deposit.rs:1069`).
- The CSP's `script-src` includes `'unsafe-inline'`
  (`worker/src/middleware/headers.rs:41`), so a `javascript:` URL is not
  blocked by CSP.

A staff member, or anyone holding a staff session, can set a refund proof of
`javascript:…`. It runs on the bethere origin when the attendee clicks
"View Refund Receipt".

Not reproduced in a browser yet. This is from reading the code; the repro below
is the check.

## Fix (proposed)

1. Server: validate `refund_proof_url` in `mark_refund_handler`, and in any
   batch or manual path that writes the column, against the same allow-list
   as slips. Either reuse `validate_slip_url` or add a sibling that accepts
   `https://` plus image data URLs. Reject everything else with a 400.
2. Frontend: render the link only when the value starts with `https://` or
   `/api/storage/`. This is defence in depth for rows already stored.
3. Read-only prod check: count rows whose `refund_proof_url` is non-empty and
   matches neither prefix:
   `SELECT COUNT(*) FROM thb_deposits WHERE refund_proof_url <> '' AND refund_proof_url NOT LIKE 'https://%' AND refund_proof_url NOT LIKE '/api/storage/%'`.

## Repro (staging)

As staff, mark a verified THB deposit refunded with proof
`javascript:alert(document.domain)`. Then open that attendee's ticket page and
click "View Refund Receipt". Expected after the fix: a 400 at step one.
