# 001 — Deposit Module Extraction & Housekeeping

**Date:** 2026-05-29
**Scope:** `frontend-leptos/src/pages/deposit/`, `frontend-leptos/js/`, `frontend-leptos/src/pages/ticket/action_cards.rs`

---

## What Happened

Two sessions of extracting the deposit page's monolithic `mod.rs` (1682 lines) into focused sub-modules, plus two small housekeeping fixes.

### Session 1: `choose_payment.rs` extraction (prior session)
- Extracted the inline `ChoosePayment` view (~464 lines) from `mod.rs` into `choose_payment.rs`
- `mod.rs`: 1682 → 1244 lines

### Session 2: `handlers.rs` extraction + housekeeping (this session)

**1. Extracted `handlers.rs` (919 lines) from `mod.rs`**
- 10 handler factory functions moved out as `make_*` closures
- `mod.rs`: 1244 → **395 lines** (well under 1024 target)
- Removed unused imports (`CloseDepositRequest`, `ConfirmDepositResponse`, `RefundTxRequest`, `ThbSlipUploadRequest`, `UsdcDepositRequest`, `ToastType`)
- Used `DepositParamsSignal` type alias from `types.rs` for the params parameter

**2. Fixed `RolloverState::Signing` dead_code warning**
- Added `#[allow(dead_code)]` on `Signing(String, String)` variant in `ticket/action_cards.rs:253`
- Fields are captured for context but only matched with `_` — appropriate suppression

**3. Renamed `js/qr_generate.js` → `js/clipboard.js`**
- All 5 Rust `wasm_bindgen` references updated (`claim.rs`, `deposit/js_interop.rs`, `scanner.rs`, `ticket/in_person_view.rs`, `ticket/online_view.rs`)
- Updated JS file header doc comment

---

## Where Is the Code

| File | Lines | Role |
|------|-------|------|
| `deposit/mod.rs` | 395 | Component entry: imports, signals, view routing |
| `deposit/handlers.rs` | 919 | 10 handler factory functions |
| `deposit/choose_payment.rs` | 511 | ChoosePayment view |
| `deposit/types.rs` | 291 | State machine, types, helpers |
| `deposit/usdc_payment.rs` | 316 | USDC flow views |
| `deposit/components.rs` | 212 | Shared UI components |
| `deposit/refund.rs` | 176 | Refund flow views |
| `deposit/js_interop.rs` | 165 | JS FFI bindings |
| `deposit/close_deposit.rs` | 156 | Close deposit views |
| `deposit/already_deposited.rs` | 148 | Already deposited views |
| `deposit/thb_payment.rs` | 46 | THB upload views |
| `js/clipboard.js` | ~100 | Clipboard + QR utilities |

---

## Reflection

### Struggled
- `leptos_router::hooks::Memo<...>` is private in Leptos 0.8 — can't spell the type directly in function signatures. Solved by using the existing `DepositParamsSignal` type alias from `types.rs`.
- Fuzzy matching in `edit_file` can't distinguish between nearly identical function signatures (3 handlers with same `params` type). Had to include unique doc comments as context.

### Solved
- The "functions returning closures" pattern (`make_*` → `impl Fn(...) + Clone + Send + Sync + 'static`) works cleanly with Leptos 0.8's reactive system. `ReadSignal<T>` and `WriteSignal<T>` are `Copy`, so closures capturing them are automatically `Clone`.
- Build is completely clean: `cargo check` → 0 errors, 0 warnings.

---

## Remaining Work

| Priority | Item | Effort | Notes |
|----------|------|--------|-------|
| 🔴 High | Browser test & deploy two-tier polling + stepper fix | ~1h | Requires manual testing |
| 🟡 Med | THB flow layout cleanup | ~2h | PromptPay slip upload UI polish |
| 🟡 Med | PDPA consent checkbox #043 Phase A | ~3h | Scaffold UI + state |
| 🟡 Med | Scanner QR decode → rqrr (Phase 3 JS→Rust) | ~3h | Security + CSP simplification |
| 🔵 Low | Backend #041 — D1 for audit reads | ~4h | Worker-side, separate scope |

---

## How to Dev/Test

```bash
# Build check (fast)
cd frontend-leptos && cargo check --quiet

# Full build
cd frontend-leptos && trunk build

# Dev server
cd frontend-leptos && trunk serve --open
```

No tests were added or modified in this session — the changes were purely structural extraction with identical runtime behavior.
