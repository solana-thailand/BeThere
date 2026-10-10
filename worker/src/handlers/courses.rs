//! `/api/courses/{course}/…` (.plans/045 R4.4), attendee-authed: register
//! once, then mark episodes watched. The course and episode must be in the
//! catalogue (`domain::models::catalogue`), so nothing else reaches D1.
//!
//! - `GET  /api/courses/{course}/progress` → `{ enrolled, watched: [slug] }`
//! - `POST /api/courses/{course}/enrol`
//! - `POST /api/courses/{course}/watched` `{ episode }`

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::catalogue::{Series, is_course_episode};
use event_checkin_domain::models::error::AppError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct WatchedRequest {
    pub episode: String,
}

fn course(slug: &str) -> Result<(), AppError> {
    match Series::from_course_slug(slug) {
        Some(_) => Ok(()),
        None => Err(AppError::NotFound("no such course".into())),
    }
}

fn d1(state: &AppState) -> Result<&worker::D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".into()))
}

#[worker::send]
pub async fn progress(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(slug): Path<String>,
) -> Result<ApiOk<Value>, WorkerError> {
    course(&slug)?;
    let email = claims.email.to_lowercase();
    let (enrolled, watched) = crate::courses::progress(d1(&state)?, &email, &slug)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(
        json!({ "enrolled": enrolled, "watched": watched }),
    ))
}

#[worker::send]
pub async fn enrol(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(slug): Path<String>,
) -> Result<ApiOk<Value>, WorkerError> {
    course(&slug)?;
    let email = claims.email.to_lowercase();
    crate::courses::enrol(d1(&state)?, &email, &slug)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(json!({ "enrolled": true })))
}

#[worker::send]
pub async fn watched(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(slug): Path<String>,
    Json(body): Json<WatchedRequest>,
) -> Result<ApiOk<Value>, WorkerError> {
    course(&slug)?;
    if !is_course_episode(&slug, &body.episode) {
        return Err(AppError::Validation("not an episode of this course".into()).into());
    }
    let email = claims.email.to_lowercase();
    let recorded = crate::courses::mark_watched(d1(&state)?, &email, &slug, &body.episode)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(json!({ "recorded": recorded })))
}
