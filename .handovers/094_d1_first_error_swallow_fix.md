# Handover #094: D1 `.first::<T>()` Error Swallowing Fix

## What Happened

Audited and fixed all 10 D1 `.first::<T>(None).await.ok().flatten()` calls across 4 database modules. The `.ok()` silently converted deserialization errors (column mismatch, type mismatch) into `None`, making them indistinguishable from "row not found" — the same pattern that caused the D1 delete bug. Replaced with `.map_err(|e| format!("... query: {e:?}"))?` to properly propagate errors.

## Root Cause

The `worker` crate's `.first::<T>(None)` returns `Result<Option<T>, JsError>`. Using `.ok().flatten()` collapsed both `Err(deserialization_error)` and `Ok(None)` into `None`. A legitimate "row not found" and a critical "struct column mismatch" were treated identically.

## Changes Made

### Files Modified (4 files)

| File | Functions Fixed |
|------|----------------|
| `worker/src/db/attendees.rs` | `get_deposit_status_from_d1`, `get_attendee_by_id`, `get_attendee_by_claim_token` |
| `worker/src/db/claim_locks.rs` | `get_claim_lock` |
| `worker/src/db/developers.rs` | `get_developer_profile`, `developer_count`, `outreach_opt_in_count` |
| `worker/src/db/events.rs` | `get_event`, `get_active_event`, `get_event_by_slug` |

### Already Correct (not modified — 9 functions)

| File | Functions |
|------|-----------|
| `worker/src/db/attendees.rs` | `count_deposits_by_event`, `count_by_status`, `count_in_person_attendees`, `check_walkin_duplicate`, `count_walkin_attendees` |
| `worker/src/db/quiz.rs` | `get_quiz_config_from_d1`, `get_quiz_progress_from_d1` |
| `worker/src/db/escrow_index.rs` | `get_event_id_by_escrow_from_d1` |
| `worker/src/handlers/health.rs` | `check_d1_health` |

### Pattern Change

```rust
// BEFORE: swallows deserialization errors as None
.first::<T>(None)
.await
.ok()    // Err → None, Ok(None) → None — indistinguishable
.flatten()

// AFTER: propagates errors, returns Ok(None) for row-not-found
.first::<T>(None)
.await
.map_err(|e| format!("D1 {function} query: {e:?}"))?
```

### Clippy Fix (2 additional simplifications)

- `claim_locks.rs` — removed needless `Ok(...?)` wrapper → direct return
- `developers.rs` — removed needless `Ok(...?)` wrapper → direct return

## Build & Test

| Check | Result |
|-------|--------|
| `cargo clippy -p worker --quiet` | 0 errors, 0 warnings |
| `cargo test -p event-checkin-domain` | 72/73 pass (1 pre-existing failure: `test_last_column_letter_hardcoded`) |

## Call Site Impact

All callers use one of two patterns:
1. **`let row = func(...).await?`** — errors now propagate instead of silently becoming `None` ✅
2. **`if let Ok(Some(x)) = func(...).await`** — errors cause fallthrough to next path (graceful fallback) — same behavior as before but now errors are distinguishable from empty results in logs

No call sites needed modification.

## Remaining Items

| Priority | Item | Description |
|----------|------|-------------|
| 1 | **Deploy frontend** | Commits `e03f526`→`9493b62` need `trunk build --release` + upload |
| 2 | **#049 Phase 3** | Campaigns & Series — campaigns table, progress tracking, certificates |
| 3 | **#049 Phase 4** | Organizer Community Dashboard — skills charts, interest heatmap |
| 4 | **#050 DO deployment** | Retry when CF API recovers from error 10013 |
| 5 | **SVG parsing warnings** | Console noise from QR/badge rendering |
