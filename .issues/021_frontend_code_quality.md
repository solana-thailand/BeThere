# Issue 021 — Frontend Code Quality Improvements

> **Priority**: P2 (maintenance, not blocking)
> **Scope**: `frontend-leptos/src/api.rs`, `auth.rs`, `utils.rs`, `pages/public_event.rs`
> **Status**: ❌ Open
> **Source**: Code review during awesome-leptos ecosystem analysis (2026-06-12)

## Problem

Code quality issues identified in the Leptos frontend that reduce maintainability:

1. **DRY violation** — `api_post_json` and `api_put_json` are ~70 lines of near-identical code
2. **Timer leak** — `public_event.rs` countdown `set_interval` never cleaned up
3. **Repeated `web_sys` chains** — `auth.rs` has the same `window().and_then(|w| w.local_storage().ok()).flatten()` 3 times
4. **Verbose JS interop** — `utils.rs` `format_timestamp` uses 25 lines of `Reflect::set` for a JS options object
5. **God file** — `api.rs` is 2865 lines with 100+ structs, 50+ functions, built-in cache

## Plan

### Step 1: DRY `api_post_json` / `api_put_json` (~15 min)

Extract shared `api_json_with_body(method, path, body)`:

```rust
enum HttpMethod { Post, Put }

async fn api_json_with_body<T: serde::de::DeserializeOwned + Default>(
    method: HttpMethod,
    path: &str,
    body: &impl serde::Serialize,
) -> Result<T, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();
    let json_body = serde_json::to_string(body).map_err(|e| ApiError {
        message: format!("Failed to serialize request: {e}"),
        status: 0,
    })?;

    let mut req = match method {
        HttpMethod::Post => gloo::net::http::Request::post(&url),
        HttpMethod::Put => gloo::net::http::Request::put(&url),
    };
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    req = req.header("Content-Type", "application/json");

    // ... shared error handling ...
}

async fn api_post_json<T>(path: &str, body: &impl serde::Serialize) -> Result<T, ApiError>
where T: serde::de::DeserializeOwned + Default,
{
    api_json_with_body(HttpMethod::Post, path, body).await
}

async fn api_put_json<T>(path: &str, body: &impl serde::Serialize) -> Result<T, ApiError>
where T: serde::de::DeserializeOwned + Default,
{
    api_json_with_body(HttpMethod::Put, path, body).await
}
```

**Files**: `frontend-leptos/src/api.rs` L392-527
**Risk**: None — same behavior, just deduplicated

### Step 2: Fix timer leak in `public_event.rs` (~10 min)

**Bug**: `set_interval` inside `Effect::new` creates a new interval on every resource refetch without clearing the old one. Also, interval keeps firing after countdown reaches zero.

**Fix**: Store interval handle, clear on zero and on component drop:

```rust
// In the Effect, before set_interval:
let interval_handle = Rc::new(RefCell::new(None));

set_interval(
    move || {
        let now = js_sys::Date::now() as i64;
        let remaining = start_ms - now;
        if remaining <= 0 {
            set_countdown.set(String::new());
            // Clear interval when done
            *interval_handle.borrow_mut() = None;
        } else {
            set_countdown.set(format_countdown(remaining));
        }
    },
    std::time::Duration::from_secs(1),
);
```

**Files**: `frontend-leptos/src/pages/public_event.rs` L393-410
**Risk**: Low — behavioral fix only

### Step 3: Extract `local_storage()` helper in `auth.rs` (~5 min)

```rust
/// Get the browser's localStorage, if available.
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
}

pub fn get_token() -> Option<String> {
    local_storage()
        .and_then(|s| s.get_item(TOKEN_KEY).ok())
        .flatten()
}

pub fn set_token(token: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(TOKEN_KEY, token);
    }
}

pub fn clear_token() {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(TOKEN_KEY);
    }
}
```

**Files**: `frontend-leptos/src/auth.rs` L14-32
**Risk**: None — pure refactor

### Step 4: Simplify `format_timestamp` JS interop (~10 min)

Replace 6 `Reflect::set` calls with a helper:

```rust
fn js_object(pairs: &[(&str, &str)]) -> js_sys::Object {
    let obj = js_sys::Object::new();
    for (key, val) in pairs {
        let _ = js_sys::Reflect::set(
            &obj,
            &wasm_bindgen::JsValue::from_str(key),
            &wasm_bindgen::JsValue::from_str(val),
        );
    }
    obj
}

pub fn format_timestamp(iso: &str) -> String {
    if iso.is_empty() {
        return "N/A".to_string();
    }
    let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(0, 0, 0, 0, 0, 0);
    js_date.set_time(js_sys::Date::parse(iso));
    if js_date.get_time().is_nan() {
        return iso.to_string();
    }
    let opts = js_object(&[
        ("year", "numeric"),
        ("month", "short"),
        ("day", "numeric"),
        ("hour", "2-digit"),
        ("minute", "2-digit"),
    ]);
    js_date
        .to_locale_string("en-US", &opts)
        .as_string()
        .unwrap_or_else(|| iso.to_string())
}
```

**Files**: `frontend-leptos/src/utils.rs` L144-177
**Risk**: Low — same output, cleaner code

### Step 5: Split `api.rs` into domain modules (~30 min)

Current: 1 file, 2865 lines, 100+ structs, 50+ functions.

Target:
```
frontend-leptos/src/api/
  mod.rs       — public re-exports (~50 lines)
  client.rs    — api_base(), api_get/post/put/delete, error types, cache (~250 lines)
  types.rs     — ApiResponse, EventMeta, EventDetail, EventConfig (~400 lines)
  event.rs     — event CRUD, archive, restore, hard_delete (~300 lines)
  attendee.rs  — check_in, undo_check_in, walk-in, QR generation (~350 lines)
  deposit.rs   — USDC/THB deposit, refund queue, escrow TX builders (~400 lines)
  claim.rs     — claim, quiz, adventure (~400 lines)
  admin.rs     — admin-specific: audit, on-chain events, cancel, adventure config (~300 lines)
```

**Steps**:
1. Create `frontend-leptos/src/api/` directory
2. Move structs/functions by domain into separate files
3. `mod.rs` re-exports everything so `use crate::api::*` still works in all pages
4. Run `cargo check -p event-checkin-frontend --target wasm32-unknown-unknown`

**Files**: `frontend-leptos/src/api.rs` → split into 8 files
**Risk**: Medium — large structural change, but purely mechanical (no logic changes)
**Compile time benefit**: changing deposit types won't recompile adventure page

## Acceptance Criteria

- [ ] `api_post_json` and `api_put_json` share a single implementation
- [ ] `public_event.rs` countdown interval is cleaned up on unmount and on zero
- [ ] `auth.rs` has a single `local_storage()` helper, no repeated chains
- [ ] `utils.rs` `format_timestamp` uses `js_object` helper instead of 6 `Reflect::set` calls
- [ ] `api.rs` split into domain modules under `src/api/`
- [ ] `cargo check -p event-checkin-frontend --target wasm32-unknown-unknown` passes
- [ ] `cargo clippy --all-targets` passes
- [ ] Manual test: countdown timer on `/e/{slug}` still works

## Related

- `.handovers/062_leptos_ecosystem_review.md` — the review that identified these issues
- `docs/ux_roadmap.md` — Leptos Ecosystem section (library recommendations for future phases)
- `docs/research_technology_review.md` §14 — awesome-leptos analysis
