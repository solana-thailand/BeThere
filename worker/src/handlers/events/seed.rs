use axum::Extension;
use axum::extract::State;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

/// POST /api/events/seed
/// Seed the first event from global env vars.
///
/// Idempotent — if an active event already exists, returns it.
/// Requires SuperAdmin role.
#[worker::send]
pub async fn seed_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "seed event requested");

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can seed events".into()).into());
    }

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
    })?;

    let config = crate::event_store::seed_from_config(kv, &state.config, &state)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to seed event");
            AppError::Internal(e.to_string())
        })?;

    // D1 dual-write (non-fatal)
    crate::event_store::sync_event_to_d1(state.d1.as_deref(), &config).await;

    tracing::info!(
        event_id = %config.id,
        event_name = %config.name,
        staff_email = %claims.email,
        "event seeded",
    );

    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "status": config.status.as_str(),
    })))
}

/// POST /api/events/reseed-kv
/// Rebuild the KV event index and configs from D1.
/// SuperAdmin only. Idempotent — overwrites KV entries with D1 data.
#[worker::send]
pub async fn reseed_kv_from_d1(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "reseed KV from D1 requested");

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can reseed KV from D1".into()).into());
    }

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not configured".into()))?;

    let count = crate::event_store::seed_kv_from_d1(kv, d1)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to reseed KV from D1");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(count, staff_email = %claims.email, "KV reseeded from D1");

    Ok(ApiOk::new(json!({
        "synced": count,
        "message": format!("{count} events synced from D1 to KV"),
    })))
}

/// POST /api/events/migrate
/// Migrate quiz data from legacy QUIZ KV namespace to event-scoped EVENTS KV namespace.
///
/// Reads the "questions" key from QUIZ namespace and copies it to
/// "event:default:quiz:questions" in EVENTS namespace (if not already migrated).
/// SuperAdmin only.
#[worker::send]
pub async fn migrate_quiz(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "quiz migration requested");

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can migrate quiz data".into()).into());
    }

    let events_kv = state.events_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
    })?;

    let quiz_kv = state.quiz_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "quiz KV namespace not configured — add QUIZ binding in wrangler.toml".into(),
        )
    })?;

    let result = crate::event_store::migrate_quiz_to_event(events_kv, quiz_kv, "default")
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to migrate quiz data");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(
        event_id = %result.event_id,
        migrated = %result.migrated,
        staff_email = %claims.email,
        "quiz migration completed",
    );

    Ok(ApiOk::new(json!({
        "migrated": result.migrated,
        "event_id": result.event_id,
        "message": result.message,
    })))
}
