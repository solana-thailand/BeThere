//! Public event endpoint — returns sanitized event details by slug.
//! No authentication required. Only Active/Completed events are visible.

use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::ApiOk;
use crate::state::AppState;
use event_checkin_domain::models::error::AppError;

/// `GET /api/public/event/{slug}`
///
/// Returns publicly visible event details for a given slug.
/// Draft and Archived events return 404.
/// Sensitive fields (sheet_id, organizer_wallet, etc.) are excluded.
#[worker::send]
pub async fn get_public_event(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<ApiOk<Value>, crate::error::WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    let config = crate::event_store::get_event_config(kv, &slug)
        .await
        .map_err(AppError::Internal)?;

    let config = match config {
        Some(c) => c,
        None => {
            return Err(AppError::NotFound(format!("event '{slug}' not found")).into());
        }
    };

    // Only show Active or Completed events publicly
    match config.status {
        event_checkin_domain::models::event::EventStatus::Draft
        | event_checkin_domain::models::event::EventStatus::Archived => {
            return Err(AppError::NotFound(format!("event '{slug}' not found")).into());
        }
        _ => {}
    }

    tracing::info!(slug = %slug, "public event fetched");

    // Return sanitized response — exclude all sensitive/internal fields
    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "tagline": config.tagline,
        "link": config.link,
        "status": config.status.as_str(),
        "event_start_ms": config.event_start_ms,
        "event_end_ms": config.event_end_ms,
        "deposit_enabled": config.deposit_enabled,
        "deposit_amount_usdc": config.deposit_amount_usdc,
        "deposit_amount_thb": config.deposit_amount_thb,
        "nft_image_url": config.nft_image_url,
        "nft_name_template": config.nft_name_template,
        "nft_symbol": config.nft_symbol,
        "nft_description_template": config.nft_description_template,
        "quiz_enabled": config.quiz_enabled,
        "refund_deadline_hours": config.refund_deadline_hours,
        "description": config.description,
        "location": config.location,
        "created_at": config.created_at,
    })))
}
