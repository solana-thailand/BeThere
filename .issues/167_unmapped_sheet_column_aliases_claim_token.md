# 167: A sheet column with no header borrows the standard-layout slot, and on a Luma sheet that slot is claim_token

**Status:** deployed to prod `f02143d4` (2026-09-29, `deploy/production/20260929T050434Z`, session `event-checkin-1b`; staging runs the same tree as `bb8ac906`). Was: fixed on develop (part A, 2026-09-29, session `event-checkin-5f`). Part B is still open. The staging repro is not done; see "Staging" below. Reported in the 2026-09-29 UI/UX review handoff (Task 1).

## What happens

A Google account registers on an event whose sheet is shared with a
duplicated event. The duplicate-registration path then returns a
`claim_token` equal to the attendee's contact handle (`@reviewtester`), not
a UUID. The ticket API serves that value, so the `/claim/{token}` link is
broken.

## Cause (part A: column aliasing)

`ColumnMapping::get_or_default` fell back to the hardcoded 33-column layout
for **any** key the header row lacked, including keys on sheets whose header
row it did recognise. The dev sheet is a Luma export. I read its header row
read-only with the service account (`/tmp/ec-5f/sheet_hdr.mjs`):

- `claim_token` is at **L** (index 11), and again at R (17).
- There is no `contact_handle` header; the Luma column is titled
  `Contact Handle / โปรดระบุ Username…`, which no candidate matches.
- The standard layout puts `contact_handle` at **L** (index 11).

The self-registration writers therefore did this:

1. They wrote the claim token to L.
2. They wrote the handle to L as well, overwriting the claim token.

The row reader then read L back as the claim token. The dev sheet has one
data row with an `@handle` in L.

The same fallback put other unmapped keys into organizers' columns on this
sheet:

| Key | Standard slot | Luma column overwritten |
|---|---|---|
| `contact_channel` | K | `qr_code_url` |
| `deposit_agreed` | M | `amount_tax` |
| `deposit_method` | N | `amount_discount` |
| `deposit_amount` | O | `currency` |
| `deposit_verified` | Q | `bethere_link` |
| `refund_link` | AC | "Who did you receive this invite from?" |
| `consent_given` | AE | the Luma Contact Handle column |
| `consent_marketing` | AG | `payment_status` |

The single-cell writers (`column_letter`) and the D1→sheet backfill planner
had the same fallback.

## Fix (part A)

- `ColumnMapping::resolve(key) -> Option<usize>` is now the single rule, the
  one `pii_column_letters` already followed:
  - A recognised header row is trusted as is: a key with no header has no
    column.
  - Only an unrecognised header row falls back to the standard layout.
- Every consumer goes through `resolve`, and `get_or_default` is removed:
  - `ColumnMapping::put`, used by the row writers: registration append (bg
    and blocking) and the walk-in append.
  - `AttendeeRow::from_sheet_values`: an unmapped key reads as empty.
  - `column_letter` now returns `Option`. The new helper `sheets::a1::cell`
    returns `None` for a missing column, so all 47 single-cell writes in
    `bg_sync.rs`, `write/checkin.rs`, `write/deposit.rs` and
    `write/append.rs` skip the cell instead of writing into a neighbour.
  - `batch_update_sheet` returns early on an empty batch.
  - `plan_backfill` counts `column_missing` and skips. The count is exposed
    in the backfill response.
- Side fix: `write_deposit_verification` built its ranges from the raw
  `ctx.sheet_name` instead of `a1::sheet_ref`, so a tab name with a space
  failed. The new helper always uses `sheet_ref`.
- Regression test: `domain/tests/unmapped_column_aliasing.rs` (5 tests) uses
  the real Luma header row.
  - With `resolve` switched back to the old fallback, 4 of the 5 fail.
  - The first of those fails with `left: "@reviewtester"`, the reported
    symptom.

Consequence: on a Luma-layout sheet, BeThere no longer mirrors the handle,
channel, deposit or consent columns. D1 stays the source of truth. An
organizer who wants those columns in the sheet adds the standard headers
(`contact_handle`, `deposit_method`, …).

## Part B (open): the dedup falls back to a shared sheet across events

`registration_attendees` → `get_attendees_for_event` reads D1 first. When D1
has **no rows for the event**, it falls back to the whole sheet. A
duplicated event copies the source's `sheet_id` (Decision A1 in
`handlers/events/duplicate.rs`), and a sheet has no event column. So on a
freshly duplicated event, the first registrant who also registered for the
source event matches their **source-event** row. The response then carries
the source event's `api_id` / `claim_token` together with the new
`event_id`. This is the same class of bug as `.issues/153` (attendee id is
global).

Not fixed here. It needs a product decision on what an empty-D1 event
should read:

- skip the sheet fallback for events created in BeThere, or
- scope the sheet rows by registration date after the event's creation.

## Staging

Not reproduced on staging. The fix is not on staging, and this session was
told not to deploy, so a staging run could only reproduce the old bug, not
verify the fix. A repro would also append rows to whatever sheet the staging
events use. The local evidence stands in for it:

- the real header row;
- the corrupted row count;
- the negative-control test run.

To reproduce on staging:

1. Create an event whose sheet has the Luma header row.
2. Register with a contact handle.
3. `GET /api/public/ticket/{id}?event_id=…` and read `claim_token`.

## Part B decision (owner, 2026-09-29)

Skip the sheet fallback for events created in BeThere: an empty D1 roster
reads as empty, never as another event's sheet rows. Legacy sheet-only
events keep the fallback. Implementation is still open.

Plan: D1 has been the authoritative attendee store since `fa0dca12`
(2026-08-12, "make D1 attendee write authoritative + fail-closed"). For an
event whose `created_at` is on or after that date, an empty D1 roster is the
truth, and the sheet fallback can only return another event's rows. Gate the
fallback in `sheets::get_attendees_inner` on that, as an explicit option, so
`handlers/events/sync.rs` (which exists to read the sheet) keeps reading it.
Test with two events sharing one sheet (compare `.issues/153`).
