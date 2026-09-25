# 151: Sheet status writes addressed row 0, and an append landed one column right

**Status:** fixed on develop 2026-09-25 (session `event-checkin-19`), not
deployed. The RTM#6 sheet's row 57 still has to be moved back by hand; the fix
only covers new appends.
**Found by:** the owner, who saw the RTM#6 sheet and the admin Deposits tab
go wrong after rows were inserted by hand. The row-0 failure turned up while
investigating it.
**Severity:** medium.
- Nothing was written to the wrong person, and the app itself was unaffected:
  admin, roster, check-in and refunds all read D1.
- But the Google Sheet, which organizers use as the record and send to
  venues, silently stopped receiving statuses.
- PDPA erasure never cleared the sheet copy of a person's details.

## What happened

### 1. Every row write from D1 addressed row 0
- Writers took `row_index` from the attendee record.
  - D1 attendees carry it as `sheet_row_index` (`db/attendees/reads.rs`,
    `unwrap_or(0)`).
  - It is **empty for every attendee D1 created**: the insert in
    `db/attendees/writes.rs` never sets it.
  - Reads are D1-first, so check-in, virtual check-in, claim, deposit
    verification, bank info, participation type, refund status/link and ticket
    QR writes all built ranges like `Attendees!R0`.
  - The API rejects those, and the background task only logged the error.
- Prod D1 on 2026-09-25: the 6 most recent events had 0 of 207 attendees with
  a row index.
- RTM#6 sheet vs D1:

  | In D1 | Count | Same people's cell in the sheet |
  |---|---|---|
  | Deposit verified | 12 | 0 (`deposit_verified`) |
  | Ticket QR issued | 22 | 0 (`qr_code_url`) |

- `POST /api/events/{id}/sync-sheet` cannot repair it: it reads attendees
  D1-first, so it writes the same empty indexes back (`handlers/events/sync.rs`).
- PDPA erasure (`handlers/privacy.rs`) skipped the sheet whenever
  `row_index` was 0, which was always.
- The batch QR endpoint (`handlers/qr.rs`) matched generated URLs back to
  attendees by row number. With every row number 0, each lookup hit the first
  attendee, and the D1 write-back gave that one person every URL.

### 2. A new registration landed one column right
- Rows the owner had added by hand at the bottom of `Attendees` had column A
  (`api_id`) blank. They were VIP names for the venue; they now live in their
  own tab.
- Google's `values:append` found "the table" starting at column B, so the next
  self-registration (row 57, 2026-09-25 04:39 UTC) was written from B: the
  api_id in `name`, the email in `ticket_name`, and so on.
- The admin Deposits tab resolves names from the sheet by api_id in column A,
  so that attendee's pending slip showed no name.
- The append response's `updatedRange` was discarded, so nothing noticed.

## Fix

- `domain::models::attendee::SheetRow` names the attendee a write is for.
  `find_row` looks the api_id up in column A and refuses a duplicated id
  rather than guess. `range_start` reads an A1 range's first cell.
- `sheets::locate::resolve_row` / `resolve_rows` read column A at write time
  (once per batch).
  - Every row writer in `bg_sync.rs` and `write/*` now takes a `SheetRow` and
    resolves it before building a range.
  - An attendee with no single matching row is skipped and logged, never
    written to a guessed row.
- `sheets::locate::append_row` appends and checks `updatedRange`.
  - If the row landed `k` columns right, it rewrites it from column A in the
    same row, with `k` blank cells to clear the shifted tail, and logs an
    error.
  - All three attendee append sites use it (self-registration, walk-in,
    background append).
- PDPA erasure resolves the row by api_id; the `row_index > 0` guard is gone.
- `generate_qr_urls` returns `(api_id, url)`, and `handlers/qr.rs` matches by
  api_id.

## Evidence

- `domain/tests/sheet_row_lookup.rs` (7):
  - lookup after rows are inserted above;
  - the header never matches;
  - a duplicate is refused;
  - column letters round-trip;
  - the RTM#6 `B57:AI57` range parses.
- `worker/tests/sheet_row_resolution_guard.rs` (4): two mutants were each
  caught (a writer that skips the lookup, QR matching by row number).
- Not exercised against a live Google Sheet before deploy.
  - The local worker's `.dev.vars` points at a real sheet.
  - A write-scoped probe token was refused in the agent session.
  - The first real check is after the deploy: the next approval or check-in on
    RTM#6 should fill its cell. Read that back with a read-only token.

## Not done

- **Backfill.** The statuses missing from the sheet (12 deposits, 22 QRs on
  RTM#6, and every earlier event) are not written back. They are still correct
  in D1. A D1 → sheet backfill is a separate change.
- **`sync-sheet` still reads D1-first**, so it cannot refresh anything. It
  matters less now that nothing addresses rows by a stored number.
- **Attendee list paging.** `handlers/attendee/list.rs` pages by `row_index`,
  which is 0 for every D1-read attendee, so paging past the first page is
  unreliable. Not touched here.
- **Hardcoded PDPA columns.** `clear_sheet_pii` hardcodes column letters
  (`B`, `C`, …) instead of the header mapping. It is correct for the current
  header only.
- **Existing shifted rows.** The fix repairs new appends only. RTM#6 row 57
  has to be moved back by hand (cut `B57:AI57`, paste at `A52`, delete the
  blank rows 53–57).
