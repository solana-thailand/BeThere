# Handover 131 — Admin Record Slip on Behalf of Attendee (PR #31)

## 0. TL;DR

Added an admin-side path to record a THB payment slip on behalf of an attendee who cannot upload themselves (JWT expired, browser bug, slip sent via LINE/email). Closes the operational gap surfaced in the prior deposit-bug session (PRs #28–#30): even after the 401 CTA and OAuth return-URL threading fixed the *attendee* path, there was still no recovery for an attendee who simply could not sign in again before the event. The organizer had a confirmed payment in hand and no way to record it.

**PR:** [#31](https://github.com/solana-thailand/BeThere/pull/31)
**Branch:** `feature/admin_record_slip` → `develop`
**Commit:** `fc6245e` — `feat(deposit): admin can record THB slip on behalf of attendee`
**Diff:** 10 files, +1149 / −14

---

## 1. What Happened

### The user request

> "I can't deposit as an attendee myself; I want to bypass their status."

Translated: an admin/organizer wants to record a deposit on behalf of an attendee who is stuck. Investigation confirmed this was previously impossible — `POST /api/deposit/thb/upload` enforces VULN-012 (`claims.email == attendee.email`), with no admin override.

### The decision that drove the design

Three scoping decisions were made up-front with the user, all accepted:

1. **`auto_verify` defaults to `true`.** The use case is "attendee sent me the slip via LINE, I trust it, record + verify in one step". Flipping the default would silently turn every admin submit into a two-step record-then-manually-verify flow.
2. **Reject duplicates (no admin override).** Allowing overwrite would risk double-counting the deposit counter (`increment_deposit_counter_with_fallback`) and create financial audit drift. If a stale half-row exists, the admin must clear it through existing paths first.
3. **Deposits-tab button + modal.** Matches where admins already are when triaging stuck deposits. The per-row deep-link from the Attendees list was initially deferred, then added in the same PR (commit `15478ea`) after the core flow was stable — see §2.

### Why an admin path is safe even though it bypasses VULN-012

VULN-012 exists because the attendee endpoint is gated only by `require_identity` (any verified JWT passes). Without the email match, any signed-in user could impersonate any attendee.

The admin path runs on the **protected** router:
- `require_auth` middleware → staff allowlist check
- `resolve_event_with_access` → per-event organizer gate (`check_event_access`)

Two layers of authorization + audit log with the admin's email as actor. The VULN-012 threat model (random signed-in user impersonating another attendee) does not apply because random signed-in users cannot reach this endpoint.

---

## 2. Changes

### Backend (worker) — 6 files

#### `worker/src/handlers/deposit/thb/handlers/slip_admin_upload.rs` (new, 668 lines)

Handler `admin_upload_thb_slip_handler` mounted at `POST /api/deposit/thb/admin-upload`. Sibling of `upload_thb_slip_handler` + `verify_thb_slip_handler`:

- **Auth:** `require_auth` (staff) + `resolve_event_with_access` (organizer gate). Same pattern as `admin_hold_deposit_handler`.
- **Skips VULN-012** email-match check — admin is acting on attendee's behalf.
- **`auto_verify: bool` (default `true`)** via `serde(default = "default_auto_verify")`.
- **Reuses** the attendee-side logic verbatim:
  - `validate_slip_url` (MIME/size/SVG checks)
  - deposit_enabled / deposit_amount_thb validation
  - bank info required check
  - duplicate rejection (`existing.is_some()`)
  - deadline/reclaim flow (switches attendee back to In-Person if capacity available)
  - `maybe_upload_to_r2` for slip image
  - `save_thb_deposit` + `save_deposit_status`
  - `increment_deposit_counter_with_fallback`
  - Google Sheet bg_sync (`write_bank_info`, `write_deposit_verification`, `update_qr_urls`)
- **When `auto_verify=true`**, also fires the verify side effects (mirrors `verify_thb_slip_handler`):
  - Sets `verified_by = claims.email`, `verified_at = now`
  - QR auto-generation (D1 inline write + Sheet bg_sync)
  - D1 dual-write via `db::attendees::verify_deposit`
- **Audit:** emits `AuditAction::SlipRecordedByAdmin` with admin email as actor and a note indicating whether auto-verify was applied.

#### `worker/src/audit_store.rs`

New enum variant:
```rust
/// Admin recorded a THB payment slip on behalf of an attendee who could
/// not upload themselves (e.g. JWT expired and they sent the slip via
/// LINE/email). Skips the VULN-012 email-match gate (admin-authed +
/// audited instead). Sibling of `DepositSubmitted` / `DepositVerified`.
SlipRecordedByAdmin,
```
Serializes as `slip_recorded_by_admin` via the existing `#[serde(rename_all = "snake_case")]` on the enum.

#### Re-exports
- `worker/src/handlers/deposit/thb/handlers/mod.rs` — `mod slip_admin_upload;` + `pub use slip_admin_upload::admin_upload_thb_slip_handler;`
- `worker/src/handlers/deposit/thb/mod.rs` — added to the `pub use handlers::{...}` list
- `worker/src/handlers/deposit/mod.rs` — added to the `pub use thb::{...}` list + module doc

#### `worker/src/handlers/mod.rs`

Route wired next to `/deposit/thb/verify` on the protected router:
```rust
.route(
    "/deposit/thb/admin-upload",
    post(deposit::admin_upload_thb_slip_handler),
)
```

### Frontend (frontend-leptos) — 4 files

#### `frontend-leptos/src/pages/admin_deposit_record_slip.rs` (new)

`AdminRecordSlipModal` component. Separate file because `admin_deposit.rs` was already at 1046 lines (above the 1024-line rule).

- Props: `show`, `set_show`, `event_id`, `set_toast`, `on_success`, `pending_attendee_id` + setter (reactive deep-link trigger — when a parent sets it to `Some(id)`, the modal opens itself pre-filled and clears the signal)
- Form fields: attendee_id, slip image picker (reuses `js_interop::read_file_as_data_url`), bank_account, bank_name, account_name, `auto_verify` checkbox (default on)
- Client-side validation mirrors server checks (early return + toast on failure)
- Calls `api::admin_upload_thb_slip(&body)`; on success, closes modal + fires `on_success` (parent refresh)
- Slip preview reuses `LightboxImage` for tap-to-enlarge
- No bank-name autocomplete dropdown (admins can type codes directly — skipping the dropdown avoids duplicating `THAI_BANKS` across modules)

**Deep-link trigger design** (added in commit `15478ea`): the modal watches a `pending_attendee_id: ReadSignal<Option<String>>`. When a parent sets it to `Some(id)`, an Effect inside the modal sets `attendee_id=id`, opens itself via `set_show(true)`, and clears the signal back to `None`. Replaces the original one-shot `initial_attendee_id: Option<String>` prop (which only fired on mount). Pattern: parent owns the signal, modal only reads + clears it.

#### `frontend-leptos/src/pages/admin_deposit.rs`

- Import `AdminRecordSlipModal`
- New signal `(show_record_slip_modal, set_show_record_slip_modal)`
- Modal mounted at the top of the event-selected view (position:fixed inside the modal makes DOM placement irrelevant)
- Deposits tab header rewritten to flex layout with the new "🎫 Record Slip for Attendee" button

#### `frontend-leptos/src/pages/mod.rs`

Module registration: `pub mod admin_deposit_record_slip;`

#### `frontend-leptos/src/api/deposit.rs`

- `AdminSlipUploadRequest` struct (mirrors `ThbSlipUploadRequest` + `auto_verify: bool`)
- `Default` impl (auto_verify defaults to `true`)
- `api::admin_upload_thb_slip(&body)` → `api_post_json("/deposit/thb/admin-upload", body)`

---

## 3. Plan / Code / Test

### Verification

```
Worker tests:
  cargo test -p event-checkin-worker --quiet
  → 165 unit tests pass (157 prior + 8 new)
  → 89 integration tests pass

Frontend tests:
  cargo test --quiet (in frontend-leptos/)
  → 92 unit tests pass (no regressions)

Clippy:
  cargo clippy -p event-checkin-worker --quiet       → clean
  cargo clippy --quiet (in frontend-leptos/)          → clean on new files

Security linter (program_autofixer):
  Ran on slip_admin_upload.rs → 0 issues
```

### What the 8 new unit tests cover

All in `slip_admin_upload.rs` under `#[cfg(test)] mod tests`, focused on the `AdminSlipUploadRequest` deserialization contract:

1. `default_auto_verify_is_true` — guards the UX-critical default
2. `deserializes_with_auto_verify_defaulted_when_omitted` — serde default path
3. `deserializes_with_auto_verify_false_when_explicit` — opt-out path
4. `deserializes_with_optional_bank_fields_absent` — empty bank info deserializes (handler returns 400, not 422 parse error)
5. `deserializes_https_slip_url` — long R2 paths accepted without truncation
6. `rejects_malformed_json` — broken JSON surfaces as serde error, not panic
7. `rejects_missing_required_event_id` — required field enforcement
8. `rejects_missing_required_attendee_id` — required field enforcement

### Why no full handler integration tests

The worker crate has no pattern for mocking `AppState`'s KV/D1/Sheets/R2 surfaces — every existing test in the crate is either:
- Pure-function unit tests (`crypto.rs`, `auth.rs` static paths, `deterministic_monetary_code.rs` source-scan), or
- Tests that build a real `AppState` with `None` for all external services (only useful for testing logic that gracefully degrades)

Standing up a full mock harness for `event_store`, `sheets::bg_sync`, `db::attendees`, and `r2` just to exercise this handler would be a separate infra-epic. The handler's correctness rests on reusing `slip_upload::validate_slip_url`, `event_store`, and `sheets::bg_sync` — all of which have their own coverage.

### Live verification (still pending — production smoke)

After PR #31 merges and deploys, the user should:
1. Close ALL browser tabs of `bethere.solana-thailand.workers.dev` (let SW activate the new bundle)
2. As admin, open `/admin-deposit`
3. Pick an event with `deposit_enabled=true`
4. Click "🎫 Record Slip for Attendee" on the Deposits tab
5. Paste an `attendee_id` from the Attendees list, upload a slip, fill bank info
6. Submit with Auto-verify ON → verify the recorded deposit appears in Refund Queue (not Deposits pending list)
7. Open the attendee's ticket page → verify the QR is present
8. Check `/admin?event=...` audit feed → verify `slip_recorded_by_admin` entry with admin email

---

## 4. Reflection / Struggles / Solved

### Solved — branch naming collision

Initial attempt `git checkout -b develop/feature/admin_record_slip` failed because git cannot create `refs/heads/develop/feature/admin_record_slip` when `refs/heads/develop` already exists (nested ref conflict). Switched to flat `feature/admin_record_slip`. The user's gitflow rule mentions `develop/feature/01_plan_name` but the existing project branches all use the flat form (`feat/login_return_url_redirect`, `fix/admin_event_selector_leaked_scroll_listener`) — matched the existing convention.

### Solved — Leptos FnOnce vs Fn capture

First compile of `AdminRecordSlipModal` failed:
```
error[E0525]: expected a closure that implements the `Fn` trait,
but this closure only implements `FnOnce`
```
Root cause: `handle_submit` captured `on_success: impl Fn() + Clone + 'static` by move, making the closure FnOnce. But the Leptos `view!` body re-runs as `Fn` (for reactivity), so handlers passed to `on:click` must be `Fn`.

Fix: wrapped `on_success` in `StoredValue::new(on_success)` (which IS Copy). The captured environment became all-Copy, so the closure became Copy/Fn. The handler clones the inner closure out via `.with_value(|f| f.clone())` on each click for `spawn_local`.

Initial attempt used deprecated `store_value(...)` — clippy flagged it, switched to `StoredValue::new(...)`.

### Solved — clippy lints in new modal

- `unnecessary_map_or` → `slip.as_ref().is_none_or(|s| s.is_empty())`
- `clone_on_copy` on `NodeRef` → removed `.clone()` (NodeRef is Copy)

### Solved — file-size rule for admin_deposit.rs

`admin_deposit.rs` was already 1046 lines (above the 1024-line project rule). Adding the modal inline would have made it worse. Created `admin_deposit_record_slip.rs` as a sibling module — keeps each file focused and under the limit.

### Struggle — duplicate-handling decision

Considered three options for the duplicate case:
- (a) Reject (mirror attendee endpoint)
- (b) Overwrite (admin override)
- (c) Idempotent upsert (overwrite if unverified, reject if verified)

Went with (a) for safety. (b) and (c) would have required audit-drift recovery logic on the deposit counter — if a row gets overwritten, the counter has already been incremented once, and incrementing again would double-count, while skipping the increment would under-count. (a) sidesteps the whole question. If a stale row blocks an admin, the cleanup path is to delete the broken row directly in D1/KV, which is rare and auditable.

### Struggle — scope of admin power

Considered whether the admin should also be able to:
- Override the deposit deadline (currently: reclaim flow runs same as attendee)
- Override the duplicate check (currently: rejected)
- Skip bank info (currently: required, mirrors attendee)

Decided **no** on all three. The admin path mirrors the attendee path's business invariants exactly — it only skips the *authentication* gate (email match), not the *validation* gates. This keeps the financial invariants uniform across paths.

---

## 5. Remain Work

### Blocking — none

The feature is functionally complete (core handler + modal + Attendees-list deep-link), all tests pass, PR is open. PR #31 now contains 3 commits: core feature, handover doc, deep-link follow-up.

### Optional follow-ups (deferred, non-blocking)

1. ~~**Deep-link from Attendees list.**~~ **DONE in commit `15478ea`** (same PR). Added a per-attendee "Record Slip" button on the Attendees list (visible only when the current event has `deposit_enabled`). Clicking it switches to the Deposits section and opens the modal pre-filled with the attendee's ID. The modal was refactored to watch a reactive `pending_attendee_id` signal (replacing the one-shot `initial_attendee_id` option) so any trigger source can open it. New `current_deposit_enabled` Memo gates button visibility per event.

2. **Release cut to `main`.** Once PR #31 merges, `develop` will be ~20 commits ahead of `main`. Optional release cut to bring `main` current.

3. **Production smoke test.** See step 3 above — needs the user to manually verify after deploy.

4. **Test harness for handler integration.** The worker crate has no `MockState` pattern for KV/D1/Sheets/R2. Standing one up is a separate infra-epic; would unlock integration tests for this handler and many others.

5. **"`extract_event_id_from_url()` robustness"** (carryover from handover 129/130) — still uses `.unwrap()` on `web_sys::window()` and `location().href()`. Defensive `?` replacement is non-blocking but worth doing.

---

## 6. Issues Ref

- **PR #31** — feat(deposit): admin can record THB slip on behalf of attendee (this work)
- **PR #28** — fix(deposit): surface sign-in CTA on THB slip upload 401 (prior session, made the attendee path recoverable)
- **PR #29** — fix(admin): remove leaked scroll listener on unmount (prior session, unrelated crash fix)
- **PR #30** — feat(login): thread next param through OAuth (prior session, deep-link return after sign-in)
- **VULN-012** — original email-match mitigation that this admin path intentionally bypasses (under stronger auth)
- **Issue #061** — THB hold-deposit saga; Phase 2 added the admin `admin_hold_deposit_handler` which is the architectural template for this PR

No `.issues/` entry created for this work — it's a feature, not a bug, and the user did not request one.

---

## 7. How to Dev / Test

### Local dev

```sh
# Backend (worker)
cargo check -p event-checkin-worker
cargo test -p event-checkin-worker --quiet
cargo clippy -p event-checkin-worker --quiet

# Frontend
cd frontend-leptos
cargo check
cargo test --quiet
cargo clippy --quiet
```

### Running the new handler locally

The handler is wired into the protected router — `wrangler dev` (or whatever local worker runtime the project uses) will expose it at `http://localhost:8787/api/deposit/thb/admin-upload`. Requires a valid staff JWT in the Authorization header (or `dev-token` if `DEV_MODE=1`).

Example request:
```json
POST /api/deposit/thb/admin-upload
Authorization: Bearer <staff-jwt>
Content-Type: application/json

{
  "event_id": "evt_xxx",
  "attendee_id": "att_yyy",
  "slip_url": "data:image/png;base64,iVBORw0KGgo=",
  "bank_account": "123-4-56789-0",
  "bank_name": "KBank",
  "account_name": "Somchai",
  "auto_verify": true
}
```

Response (200):
```json
{
  "success": true,
  "data": {
    "success": true,
    "verified": true,
    "message": "slip recorded and verified"
  }
}
```

### Key files to reference

- **Backend handler:** `worker/src/handlers/deposit/thb/handlers/slip_admin_upload.rs`
- **Auth template:** `worker/src/handlers/deposit/thb/handlers/hold_admin.rs` (`admin_hold_deposit_handler` — same pattern)
- **Attendee endpoint (the one being bypassed):** `worker/src/handlers/deposit/thb/handlers/slip_upload.rs`
- **Verify side effects (mirrored when auto_verify=true):** `worker/src/handlers/deposit/thb/handlers/slip_verify.rs`
- **Route wiring:** `worker/src/handlers/mod.rs` (next to `/deposit/thb/verify`)
- **Frontend modal:** `frontend-leptos/src/pages/admin_deposit_record_slip.rs`
- **Frontend trigger:** `frontend-leptos/src/pages/admin_deposit.rs` (Deposits tab header)
- **API client:** `frontend-leptos/src/api/deposit.rs` (`admin_upload_thb_slip`, `AdminSlipUploadRequest`)

### Key invariants to preserve on future edits

1. **Never relax the auth on `/deposit/thb/admin-upload`.** It MUST stay on the protected router (`require_auth` + `resolve_event_with_access`). The VULN-012 bypass is only safe because of these two layers.
2. **Always emit `AuditAction::SlipRecordedByAdmin`** when this handler succeeds — it's the audit trail that replaces the email-match check.
3. **`auto_verify` default MUST stay `true`.** The modal UX is built around one-step record+verify. Flipping the default silently would force every admin into a two-step flow.
4. **Reject duplicates.** Don't add an "overwrite" path without solving the deposit-counter drift problem first.
5. **Bank info stays required.** The refund pipeline depends on it; the admin path mirrors the attendee path here.
6. **Test the deserialization contract** (`AdminSlipUploadRequest`) when changing the request shape — the 8 unit tests guard the serde surface.
