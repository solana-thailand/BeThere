# Issue 052: Codebase Refactoring — Files Over 1024 Lines

## Summary

21 source files exceed the 1024-line guideline. Split them into focused submodules while preserving the existing public API via re-exports.

## Scope

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
- [~] Phase 4: Hard backend — **`event_store/write.rs` ✅ done** (1430 → `write/` dir, 10 submodules
  all <600 lines, re-export-only `mod.rs`; verbatim relocation, 254 worker tests pass, clippy clean);
  `register.rs` (1678) still pending.
- [ ] Phase 3: Frontend splits (landing, quiz_editor)
- [ ] Phase 5: Hard frontend (scanner, claim, event_form, admin)

> **Note (2026-07-28):** the file inventory above is stale — the tree now has ~21 files >1024 lines
> (several new since this issue, e.g. `handlers/deposit/usdc/mod.rs`, `db/attendees.rs`,
> `frontend/api/event.rs`, `campaigns_page.rs`, `adventure/*`). **Recommended order: backend before
> frontend** — the worker has 254 tests so splits are safe to verify, whereas the frontend has ~0 native
> tests (`#[wasm_bindgen_test]` only), making its refactors higher-risk. Next backend targets: `register.rs`,
> `handlers/deposit/usdc/mod.rs`, `db/attendees.rs`.
