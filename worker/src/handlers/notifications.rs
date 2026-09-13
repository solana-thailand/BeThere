use super::events::common::{enforce_organizer, load_event};
use crate::{
    error::{ApiOk, WorkerError},
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use event_checkin_domain::models::{auth::Claims, error::AppError};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct Page {
    before: Option<i64>,
}

fn verified_email(claims: &Claims) -> Result<&str, AppError> {
    if !claims.email_verified {
        return Err(AppError::Forbidden(
            "Sign in with Google again to access notifications.".into(),
        ));
    }
    Ok(&claims.email)
}

#[worker::send]
pub async fn my_list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(page): Query<Page>,
) -> Result<ApiOk<Value>, WorkerError> {
    let email = verified_email(&claims)?;
    let before = page.before.unwrap_or(9_007_199_254_740_991);
    if !(1..=9_007_199_254_740_991).contains(&before) {
        return Err(AppError::Validation("invalid cursor".into()).into());
    }
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 unavailable".into()))?;
    let (items, unread_count) = crate::notifications::list_for_attendee(db, email, before)
        .await
        .map_err(notification_storage_error)?;
    let next_before = if items.len() == 50 {
        items.last().map(|n| n.id)
    } else {
        None
    };
    Ok(ApiOk::new(
        json!({"items":items,"unread_count":unread_count,"next_before":next_before}),
    ))
}

#[worker::send]
pub async fn my_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<ApiOk<Value>, WorkerError> {
    let email = verified_email(&claims)?;
    if id < 1 {
        return Err(AppError::Validation("invalid notification ID".into()).into());
    }
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 unavailable".into()))?;
    if !crate::notifications::mark_read(db, email, id)
        .await
        .map_err(notification_storage_error)?
    {
        return Err(AppError::NotFound("notification".into()).into());
    }
    Ok(ApiOk::new(json!({"read":true})))
}

#[worker::send]
pub async fn my_read_all(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<Value>, WorkerError> {
    let email = verified_email(&claims)?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 unavailable".into()))?;
    crate::notifications::mark_all_read(db, email)
        .await
        .map_err(notification_storage_error)?;
    Ok(ApiOk::new(json!({"read":true})))
}
#[worker::send]
pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> Result<ApiOk<Value>, WorkerError> {
    let event = load_event(&state, state.events_kv.as_ref(), &id).await?;
    enforce_organizer(&claims, &state, &event, "view notification delivery").await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 unavailable".into()))?;
    let before = page.before.unwrap_or(9_007_199_254_740_991);
    if !(1..=9_007_199_254_740_991).contains(&before) {
        return Err(AppError::Validation("invalid cursor".into()).into());
    }
    let items = crate::notifications::list(db, &id, before)
        .await
        .map_err(notification_storage_error)?;
    let next_before = if items.len() == 50 {
        items.last().map(|n| n.id)
    } else {
        None
    };
    Ok(ApiOk::new(json!({"items":items,"next_before":next_before})))
}
#[derive(Deserialize)]
pub struct RetryRequest {
    pub notification_id: i64,
}
#[worker::send]
pub async fn retry(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<RetryRequest>,
) -> Result<ApiOk<Value>, WorkerError> {
    let event = load_event(&state, state.events_kv.as_ref(), &id).await?;
    enforce_organizer(&claims, &state, &event, "retry notification delivery").await?;
    if !(1..=9_007_199_254_740_991).contains(&body.notification_id) {
        return Err(AppError::Validation("invalid notification ID".into()).into());
    }
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 unavailable".into()))?;
    let retried = crate::notifications::retry(db, &id, body.notification_id)
        .await
        .map_err(notification_storage_error)?;
    if !retried {
        return Err(AppError::Validation("Only failed notifications for this event can be retried. Accepted or uncertain messages cannot be resent.".into()).into());
    }
    tracing::info!(event_id=%id,notification_id=body.notification_id,actor_fingerprint=%state.log_fingerprint(&claims.email),"notification retry requested");
    Ok(ApiOk::new(json!({"queued":true})))
}

fn notification_storage_error(error: String) -> AppError {
    tracing::error!(error = %error, "notification storage failed");
    AppError::Internal("Notification history is temporarily unavailable. Please try again.".into())
}
