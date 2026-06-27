# 055: Duplicate Event (Server-Side Copy to Draft)

## Status: ✅ Done — shipped on `feature/solana_mobile_demo` (commit `f376fc4`)

> **Updated 2026-06-24:** Verified against code — backend handler
> (`worker/src/handlers/events/duplicate.rs`), domain types
> (`DuplicateEventRequest`), route (`POST /events/{id}/duplicate`),
> frontend API client (`api::duplicate_event` + `DuplicateEventData`),
> and frontend button (with Decision-A1 warning toast) are all
> implemented. Decisions A1 + B1 were applied as recommended.
> The 8 handler unit tests in §Tests are **not** written — they would
> require KV/D1 mock infrastructure that does not exist in
> `dev-dependencies` (only `tokio`); that is a separate testing-infra
> project, not a #055 blocker.

## Summary

Add a "Duplicate" action to the admin Events page that creates a **new Draft event**
copying all settings from an existing event, with on-chain escrow fields stripped and
the slug de-collided. The organizer then edits the copy (sets dates, sheet, escrow)
and activates it as usual.

Server-side implementation via `POST /api/events/{id}/duplicate`. Delegates to the
existing `event_store::create_event` so KV/D1/audit/slug-dedup logic is reused
verbatim (DRY).

## Motivation

- Organizers running recurring events (e.g., monthly meetups, repeated workshop
  formats) currently re-enter ~40 fields by hand.
- Most settings (NFT templates, community links, deposit amounts, organizer roster,
  promptpay_id, refund_deadline_hours, capacity caps) are identical run-to-run.
- The only fields that *must* change per instance are dates, slug, sheet_id, and
  the on-chain escrow (which is per-PDA by definition).
- A "duplicate" button is the standard lifecycle affordance organizers expect.

## Discovery (Refines Prior Design)

Investigation of `worker/src/event_store/write.rs#L136-310` and
`worker/src/handlers/events/lifecycle.rs` surfaced facts that simplify the design
relative to the original handover-104 sketch:

1. **`event_store::create_event` already strips escrow.** It unconditionally sets
   `status: EventStatus::Draft` and `escrow_status: EscrowStatus::None` regardless
   of request body. So even if a duplicate passes `escrow_address`/`organizer_wallet`,
   they are **structurally ignored**. No explicit stripping needed in the handler.

2. **Slug auto-deduplicates.** `deduplicate_slug` (called inside `create_event`)
   suffixes `-1`, `-2`, … on collision by scanning KV index + D1. So we never get
   a 409. Passing `slug = "{orig}-copy"` yields predictable output like
   `solana-bangkok-copy` (or `-copy-1` on second duplicate).

3. **`sheet_id` is required and validated.** `create_event` returns
   `"google sheet_id is required"` if empty. **This blocks the naive "clear
   sheet_id on copy" option** — see §Open Decisions.

4. **`deposit_enabled` auto-on for in-person/hybrid.** `create_event` forces
   `deposit_enabled = req.deposit_enabled || req.event_format.has_in_person()`.
   So an in-person duplicate will have deposits *enabled* but no escrow — i.e.,
   `accepts_usdc_deposits()` returns `false` (escrow_status is `None`). The
   duplicate is harmless until escrow is initialized, but the organizer must
   understand this. UI should warn.

5. **`CreateEventRequest` is a near-1:1 subset of `EventConfig`** minus: `id`,
   `status`, `created_at`/`updated_at`/`updated_by`, `dev_profile_enabled`.
   A direct struct-to-struct copy via field-by-field assignment is the right
   approach (no `From` impl exists; adding one would over-couple the types
   since the conversion only makes sense in the duplicate context).

## Open Decisions (Blocks Implementation)

### Decision A — `sheet_id` behavior on copy

The summary's "Option (a) clear sheet_id" is **not directly viable** because
`create_event` requires a non-empty `sheet_id`. Three real options:

| Option | Behavior | Pro | Con |
|--------|----------|-----|-----|
| **A1** (recommended) | Copy original `sheet_id`, return it in response, **frontend shows a yellow warning toast** "This duplicate shares the original's Sheet ID — change it before activating to avoid attendee-data collision." | Reuses validation path; organizer sees the field populated and is forced to acknowledge before activation. | Two events can briefly point at one sheet; if organizer activates without editing, attendee rows intermingle (same bug-class as the orphan-event-id issue from handover 104). |
| **A2** | Reject duplicate with `400 Validation` if organizer hasn't pre-supplied a new `sheet_id` in a request body. | Prevents the collision risk entirely. | Breaks the "one-click duplicate" UX; requires a confirm dialog/modal. |
| **A3** | Handler accepts optional `new_sheet_id` in body; if absent, **falls back to original** (A1 behavior). | Forward-compatible: future UI can pass a new ID; current UI gets one-click. | Default path still has the collision risk. |

**Recommendation: A1.** Matches existing UX patterns (toast warnings on
restore/hard-delete). Add a JSON `warnings: Vec<String>` field to the response
so the frontend can render them uniformly.

### Decision B — Should deposits be auto-disabled on copy?

`create_event` forces `deposit_enabled` on for in-person/hybrid formats. A
duplicate of an in-person event will then have deposit_enabled=true but no
escrow — confusing state. Options:

- **B1** (recommended) — Leave `create_event`'s behavior intact. The duplicate is
  `Draft`, so no deposits can happen anyway. Escrow panel shows the standard
  "Initialize escrow" CTA. The natural workflow (set dates → init escrow →
  activate) makes this self-correcting.
- **B2** — Override: handler forces `deposit_enabled = false` in the request to
  `create_event`, requiring the organizer to re-enable. Safer but adds friction
  for the common "recurring event with same deposit" case.

**Recommendation: B1.** Document the behavior in the issue close-out; rely on
Draft gating.

## Proposed API

### Endpoint

```
POST /api/events/{id}/duplicate
Authorization: Bearer <jwt>
Content-Type: application/json (body optional)
```

Optional body (Decision A3 forward-compat):
```json
{
  "new_sheet_id": "optional-new-sheet-id",
  "new_name": "optional override for the copied event name"
}
```

### Response (200)

```json
{
  "id": "solana-bangkok-copy",
  "name": "Solana Bangkok 2025 (Copy)",
  "slug": "solana-bangkok-copy",
  "status": "draft",
  "source_id": "solana-bangkok-2025",
  "warnings": [
    "Duplicate shares source event's Sheet ID — change before activating to avoid attendee-data collision."
  ],
  "updated_at": "2025-01-15T12:34:56.789Z"
}
```

### Errors

| Status | Condition |
|--------|-----------|
| 401 | No/invalid JWT |
| 403 | Role < Organizer for this event |
| 404 | Source event not found in KV or D1 |
| 500 | `create_event` failed (slug dedup exhausted, D1/KV both down) |

## Implementation Plan

### 1. Domain (`domain/src/models/event.rs`)

Add request/response types near `CreateEventRequest` (~L810):

```rust
/// POST /api/events/{id}/duplicate optional body.
/// All fields optional — defaults to copying source verbatim with de-collided slug.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DuplicateEventRequest {
    /// Override the source's sheet_id. If empty, source's sheet_id is reused
    /// (with a UI warning) because sheet_id is a required field downstream.
    #[serde(default)]
    pub new_sheet_id: String,
    /// Override the auto-generated "{name} (Copy)" display name.
    #[serde(default)]
    pub new_name: String,
}

/// POST /api/events/{id}/duplicate response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateEventResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub source_id: String,
    pub warnings: Vec<String>,
    pub updated_at: String,
}
```

### 2. Worker Handler (`worker/src/handlers/events/duplicate.rs` — NEW)

Skeleton follows `restore_event` (`worker/src/handlers/events/lifecycle.rs#L158-285`)
for the role-check + KV-then-D1 read pattern, then delegates to
`event_store::create_event`:

```rust
use axum::Extension;
use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{CreateEventRequest, DuplicateEventRequest};

/// POST /api/events/{id}/duplicate — copy an event's settings into a new Draft.
///
/// Strips on-chain escrow (forced to None by create_event), de-collides slug,
/// copies all other fields. See .issues/055 for design.
#[worker::send]
pub async fn duplicate_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(source_id): Path<String>,
    body: Option<Json<DuplicateEventRequest>>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(source_id = %source_id, staff_email = %claims.email, "duplicate event requested");

    let body = body.map(|Json(b)| b).unwrap_or_default();
    let kv = state.events_kv.as_ref();

    // 1. Load source event (KV first, D1 fallback) — mirrors restore_event pattern.
    let source = /* ... read source EventConfig ... */;

    // 2. Role check: Organizer+ for this event.
    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&source)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can duplicate events".into(),
        ).into());
    }

    // 3. Build CreateEventRequest by copying fields.
    let mut warnings: Vec<String> = Vec::new();

    let new_name = if body.new_name.trim().is_empty() {
        format!("{} (Copy)", source.name)
    } else {
        body.new_name.trim().to_string()
    };
    let new_slug = format!("{}-copy", source.slug);

    let sheet_id = if body.new_sheet_id.trim().is_empty() {
        warnings.push(
            "Duplicate shares source event's Sheet ID — change before activating to avoid attendee-data collision.".to_string(),
        );
        source.sheet_id.clone()
    } else {
        body.new_sheet_id.trim().to_string()
    };

    let req = CreateEventRequest {
        name: new_name,
        slug: new_slug,
        tagline: source.tagline.clone(),
        link: source.link.clone(),
        event_start_ms: source.event_start_ms,
        event_end_ms: source.event_end_ms,
        time_tba: source.time_tba,
        sheet_id,
        sheet_name: source.sheet_name.clone(),
        staff_sheet_name: source.staff_sheet_name.clone(),
        quiz_enabled: source.quiz_enabled,
        nft_collection_mint: source.nft_collection_mint.clone(),
        nft_metadata_uri: source.nft_metadata_uri.clone(),
        nft_image_url: source.nft_image_url.clone(),
        nft_name_template: source.nft_name_template.clone(),
        nft_symbol: source.nft_symbol.clone(),
        nft_description_template: source.nft_description_template.clone(),
        merkle_tree: source.merkle_tree.clone(),
        organization_id: source.organization_id.clone(),
        organizer_emails: source.organizer_emails.clone(),
        staff_emails: source.staff_emails.clone(),
        claim_base_url: source.claim_base_url.clone(),
        // Deposit fields copied; escrow_address/organizer_wallet/on_chain_event_id
        // are intentionally zeroed — create_event forces escrow_status=None anyway.
        deposit_enabled: source.deposit_enabled,
        deposit_amount_usdc: source.deposit_amount_usdc,
        deposit_amount_thb: source.deposit_amount_thb,
        promptpay_id: source.promptpay_id.clone(),
        escrow_address: String::new(),
        organizer_wallet: String::new(),
        on_chain_event_id: 0,
        refund_deadline_hours: source.refund_deadline_hours,
        max_refundable_deposits: source.max_refundable_deposits,
        description: source.description.clone(),
        location: source.location.clone(),
        video_url: source.video_url.clone(),
        event_format: source.event_format.clone(),
        require_contact_info: source.require_contact_info,
        require_photo_consent: source.require_photo_consent,
        in_person_capacity: source.in_person_capacity,
        online_capacity: source.online_capacity,
        online_open_mode: source.online_open_mode.clone(),
        online_registration_open: source.online_registration_open,
        deposit_deadline_hours: source.deposit_deadline_hours,
        visibility: source.visibility.clone(),
        community_links: source.community_links.clone(),
        calendar_subscribe_url: source.calendar_subscribe_url.clone(),
    };

    // 4. Delegate to create_event (handles slug dedup, KV+D1 write, audit log).
    let new_config = crate::event_store::create_event(kv, state.d1.as_deref(), &req, &claims.email)
        .await
        .map_err(|e| {
            tracing::error!(source_id = %source_id, error = %e, "duplicate create_event failed");
            AppError::Internal(e.to_string())
        })?;

    // 5. Audit log — reuse EventCreated with metadata noting source.
    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &new_config.id,
            crate::audit_store::create_entry_with_meta(
                &claims.email,
                crate::audit_store::AuditAction::EventCreated,
                &new_config.id,
                &format!("event '{}' duplicated from '{source_id}'", new_config.name),
                serde_json::json!({"source_id": source_id}),
            ),
            state.d1.as_deref(),
        )
        .await;
    } else if let Some(db) = state.d1.as_deref() {
        super::audit::audit_d1_only(
            db,
            &new_config.id,
            &claims.email,
            crate::audit_store::AuditAction::EventCreated,
            &new_config.id,
            &format!("event '{}' duplicated from '{source_id}'", new_config.name),
            Some(serde_json::json!({"source_id": source_id})),
        )
        .await;
    }

    tracing::info!(
        source_id = %source_id,
        new_event_id = %new_config.id,
        staff_email = %claims.email,
        warning_count = warnings.len(),
        "event duplicated",
    );

    Ok(ApiOk::new(json!({
        "id": new_config.id,
        "name": new_config.name,
        "slug": new_config.slug,
        "status": new_config.status.as_str(),
        "source_id": source_id,
        "warnings": warnings,
        "updated_at": new_config.updated_at,
    })))
}
```

### 3. Wire Up

**`worker/src/handlers/events/mod.rs`** — add module + re-export:

```rust
pub mod duplicate;
pub use duplicate::duplicate_event;
```

Update doc comment header to list `POST /api/events/{id}/duplicate`.

**`worker/src/handlers/mod.rs`** (~L274, next to `restore`) — add route:

```rust
.route("/events/{id}/duplicate", post(events::duplicate_event))
```

### 4. Frontend API Client (`frontend-leptos/src/api/event.rs`)

Mirror `restore_event` (`frontend-leptos/src/api/event.rs#L691-710`):

```rust
/// POST /api/events/{id}/duplicate — copy event settings into a new Draft.
pub async fn duplicate_event(id: &str) -> Result<EventMutationData, ApiError> {
    let path = format!("/events/{id}/duplicate");
    let response = api_post(&path).await?;
    // ... same error-handling pattern as restore_event ...
}
```

### 5. Frontend Button (`frontend-leptos/src/pages/events_page.rs`)

Add a "Duplicate" button in the row-actions area (~L196-340), gated by
`can_manage_events`. Pattern matches the existing Edit/Restore/Delete buttons:

```rust
let dup_id = evt.id.clone();
let dup_name = ename.clone();
// ...
{if can_manage {
    let did = dup_id.clone();
    let dname = dup_name.clone();
    view! {
        <button
            class="btn btn-outline btn-sm"
            on:click=move |_| {
                let did = did.clone();
                let confirm_msg = format!("Duplicate '{dname}' into a new Draft?");
                if !web_sys::window().unwrap().confirm_with_message(&confirm_msg).unwrap_or(false) {
                    return;
                }
                leptos::task::spawn_local(async move {
                    match api::duplicate_event(&did).await {
                        Ok(data) => {
                            components::show_toast(
                                &set_toast,
                                &format!("Duplicated as '{}' (Draft)", data.name),
                                components::ToastType::Success,
                            );
                            // Optionally: auto-navigate into Edit view on the new event
                            reload();
                        }
                        Err(e) => {
                            log::error!("[events-page] duplicate failed: {e}");
                            components::show_toast(
                                &set_toast,
                                &format!("Failed to duplicate: {e}"),
                                components::ToastType::Error,
                            );
                        }
                    }
                });
            }
        >
            "Duplicate"
        </button>
    }.into_any()
} else {
    view! { <span></span> }.into_any()
}}
```

## Scope Exclusions

- **No bulk duplicate** — one event at a time.
- **No template/preset system** — out of scope; this is a 1:1 copy.
- **No automatic escrow re-initialization** — organizer must run the standard
  escrow init flow on the duplicate (intentional; the new event needs a fresh PDA).
- **No D1 schema change** — duplicate uses the existing `events` table.

## Tests

Add to `tests/` folder per project convention.

### Worker unit tests (`worker/tests/duplicate_event.rs` — NEW)

1. **happy_path** — duplicate an Active event → returns Draft with `"{name} (Copy)"`
   name and `"{slug}-copy"` slug; source event untouched.
2. **slug_collision** — duplicate an event whose `{slug}-copy` already exists →
   returns `{slug}-copy-1` (verifies `deduplicate_slug` interaction).
3. **escrow_stripped** — duplicate an event with `escrow_status: Initialized` →
   new event has `escrow_status: None` and empty `escrow_address`.
4. **sheet_id_warning** — duplicate without `new_sheet_id` → response `warnings`
   array contains the collision warning; new event's `sheet_id` equals source's.
5. **sheet_id_override** — duplicate with `new_sheet_id` → no warning; new
   event's `sheet_id` equals the override.
6. **role_forbidden** — Staff role (not Organizer) → 403.
7. **source_not_found** — non-existent `{id}` → 404.
8. **audit_logged** — after duplicate, audit log has `EventCreated` entry with
   `meta.source_id` matching the original.

### Frontend

Visual smoke: button appears for Organizer+; hidden for Staff. (Existing events
page pattern — no separate test file.)

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Organizer duplicates an event, doesn't change `sheet_id`, activates → two events write to one sheet (attendee data collision — same class as handover-104 orphan-event bug). | (1) Warning toast on duplicate. (2) Warning toast on Activate if `slug` ends in `-copy` and `sheet_id` matches another event. (3) Document in operator runbook. |
| Organizer duplicates an event to "use as a template", forgets to change dates → duplicate keeps original's past `event_end_ms` → still works (Draft is not time-gated) but confusing. | UI hint on Edit form if event_end is in the past. (Out of scope here — file as #056.) |
| `deduplicate_slug` collides with an unrelated event that happens to be named `{slug}-copy` | `deduplicate_slug` already handles this — yields `{slug}-copy-1`. No silent data loss. |
| Large events with many fields → request payload surprise | All fields come from server-side source read, not request body. Request body is just optional overrides. |

## Dependencies

- None — fully self-contained.
- Should land on its own branch off `develop`: `develop/feature/055_duplicate_event`.

## Related Issues / Handovers

- Handover 104 — identified the orphan-event-id bug class that the `sheet_id`
  collision warning mitigates.
- Issue 004 — Multi-event management (provides the lifecycle/CRUD foundation).
- Plan 005 — #19 fix (already implemented on this branch); unrelated.

## Definition of Done

- [ ] Decision A resolved (recommend A1)
- [ ] Decision B resolved (recommend B1)
- [ ] Domain types added (`DuplicateEventRequest`, `DuplicateEventResponse`)
- [ ] `duplicate.rs` handler implemented with role check + KV/D1 fallback read
- [ ] Route wired in `handlers/mod.rs`
- [ ] Frontend `api::duplicate_event` added
- [ ] Frontend "Duplicate" button on events page (gated by `can_manage_events`)
- [ ] Warning toast on duplicate (sheet_id collision)
- [ ] 8 worker unit tests passing
- [ ] `cargo check` clean across `domain`, `worker`, `frontend-leptos`
- [ ] `cargo clippy --fix --allow-dirty` clean
- [ ] Handover doc written (`.handovers/{N}_duplicate_event.md`)
