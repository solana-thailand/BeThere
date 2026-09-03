# Issue 052: Codebase Refactoring — Files Over 1024 Lines

## Summary

Split source files over the 1024-line guideline into focused submodules while preserving the
existing public API via re-exports.

**Current inventory (re-measured 2026-09-04, repo-wide `fd -e rs -E target -E backups`): 8 files remain,
all in `frontend-leptos`, all blocked on the same structural limit — see "Phase 5 remainder" below.
No backend file exceeds 1024 lines.** The original 21-file scope below is kept as the historical record;
every file it names has since been split or renamed.

## Scope (original, 2026-07 — historical; all entries resolved)

### Backend (worker)

| File | Lines | Split Into | Difficulty |
|------|-------|------------|------------|
| `durable_objects/event_do.rs` | 1284 | 6 files (types, lifecycle, claim_lock, checkin, sync, tests) | Easy |
| `handlers/deposit/thb/handlers.rs` | 1485 | 5 files (slip_upload, slip_verify, slip_list, refund, hold_credit) | Medium |
| `event_store/write.rs` | 1358 | 6 files (index, escrow, mutations, lifecycle, seed, deposit) | Medium |
| `handlers/events.rs` | 1349 | 7 files (list, seed, create, read, update, lifecycle, audit) | Medium |
| `handlers/register.rs` | 1262 | 5 files (types, register, my_registration, helpers, developer_data) | Hard |
| `sheets/write.rs` | 1034 | 4 files (checkin, append, deposit, mod.rs) | Easy |
| `handlers/attendee.rs` | 844 | 4 files (list, read, helpers, delete) | Low priority |

### Frontend (frontend-leptos)

| File | Lines | Split Into | Difficulty |
|------|-------|------------|------------|
| `pages/scanner.rs` | 2108 | 8 files (types, js_interop, camera, escrow, walkin, check_in, state, mod) | Hard |
| `pages/claim.rs` | 1991 | 8 files (types, js_interop, helpers, components, quiz, claim_flow, face_grid, mod) | Hard |
| `pages/event_form.rs` | 1887 | 10 files (types, helpers, constructors, sections/*, mod) | Hard |
| `pages/admin.rs` | 1828 | 8 files (types, csv_export, event_selector, attendee_list, stats, qr, recent, mod) | Hard |
| `pages/landing.rs` | 1244 | 6 files (auth, waitlist, upcoming_events, my_registrations, sections, mod) | Medium |
| `pages/quiz_editor.rs` | 1236 | 5 files (helpers, question_form, config_panel, preview, mod) | Medium |

### Domain

| File | Lines | Note |
|------|-------|------|
| `models/event.rs` | 1307 | Review — may be acceptable for a central model |
| `models/attendee.rs` | 1239 | Review — ColumnMapping could be extracted |

## Execution Order (by ease × impact)

1. ✅ `event_do.rs` — cleanest boundaries, already grouped by impl blocks
2. ✅ `sheets/write.rs` — independent functions, no shared state
3. ✅ `events.rs` (1349→8 files) + `thb/handlers.rs` (1485→5 files + mod.rs)
4. Backend write.rs (event_store)
5. Frontend landing.rs, quiz_editor.rs (medium difficulty)
6. Frontend scanner.rs, claim.rs, event_form.rs, admin.rs (hard — Leptos view! macros)

## Rules

- Each new `mod.rs` only re-exports — no business logic
- All existing public API preserved via `pub use`
- No call-site changes required
- Each sub-file < 600 lines
- Run `cargo check` + `cargo test` after each file split

## Status

- [x] Phase 1: Easy backend splits (event_do, sheets/write)
- [x] Phase 2: Medium backend splits (events→8 files, thb/handlers→5 files)
- [~] Phase 4: Hard backend — **`event_store/write.rs` ✅** (1430 → `write/`, 10 submodules) and
  **`handlers/register.rs` ✅** (1678 → `register/`, 8 submodules incl. relocated `#[cfg(test)]`;
  `DeveloperData` widened to `pub(super)` as a cross-submodule necessity; `register.rs`→`signup.rs`
  to avoid `module_inception`). Both verbatim, all 254 worker tests pass, clippy clean under `-D warnings`.
  Also **`handlers/attendee.rs` ✅** (1303 → `attendee/`, 7 submodules: list/read/delete/participation/admin
  + tests; `normalize_override`→`pub(super)`; `get_attendee`+`get_public_ticket` kept together as they share
  `get_cached_qr_image`). Verbatim, clippy clean under `-D warnings`, 254 tests pass.
  Also **`db/attendees.rs` ✅** (1470 → `attendees/`, 6 submodules: writes/deposit/reads/walkin/management;
  all 58 external `attendees::` call sites preserved via `pub(crate) use` globs; `D1AttendeeRow` widened to
  `pub(super)` for one cross-submodule reuse; no SQL/bind changes). Verbatim, clippy clean under `-D warnings`,
  254 tests pass.
  Also **`handlers/deposit/usdc/mod.rs` ✅** (1813 → 7 submodules: types/gating/rpc/discovery/recover/confirm
  + tests, alongside the pre-existing `handlers.rs`; `mod.rs` is now 32 lines of re-exports only).
  Split **verbatim** — every code line is byte-identical to the original, proven by diffing the concatenated
  submodules against the pre-split file: the only differences are the removed section banners and the
  `mod tests { … }` wrapper. Deliberately **not** rustfmt'd: the pre-split file was already 141 lines of
  rustfmt drift (as is much of the repo), so reformatting would have buried a money-critical split in
  unrelated churn. `DISCOVERY_COOLDOWN_SECS` widened to `pub(super)` as the one cross-submodule necessity;
  the test module imports its subjects from the owning submodules rather than a `super::*` glob.
  Clippy clean under `-D warnings`, 481 workspace tests pass.

  Also **`domain/models/event.rs` ✅** (1609 → `event/`, 6 submodules: enums/config/requests/responses/form
  + `defaults` + tests) and **`domain/models/attendee.rs` ✅** (1405 → `attendee/`, 5 submodules:
  status/core/columns/row/walkin + tests; `index_to_column_letter` widened to `pub(super)` because a test
  exercises it directly).
  Also **`worker/claim/mint.rs` ✅** (1118 → `mint/`, 7 submodules: types/helpers/lookup/quest/execute/walkin
  + tests; `resolve_event_id_from_token`, `crossmint_image_url`, `orb_nft_url`,
  `verify_online_quest_completion` and `execute_walkin_claim` widened to `pub(super)`) and
  **`worker/db/campaigns.rs` ✅** (1105 → `campaigns/`, 8 submodules: types/crud/events/progress/stats/
  checkin/series + tests; `totals_sql` widened to `pub(super)`).
  All four splits **verbatim** — proven by diffing the concatenated submodules against the pre-split file;
  the only differences are removed section banners, the `mod tests { … }` wrapper, and the listed
  visibility widenings. Clippy clean under `-D warnings`, 481 workspace tests pass.

  **No backend file over 1024 lines remains** (re-verified 2026-09-04: the largest are
  `handlers/deposit/usdc/handlers.rs` at 1021, `bethere-escrow/src/tests/close.rs` at 976,
  `handlers/campaigns.rs` at 970 and `db/event_summaries.rs` at 968).
  **Watch `handlers/deposit/usdc/handlers.rs` — 3 lines from breaching the guideline.**
- [x] Phase 3: Frontend splits — **`pages/landing.rs` ✅** (1255 → `landing/`: auth/waitlist/upcoming/
  registrations/page) and **`pages/quiz_editor.rs` ✅** (1240 → `quiz_editor/`: helpers/editor/preview).
- [~] Phase 5: Hard frontend — done so far, all **verbatim** (concatenated submodules diffed against the
  pre-split file; the only differences are removed section banners, `pub(super)` visibility widenings, and
  import re-qualification forced by the extra module level):
  - **`api/event.rs` ✅** 1406 → `event/`: enums/types/crud/summary/recap/pr_pack/post_event (largest 421).
    `super::types::CommunityLink` → `crate::api::types::CommunityLink` (same type, deeper module).
  - **`pages/claim.rs` ✅** 2060 → `claim/`: interop/state/helpers/stepper/widgets/quiz_helpers/quiz_views/page.
  - **`pages/scanner.rs` ✅** 2109 → `scanner/`: interop/state/logic/views/page.
  - **`pages/adventure/levels.rs` ✅** 1197 → `levels/`: basics/advanced/registry/config (byte-identical).
  - **`pages/deposit/handlers.rs` ✅** 1060 → `handlers/`: wallet/send/polling/qr/slip/refund/close.
  - **`pages/escrow_init.rs` ✅** 1183 → `escrow_init/`: wallet/state/panel (largest 967).

  Verified with the real CI gates: `cargo clippy --workspace --locked --all-targets -- -D warnings`,
  481 workspace tests, `cargo build -p event-checkin-worker --target wasm32-unknown-unknown --release`,
  and `cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings` in `frontend-leptos`.

### Phase 5 remainder — blocked on a structural limit, not on effort

Every file still over 1024 lines is **one Leptos component whose single `view!` macro is the bulk**.
A contiguous, verbatim move cannot split a `view!` block, so these cannot be brought under the guideline
by the mechanical technique used above:

| File | Lines | Component starts | `view!` starts | Residual after pulling out all non-component code |
|---|---|---|---|---|
| `pages/event_form.rs` | 2499 | 358 | 1002 | ~2142 |
| `pages/admin.rs` | 2317 | 195 | 929 | ~2123 |
| `pages/campaigns_page.rs` | 1845 | 214 | 868 | ~1632 |
| `pages/adventure/page.rs` | 1433 | 26 | 607 | ~1408 |
| `pages/scanner/page.rs` | 1303 | 18 | 831 | ~1286 |
| `pages/admin_deposit.rs` | 1170 | 40 | 432 | ~1131 |
| `pages/claim/page.rs` | 1072 | 27 | 307 | ~1046 |

Line counts re-measured 2026-09-04; every one has grown since the 2026-07 measurement (`event_form.rs`
+208, `admin.rs` +37), so the residuals are floors, not targets. `adventure/tests/playtest.rs` (1052) is a
test file and is left alone. `quiz_editor/editor.rs` was listed here at 1013 and is now under the line.

Closing these requires **extracting real sub-components with props** — a behaviour-affecting refactor that
changes reactivity boundaries, not a move. The frontend has ~0 native tests (`#[wasm_bindgen_test]` only),
so `-D warnings` clippy is the only automated check; correctness would have to be confirmed in a browser.
(That test claim is **false** — see the Follow-up below; the browser-verification point still stands.)
**Recommend treating that as its own owner-gated task rather than folding it into this issue.**

> **Note (2026-07-28, superseded 2026-09-04):** this warned that the Scope inventory was stale and
> recommended "backend before frontend" because the worker had 254 tests while the frontend had "~0".
> Both halves are now obsolete: the backend queue is empty (no backend file over 1024 lines), and the
> "~0 frontend tests" claim was wrong — see the Follow-up below.

---

## Follow-up (2026-09-04) — a split regression the gates never caught

The claim above that "the frontend has ~0 native tests (`#[wasm_bindgen_test]` only)" is **wrong**.
`frontend-leptos` carries **181 native `#[test]` cases** (167 when this was written; re-counted
2026-09-04, plus 6 ignored), including the SSOT mirror audit in
`frontend-leptos/tests/ssot_mirror_audit.rs`. They were never run because:

- `frontend-leptos` is excluded from the root cargo workspace, so `cargo test --workspace` skips it;
- the `frontend-clippy` CI job only ran `cargo clippy`, which compiles but does not execute tests.

Consequence: the Phase 5 splits **broke four of those tests** and nothing noticed. The audit pins
`MIRROR_FILES` / `DOMAIN_PREDICATE_PATHS` to literal `.rs` paths, and `domain/src/models/attendee.rs`,
`domain/src/models/event.rs` and `frontend-leptos/src/api/event.rs` had all become module directories.

Fixed on this branch:

| Commit | Change |
|---|---|
| `5dfb874` | audit resolves a configured entry as either a `.rs` file or a module directory (recursive, sorted), and the constants point at the split directories |
| `6bd2d77` | cleared two native-target clippy errors in `pages/adventure/tests/playtest.rs` (`module_inception`, `collapsible_if`) that `--all-targets` had never seen |
| `d103421` | `frontend-clippy` job now also runs `cargo test --locked` on the host target |
| `069b97b`, `b2f6019` | repo-wide `cargo fmt` (workspace + frontend), deliberately deferred during the splits to preserve the verbatim property |
| `0978334` | `cargo fmt --all -- --check` gate added to the `build-test` job |

**Verification bar for any future frontend work is now higher than clippy alone:**

```
cd frontend-leptos
cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings
cargo clippy --locked --all-targets -- -D warnings   # native, catches test-only lints
cargo test --locked                                   # 181 tests, 6 ignored
```
