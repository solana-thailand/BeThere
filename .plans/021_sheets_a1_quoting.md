# 021 — Google Sheets A1 ranges did not quote the sheet name

Status: **fixed on `feature/sql_binding_phase2`, unpushed — still live in production.**
Date: 2026-09-04

## 1. The bug

Every Sheets read and write in the worker addresses cells with an A1 range built
by interpolation:

```rust
let range = format!("{sheet_name}!A2:R");
```

Google's A1 grammar accepts a bare sheet name only when it is a plain
identifier. Anything else — a space, a hyphen, an apostrophe, a leading digit,
or a name that reads as a cell reference — must be single-quoted, with interior
quotes doubled: `'Attendee List'!A2:R`.

The name is not ours. `sheet_name` and `staff_sheet_name` are free-text inputs
on the event form (`frontend-leptos/src/pages/event_form.rs`), defaulting to
`"Attendees"` / `"staff"` and only `.trim()`ed — no validation, client or
server. An organiser who named their tab `Attendee List`, `Day 1` or `Walk-ins`
produced `Attendee List!A2:R`, which the API rejects with `INVALID_ARGUMENT`.

It failed **silently**. Nearly every Sheets call in the worker is detached
best-effort work (`ctx.wait_until()`, "D1 is the source of truth; Sheets is a
legacy mirror"), so the 400 was logged and dropped. From the organiser's side
the tab simply never filled in: no check-in timestamps, no claim tokens, no
deposit rows, no appended attendees.

Scope: 66 range sites across `sheets/mod.rs`, `sheets/bg_sync.rs`,
`sheets/contacts.rs`, `sheets/events_tab.rs`, `sheets/write/{append,checkin,
deposit}.rs`, and `handlers/waitlist.rs`.

## 2. The fix

New `worker/src/sheets/a1.rs` — `sheet_ref(name: &str) -> String`, the single
place the quoting decision is made. Bare when the name is `[A-Za-z0-9_]`, does
not start with a digit, and is not cell-shaped; otherwise `'…'` with `'` → `''`.

Bare is preserved wherever it is legal rather than quoting unconditionally,
because the API echoes ranges back in responses and unconditional quoting would
change the shape of every existing event's ranges.

Cell-shape detection is **bounded** — one to three letters then one to seven
digits — because Sheets addresses at most column `ZZZ`. An unbounded
"letters-then-digits" rule would quote `Sheet1`, Google's own default tab name,
and `Attendees2`. Both stay bare; `A1`, `AB12`, `Z999` and `R1C1` are quoted,
since `A1!B2` resolves against a *cell*, not a tab.

The empty name is quoted rather than left bare: bare would yield `!A2:R`, which
Sheets reads as "the first tab" — a silent write to the wrong sheet. `''!A2:R`
is rejected loudly instead.

Call sites bind `let sheet_ref = a1::sheet_ref(sheet_name);` and interpolate
`{sheet_ref}`. The binding sits **after** any early return
(`get_column_mapping`'s KV cache hit, `get_attendees_inner`'s D1-first path,
`get_staff_members`'s staff cache, `fetch_sheet_range_with_retry`'s two
fallbacks, `write_batch_updates`'s empty-ranges guard) so the common path does
not allocate.

The raw name is still used, deliberately, for the three things that want it: KV
cache keys, gid resolution, and comparison against a title returned by the API.

### 2.1 Four `:append` URLs were a second half of the same bug

`sheets/write/append.rs` (×2), `sheets/events_tab.rs` and `sheets/contacts.rs`
build the append endpoint by URL-encoding the sheet name into the path directly
rather than encoding an assembled `range`:

```rust
".../values/{}!A:R:append?…", urlencoding::encode(sheet_name)
```

Fixing only the `ValueRange.range` field would have left these still broken, and
the body's `range` is validated against the URL's. They now encode `sheet_ref`.

### 2.2 `handlers/waitlist.rs`

Its three ranges use the literal `"waitlist"`, which needs no quoting — and one
of them is interpolated into a URL with no encoding at all, so quoting it would
have introduced raw `'` into a URL path. The literal was hoisted to
`const WAITLIST_SHEET` and pinned by a unit test asserting
`sheet_ref(WAITLIST_SHEET) == WAITLIST_SHEET`, so renaming it to something like
`"waiting list"` fails the build rather than the API.

## 3. The regression gate

`worker/tests/a1_range_guard.rs`:

- `a1_ranges_do_not_interpolate_a_raw_sheet_name` — scans every `.rs` under
  `worker/src` for `{sheet_name}!` / `{staff_sheet_name}!` outside a comment.
  Textual, because the pattern that reintroduces the bug is textual and gets
  copied off a neighbouring line.
- `the_sheets_module_actually_uses_sheet_ref` — asserts at least five files
  under `src/sheets` still call `a1::sheet_ref`. Without it the first test would
  also pass if the quoting were routed around or the Sheets code deleted.

Mutation-tested: reverting one range in `sheets/write/checkin.rs` to
`{sheet_name}!` fails the guard with the file, line and offending line text.
Reverted after.

`a1.rs` carries seven unit tests, including one pinning the shipped defaults
(`Attendees`, `staff`, `waitlist`) as bare.

## 4. Gates

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace` (all binaries green), and `cargo build -p
event-checkin-worker --target wasm32-unknown-unknown --release`.

## 5. Not done

- **No server-side validation of `sheet_name`.** Quoting makes any name *work*;
  it does not stop an organiser from naming a tab something the rest of the
  system handles badly. Out of scope here.
- **Not verified against the live API.** The quoting rule is implemented from
  Google's A1 grammar and unit-tested; no request with a spaced tab name has
  been made against a real spreadsheet. That needs an owner with a sheet.
- Like `.plans/020` §6 and §7, this fix is unpushed and therefore still live in
  production.
