//! `PATCH /api/attendee/{id}/participation-type` — manual In-Person ⇄ Online override.

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{EventIdQuery, resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

/// Normalize a manual participation_type override into its canonical form.
///
/// Accepts both display-case (`In-Person`/`Online`) and canonical
/// (`in_person`/`online`) input — the frontend currently sends display-case.
/// Rejects `Other` (covers `walkin`, `test`, junk) so this endpoint can't be
/// used to clobber the `walkin` sentinel or set garbage.
///
/// Returns `(canonical_for_d1, display_for_sheet)`.
pub(super) fn normalize_override(raw: &str) -> Result<ParticipationType, AppError> {
    // Reject empty explicitly — the parse() default-to-in-person behavior is for
    // legacy reads, not for an explicit manual override (empty is a no-op/error).
    if raw.trim().is_empty() {
        return Err(AppError::Validation(
            "participation_type must not be empty".to_string(),
        ));
    }
    let parsed = ParticipationType::parse(raw);
    if parsed == ParticipationType::Other {
        return Err(AppError::Validation(format!(
            "participation_type must be in-person or online, got '{raw}'"
        )));
    }
    Ok(parsed)
}

/// Request body for `PATCH /attendee/:id/participation-type`.
#[derive(serde::Deserialize)]
pub(crate) struct UpdateParticipationTypeBody {
    participation_type: String,
}

/// PATCH /api/attendee/{id}/participation-type
///
/// Manually override an attendee's participation_type (In-Person ⇄ Online).
///
/// Fills the gap left by the deposit-deadline auto-switch: when an attendee
/// chose the deposit flow, never came back to the system, and later confirms
/// out-of-band (phone/Telegram/email) that they will attend online, the admin
/// can flip them here instead of editing the Google Sheet cell by hand.
///
/// Writes **both** the Google Sheet (column I, via detached `wait_until`)
/// **and** D1 — unlike the auto-switch, which only writes the Sheet. Deposit
/// records and slips are intentionally left intact; this is purely a
/// participation-mode change, not a payment-state change.
#[worker::send]
pub async fn update_participation_type(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
    axum::Json(body): axum::Json<UpdateParticipationTypeBody>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let resolved = normalize_override(body.participation_type.trim())?;
    // Canonical form for D1 storage; display form for the organizer-facing Sheet.
    let new_value = resolved.as_str().to_string();
    let new_value_display = resolved.display().to_string();

    tracing::info!(
        attendee_id = %id,
        new_participation_type = %new_value,
        staff_email = %claims.email,
        "manual participation_type override"
    );

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    // Look up the attendee to get the correct Sheet row_index (mirrors delete_attendee).
    let attendee = {
        let map = sheets::get_attendees_map(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
            .map_err(|e| AppError::Internal(format!("failed to look up attendee: {e}")))?;
        map.get(&id).cloned()
    };
    let row_index = match attendee.as_ref().map(|a| a.row_index) {
        Some(idx) => Some(idx),
        None => {
            // No sheet row — likely a walk-in or D1-only record. Not an error:
            // we still update D1 below.
            tracing::warn!(
                attendee_id = %id,
                event_id = %event.id,
                "participation_type override: attendee not found in sheet (will update D1 only if present)"
            );
            None
        }
    };

    // 1. Update the Google Sheet cell (detached via wait_until when possible).
    if let Some(row_index) = row_index {
        let mapping = sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
            .unwrap_or_else(|_| event_checkin_domain::models::attendee::ColumnMapping::hardcoded());

        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
                state.clone(),
                row_index,
                new_value_display.clone(),
                mapping,
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                kv.cloned(),
            ));
        } else {
            // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
            crate::sheets::write::update_participation_type(
                row_index,
                &new_value_display,
                &mapping,
                &state,
                &event.sheet_id,
                &event.sheet_name,
                kv,
            )
            .await
            .map_err(|e| {
                AppError::Internal(format!("failed to update sheet participation_type: {e}"))
            })?;
        }
    }

    // 2. Update D1 (primary store for walk-ins; keeps ticket page + admin list
    //    in sync with the Sheet for regular attendees too).
    let d1_updated = if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::set_participation_type(db, &id, &new_value).await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(
                    attendee_id = %id,
                    error = %e,
                    "participation_type override: D1 update failed (sheet was still updated)"
                );
                false
            }
        }
    } else {
        false
    };

    // 3. Audit log.
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry_with_meta(
                &claims.email,
                crate::audit_store::AuditAction::ParticipationTypeChanged,
                &id,
                &format!("participation_type → {new_value}"),
                serde_json::json!({
                    "old_value": attendee.as_ref().map(|a| a.participation_type.clone()).unwrap_or_default(),
                    "new_value": new_value,
                    "sheet_row_updated": row_index.is_some(),
                    "d1_updated": d1_updated,
                }),
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    // Note: there is no server-side attendee-list cache to invalidate
    // (removed in Phase 2d — the list is read directly from D1/sheet each
    // request). The client-side cache is invalidated by the frontend caller
    // after a successful PATCH.

    tracing::info!(
        attendee_id = %id,
        event_id = %event.id,
        new_participation_type = %new_value,
        sheet_row_updated = row_index.is_some(),
        d1_updated,
        "participation_type override complete"
    );

    Ok(ApiOk::new(json!({
        "attendee_id": id,
        "event_id": event.id,
        "participation_type": new_value,
        "sheet_row_updated": row_index.is_some(),
        "d1_updated": d1_updated,
    })))
}
