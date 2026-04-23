# Handover 010: Column B Name Fix — Domain Crate + Legacy Axum

**Date**: 2025-06-28
**Branch**: `develop/feature/01_workers_migration`
**Scope**: Fix attendee name display to use Google Sheet column B instead of column D

---

## What Happened

The staff and admin pages were showing names from **column D** (`display_name`, index 3) instead of **column B** (`name`, index 1) of the Google Sheet. The doc comment correctly documented `B=name`, but the `AttendeeRow::from_sheet_values()` code prioritized column D over column B.

The bug existed in **two copies** of `attendee.rs`:

1. **Domain crate** (`domain/src/models/attendee.rs`) — used by the Cloudflare Worker (the active deployment)
2. **Legacy Axum** (`src/models/attendee.rs`) — the original backend build

The first fix attempt only patched the legacy Axum copy, which had no effect because the running system is the Cloudflare Worker using the domain crate. After identifying this, the same fix was applied to the domain crate.

---

## Root Cause

In `AttendeeRow::from_sheet_values()`, the `name` field was constructed as:

```rust
// BROKEN: prefers column D (index 3), only falls back to B+C if D is empty
name: {
    let display = get(3);
    if display.is_empty() {
        format!("{} {}", get(1), get(2)).trim().to_string()
    } else {
        display
    }
},
```

Column mapping per the doc comment:
- A (idx 0) = `api_id`
- B (idx 1) = `name` ← should be the primary name source
- C (idx 2) = `last_name`
- D (idx 3) = `display_name` ← was incorrectly prioritized
- E (idx 4) = `email`

---

## Fix Applied

Both files patched identically. The `name` field now reads column B (index 1) first, falling back to column D (index 3) only if B is empty:

```rust
// FIXED: column B is the primary name source
name: {
    let col_b = get(1);
    if !col_b.is_empty() { col_b } else { get(3) }
},
```

### Files Modified

| File | Status |
|------|--------|
| `domain/src/models/attendee.rs` | Fixed (Worker uses this — the active deployment) |
| `src/models/attendee.rs` | Fixed (legacy Axum build — kept in sync) |

---

## Build & Test Status

| Check | Result |
|-------|--------|
| `cargo check -p event-checkin-domain` | ✅ Zero errors |
| `cargo test -p event-checkin-domain` | ✅ 14 passed |
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ Zero errors |
| `cargo check` (main Axum) | ✅ Zero errors |
| `cargo test` (main Axum) | ✅ 25 passed |

---

## Where Is the Plan/Code/Test

**Plan**: Bug report — staff/admin pages not showing column B names.

**Code**: Two-line change in `from_sheet_values()` in both `domain/src/models/attendee.rs` and `src/models/attendee.rs`.

**Tests**: Existing tests pass (no new tests needed — this is a data mapping fix, not logic change). No unit test directly asserts column mapping; verified manually by checking the code path.

---

## Reflection — Struggles / Solved

### Fixed the wrong file first
**Struggle**: Initial fix was applied only to `src/models/attendee.rs` (legacy Axum). User reported no change visible.

**Root cause**: The Cloudflare Worker (`worker/`) imports `AttendeeRow` from the shared **domain crate** (`event-checkin-domain`), not from the `src/` directory. The `src/models/attendee.rs` file is only used by the legacy Axum build which isn't deployed.

**Solution**: Applied the same fix to `domain/src/models/attendee.rs`. This is the copy that matters for the active Workers deployment.

### Lesson
When the project has multiple build targets sharing code, always identify which copy the **running** system uses before patching. The workspace structure is:

```
domain/          ← shared crate (used by Worker)
worker/          ← Cloudflare Worker (active deployment)
src/             ← legacy Axum backend (not deployed)
```

---

## Remain Work

1. Restart `wrangler dev` to rebuild WASM with the fixed domain crate
2. Verify in browser that staff/admin pages show column B names
3. Deploy via `npx wrangler deploy` when ready

---

## Issues Ref

- Handover 009: Workers migration — explains the workspace structure and domain crate
- Handover 008: System review and migration plan

---

## How to Dev / Test

```bash
# Verify domain crate compiles + tests pass
cargo check -p event-checkin-domain
cargo test -p event-checkin-domain

# Verify Worker WASM build
cargo check -p event-checkin-worker --target wasm32-unknown-unknown

# Restart Worker dev server to pick up the fix
cd worker
npx wrangler dev  # port 8787

# Verify in browser:
# 1. Open http://localhost:8787/admin.html
# 2. Check attendee names match column B in Google Sheet
# 3. Open http://localhost:8787/staff.html
# 4. Scan QR or manual lookup — verify name shows column B value
```
