//! Courses (.plans/045 R4.4): active campaigns as courses.
//!
//! Public, cached 120 s:
//! - `GET /api/public/courses` → `[CourseSummary]`
//! - `GET /api/public/courses/{course}` → `CourseDetail` (404 if none)
//!
//! Attendee-authed: register once, then mark episodes watched. The course id
//! is checked for shape before D1, registration needs an active campaign,
//! and a watched mark needs an episode of it (in `courses::WATCHED_SQL`).
//! - `GET  /api/courses/{course}/progress` → `{ enrolled, watched: [slug] }`
//! - `POST /api/courses/{course}/enrol`
//! - `POST /api/courses/{course}/watched` `{ episode }`

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::course::is_course_id;
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
    match is_course_id(slug) {
        true => Ok(()),
        false => Err(AppError::NotFound("no such course".into())),
    }
}

#[worker::send]
pub async fn list(State(state): State<AppState>) -> Result<ApiOk<Value>, WorkerError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let courses = crate::courses::courses(d1(&state)?, now_ms)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(json!(courses)))
}

#[worker::send]
pub async fn detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<ApiOk<Value>, WorkerError> {
    course(&slug)?;
    match crate::courses::course(d1(&state)?, &slug)
        .await
        .map_err(AppError::Internal)?
    {
        Some(detail) => Ok(ApiOk::new(json!(detail))),
        None => Err(AppError::NotFound("no such course".into()).into()),
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
    let db = d1(&state)?;
    if !crate::courses::is_open(db, &slug)
        .await
        .map_err(AppError::Internal)?
    {
        return Err(AppError::NotFound("no such course".into()).into());
    }
    let email = claims.email.to_lowercase();
    crate::courses::enrol(db, &email, &slug)
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
    // Event slugs share the course id's shape; anything else never reaches D1.
    if !is_course_id(&body.episode) {
        return Err(AppError::Validation("not an episode of this course".into()).into());
    }
    let email = claims.email.to_lowercase();
    let recorded = crate::courses::mark_watched(d1(&state)?, &email, &slug, &body.episode)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(json!({ "recorded": recorded })))
}
