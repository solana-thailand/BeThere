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

## 6. Validation (2026-09-04)

§5's first "not done" item, closed. Quoting makes any *legal* name work; it
cannot make an *illegal* one exist. Google refuses a tab name that is over 100
characters, contains `[ ] * ? / \`, or starts or ends with an apostrophe. Such
a name was accepted by the form, stored, and then failed on every Sheets call —
silently, for the same reason the quoting bug was silent: nearly every Sheets
call is detached best-effort work whose errors are logged and dropped.

The rule lives in `domain/src/models/event/sheet_name.rs::normalize_sheet_name`,
in the crate the worker and the Leptos form share, so the two cannot drift. It
trims, falls back to a default when blank, and otherwise rejects with a message
written for the organiser looking at the form field. It also exports
`DEFAULT_ATTENDEE_SHEET_NAME` / `DEFAULT_STAFF_SHEET_NAME`, which replace the
four hardcoded `"Attendees"` / `"staff"` literals in `create.rs` and
`event_form.rs`.

Design points:

- **Length is counted in characters, not bytes.** Thai tab names are ordinary
  here; a byte cap would reject a 34-character Thai name.
- **A leading or trailing `'` is rejected** rather than quoted. It is the shape
  an organiser produces by pasting the already-quoted form (`'Day 1'`) out of a
  formula; left alone it addresses a tab literally named `'Day 1'`, which cannot
  exist.
- **Control characters are rejected.** They cannot be typed into a tab title at
  all — they arrive from a paste or a scripted request — and would otherwise
  reach a Sheets URL path via the four `:append` endpoints.
- **Blank on update keeps the event's current tab**, not the default: clearing
  the field must not silently retarget a live event from `Registrations` to
  `Attendees`. Only an event that has no stored name (predating this) falls
  back to the default.
- `:` is *allowed*. Google accepts it in a tab title, and the A1 parser splits
  on `!` before the range separator, so `'a:b'!A1:B2` is fine.

Wiring: `event_store/write/create.rs` validates both names before building the
config; `event_store/write/update.rs` routes both of its near-identical copies
of the update logic (`update_event`, async and DB-backed; `apply_update`, pure)
through one new `apply_sheet_names` helper. The Leptos form checks the same rule
before submit, in the established toast style, so the organiser is told which
field is wrong without a round trip — the server 400 already surfaced as a toast,
but without naming the field.

`worker/tests/sheet_name_validation.rs` pins it: behaviour through
`apply_update`, plus two source scans — that both copies call
`apply_sheet_names`, and that neither assigns a tab name from the request
directly (the shape that was there before, and the one that gets reintroduced by
copying a neighbouring field's line). Mutation-tested by restoring the old
assignment in `apply_update`: all seven tests fail and both scans name the file
and line. Reverted after.

Six unit tests in `sheet_name.rs` cover the shipped defaults, legal-but-unusual
names (`Attendee List`, `Bob's tab`, `ผู้เข้าร่วม`, `2026`, `a:b`), the rejected
set, the character-vs-byte cap, and control characters.

Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -D
warnings`, `cargo test --workspace`, the wasm release build, plus
`frontend-leptos` clippy on `wasm32-unknown-unknown` and its own `cargo test`
(the crate is outside the workspace).

## 7. Not done

- **`worker/src/state.rs` still hardcodes** `"Attendees"` / `"staff"` /
  `"Contacts"` / `"Events"` as env-var fallbacks. Deliberately left: those are
  platform deployment config, not organiser input, and two of the four names
  have no constant to share.
- ~~`update.rs` holds two near-identical copies of the whole update path.~~
  Done, same session — see §8.
- **Existing events are not migrated.** An event stored with an illegal tab name
  before this keeps it; validation only fires on write. No such event is known
  to exist, and a migration would have to guess a replacement name.
- **Not verified against the live API.** The quoting rule is implemented from
  Google's A1 grammar and unit-tested; no request with a spaced tab name has
  been made against a real spreadsheet. That needs an owner with a sheet.
- Like `.plans/020` §6 and §7, this fix is unpushed and therefore still live in
  production.

## 8. The two copies of the update path, collapsed (2026-09-04)

Wiring the tab-name validation surfaced why `update.rs` was 570 lines:
`update_event` (async, DB-backed) and `apply_update` (pure) each carried a full,
independent copy of the field-application body — 236 lines, applied in the same
order, including the SEC-002 escrow field lock and the SEC-003 deposit cap.
`escrow_transition_contract.rs` was written *because* of that duplication and
pinned the escrow allowlist at exactly 2 occurrences.

Normalising for comments and `&mut`, the two bodies differed in exactly one
place: `apply_update` applied `req.dev_profile_enabled` and `update_event` did
not. Latent rather than live — the three `update_event` callers (escrow status
confirmation, poster upload, poster delete) all leave that field `None` — but it
is precisely the drift the duplication invites, and it had already happened
unnoticed.

`update_event` now loads the config, checks `expected_updated_at`, calls
`apply_update`, stamps `updated_at`/`updated_by`, and persists. 570 lines → 343.

`escrow_transition_contract.rs` Layer 2 had to be inverted: its three source
scans asserted "exactly 2×, once per copy", which the collapse turns red. They
now assert exactly 1×, so a *reintroduced* second copy fails the guard — the
property worth pinning now. Layer 1 (the exhaustive 25-case transition matrix on
`apply_update`) is unchanged and still passes. The self-test that simulated
removing one occurrence now expects a count of 0 rather than 1.
