//! "I showed up" links (Super GOAT SG4, `crate::showed`):
//!
//! - `POST   /api/me/showed/{slug}`: the person's link for an event they came
//!   to (`door_history`), made once; 403 when they did not come.
//! - `DELETE /api/me/showed/{token}`: revoke one of their own links.
//! - `GET    /api/public/showed/{token}`: the event a live link names, and
//!   nothing about the person; 404 for unknown or revoked.

use axum::Extension;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::record::{ShowedPage, is_showed_token, may_share};

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

fn d1(state: &AppState) -> Result<&worker::D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".into()))
}

#[worker::send]
pub async fn share(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(slug): Path<String>,
) -> Result<ApiOk<Value>, WorkerError> {
    let db = d1(&state)?;
    let email = claims.email.to_lowercase();
    let record = crate::record::my_record(db, &email)
        .await
        .map_err(AppError::Internal)?;
    if !may_share(&record.events, &slug) {
        return Err(AppError::Forbidden("a link is only for an event you came to".into()).into());
    }
    let event_id = crate::showed::event_id(db, &slug)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("no such event".into()))?;
    let token = crate::showed::link(db, &email, &event_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(
        json!({ "token": token, "path": format!("/showed/{token}") }),
    ))
}

#[worker::send]
pub async fn revoke(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(token): Path<String>,
) -> Result<ApiOk<Value>, WorkerError> {
    if !is_showed_token(&token) {
        return Err(AppError::NotFound("no such link".into()).into());
    }
    let revoked = crate::showed::revoke(d1(&state)?, &claims.email.to_lowercase(), &token)
        .await
        .map_err(AppError::Internal)?;
    match revoked {
        true => Ok(ApiOk::new(json!({ "revoked": true }))),
        false => Err(AppError::NotFound("no such link".into()).into()),
    }
}

#[worker::send]
pub async fn public_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<ApiOk<ShowedPage>, WorkerError> {
    if !is_showed_token(&token) {
        return Err(AppError::NotFound("no such link".into()).into());
    }
    let (event_id, mut page) = crate::showed::page(d1(&state)?, &token)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("no such link".into()))?;
    if let Some(bucket) = state.r2.as_ref() {
        page.card = crate::showed::card(bucket, &event_id).await;
    }
    Ok(ApiOk::new(page))
}
