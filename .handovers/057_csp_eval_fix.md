# Handover 057: CSP Fix — Remove All eval() Calls

**Date**: 2026-05-16
**Branch**: main
**Status**: ✅ Built, not yet deployed

---

## What Happened

Registration auto-redirect was not working. Root cause: CSP `script-src` policy blocks `eval()`, and the frontend code used `js_sys::eval()` for all navigation and localStorage operations. The browser console showed:

```
Content-Security-Policy: The page's settings blocked a JavaScript eval (script-src)
(Missing 'unsafe-eval')
```

Per handover 007, the project policy is **zero eval() calls** — no `'unsafe-eval'` directive in CSP. The fix follows the same pattern as `scanner.js`: create a proper JS module and import via `#[wasm_bindgen(module = "/js/...")]`.

## Changes Summary

| File | Change |
|------|--------|
| `frontend-leptos/js/navigation.js` | **New** — CSP-safe JS module for localStorage + navigation + clipboard |
| `frontend-leptos/src/pages/public_event.rs` | Replaced 5 eval() calls → saveProgress(), loadProgress(), navigateTo() |
| `frontend-leptos/src/pages/deposit.rs` | Replaced 1 eval() call → navigateTo() |
| `frontend-leptos/src/pages/claim.rs` | Replaced 2 eval() calls → read_clipboard_text_js() |

## JS Module: `navigation.js`

Provides 4 functions:
- `saveProgress(attendeeId, eventId, slug)` — localStorage.setItem
- `loadProgress()` — localStorage.getItem
- `navigateTo(path)` — window.location.href = path
- `readClipboardText()` — navigator.clipboard.readText (returns Promise)

## Build

| Step | Result |
|------|--------|
| `cargo check` (frontend-leptos) | ✅ Clean |
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ Clean |
| `~/.cargo/bin/trunk build --release` | ✅ 5m08s, success |

## Issues Ref

- Identified during testing of handover 056 (UX flow improvements)
- CSP eval blocking was previously solved for scanner.js (handover 007) but regression occurred when adding registration redirect in handover 056

## Next Steps

- Deploy and test redirect in browser
- Implement Google Sign-In for attendees (issue 016) for proper identity verification
