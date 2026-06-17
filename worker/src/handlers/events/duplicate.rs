//! POST /api/events/{id}/duplicate — copy an event's settings into a new Draft.
//!
//! Strips on-chain escrow (forced to `None` by `create_event`), de-collides slug,
//! copies all other fields. See `.issues/055_duplicate_event.md` for design
//! (Decisions A1 + B1).

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
/// Delegates to `event_store::create_event` so KV/D1/audit/slug-dedup logic is
/// reused verbatim (DRY). The handler's only responsibilities are:
///   1. Load source event (KV first, D1 fallback — mirrors `restore_event`).
///   2. Role check (Organizer+ for this event).
///   3. Build a `CreateEventRequest` from the source, stripping escrow fields
///      and applying Decision A1 (sheet_id copy + warning) and Decision B1
///      (deposits left as-is; Draft gating handles the rest).
///   4. Emit an `EventCreated` audit entry with `source_id` metadata.
#[worker::send]
pub async fn duplicate_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(source_id): Path<String>,
    body: Option<Json<DuplicateEventRequest>>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let kv = state.events_kv.as_ref();

    tracing::info!(
        source_id = %source_id,
        staff_email = %claims.email,
        override_sheet_id = !body.new_sheet_id.trim().is_empty(),
        override_name = !body.new_name.trim().is_empty(),
        "duplicate event requested",
    );

    // ── 1. Load source event (KV first, D1 fallback) ────────────────────────
    let source = if let Some(kv_ref) = kv {
        crate::event_store::get_event(kv_ref, &source_id)
            .await
            .map_err(|e| {
                tracing::error!(source_id = %source_id, error = %e, "failed to fetch source event for duplicate");
                AppError::Internal(format!("failed to read source event: {e}"))
            })?
    } else {
        None
    };

    let source = match source {
        Some(c) => c,
        None => {
            tracing::info!(source_id = %source_id, "KV miss, trying D1 for source event");
            if let Some(ref d1) = state.d1 {
                crate::db::events::get_event(d1, &source_id)
                    .await
                    .map_err(|e| {
                        tracing::error!(source_id = %source_id, error = %e, "D1 get source event failed");
                        AppError::Internal(format!("failed to read source event from D1: {e}"))
                    })?
                    .map(|row| row.to_event_config())
                    .ok_or_else(|| AppError::NotFound(format!("event '{source_id}' not found")))?
            } else {
                return Err(AppError::NotFound(format!("event '{source_id}' not found")).into());
            }
        }
    };

    // ── 2. Role check (Organizer+ for this event) ───────────────────────────
    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&source)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can duplicate events".into(),
        )
        .into());
    }

    // ── 3. Build CreateEventRequest from source ─────────────────────────────
    //
    // Decision A1: if no `new_sheet_id` override, copy source's sheet_id and
    // emit a warning so the UI can render a yellow toast. sheet_id is a
    // required field in create_event, so we cannot leave it blank.
    //
    // Decision B1: do not touch deposit_enabled — create_event's natural
    // behavior (auto-on for in-person/hybrid) is left intact, and the duplicate
    // is Draft so no deposits can occur until the organizer activates it.
    let mut warnings: Vec<String> = Vec::new();

    let new_name = if body.new_name.trim().is_empty() {
        format!("{} (Copy)", source.name)
    } else {
        body.new_name.trim().to_string()
    };
    // Predictable suffix keeps deduplicate_slug output readable
    // (e.g. "solana-bangkok-copy" rather than "solana-bangkok-1").
    let new_slug = format!("{}-copy", source.slug);

    let sheet_id = if body.new_sheet_id.trim().is_empty() {
        warnings.push(
            "Duplicate shares source event's Sheet ID — change it before activating to avoid attendee-data collision.".to_string(),
        );
        source.sheet_id.clone()
    } else {
        body.new_sheet_id.trim().to_string()
    };

    // Escrow fields intentionally zeroed — create_event forces escrow_status=None
    // regardless, but we zero them too so the request is self-documenting and
    // survives any future loosening of create_event's escrow stripping.
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

    // ── 4. Delegate to create_event (slug dedup, KV+D1 write, validation) ───
    let new_config = crate::event_store::create_event(kv, state.d1.as_deref(), &req, &claims.email)
        .await
        .map_err(|e| {
            let err_msg = e.to_string();
            // Mirror create.rs: duplicate-slug collapse is a validation error.
            // In practice deduplicate_slug should make this rare, but if the
            // -copy/-copy-1/... namespace is exhausted we surface a 409-style.
            if err_msg.contains("already exists") {
                tracing::warn!(source_id = %source_id, error = %err_msg, "duplicate slug collision after dedup");
                AppError::Validation(err_msg)
            } else {
                tracing::error!(source_id = %source_id, error = %err_msg, "duplicate create_event failed");
                AppError::Internal(err_msg)
            }
        })?;

    // ── 5. Audit log — reuse EventCreated with metadata noting source ───────
    let audit_desc = format!("event '{}' duplicated from '{}'", new_config.name, source_id);
    let audit_meta = json!({ "source_id": source_id });

    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &new_config.id,
            crate::audit_store::create_entry_with_meta(
                &claims.email,
                crate::audit_store::AuditAction::EventCreated,
                &new_config.id,
                &audit_desc,
                audit_meta.clone(),
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
            &audit_desc,
            Some(audit_meta),
        )
        .await;
    }

    tracing::info!(
        source_id = %source_id,
        new_event_id = %new_config.id,
        new_event_name = %new_config.name,
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
