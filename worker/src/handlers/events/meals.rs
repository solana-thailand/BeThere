//! GET / PUT /api/events/{id}/meals: how many meals the organizer ordered
//! (.plans/045 R4.11, `db/event_meals.rs`). Organizer-gated. The count only
//! feeds the public stats' aggregate (`meals`); `null` removes it.

use axum::Extension;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::audit_store::{AuditAction, append_event_audit, create_entry_with_meta};
use crate::db::event_meals::{self, MAX_MEALS};
use crate::error::ApiOk;
use crate::state::AppState;

use super::common::{enforce_organizer, load_event};

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

/// The body of both routes: the count, or `null` for none entered.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Meals {
    pub ordered: Option<u32>,
}

fn d1(state: &AppState) -> Result<&worker::D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 is required for meals ordered".into()))
}

#[worker::send]
pub async fn get_meals(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<Meals>, crate::error::WorkerError> {
    let event = load_event(&state, state.events_kv.as_ref(), &id).await?;
    enforce_organizer(&claims, &state, &event, "read meals ordered").await?;
    let ordered = event_meals::read(d1(&state)?, &event.id)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(Meals { ordered }))
}

#[worker::send]
pub async fn put_meals(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<Meals>,
) -> Result<ApiOk<Meals>, crate::error::WorkerError> {
    let event = load_event(&state, state.events_kv.as_ref(), &id).await?;
    enforce_organizer(&claims, &state, &event, "set meals ordered").await?;
    if req.ordered.is_some_and(|n| n > MAX_MEALS) {
        return Err(
            AppError::Validation(format!("meals ordered must be at most {MAX_MEALS}")).into(),
        );
    }
    // 0 is "none entered", not a count: it would add registrations to the
    // meals not ordered without a meal behind them
    let ordered = req.ordered.filter(|n| *n > 0);
    let db = d1(&state)?;
    event_meals::write(db, &event.id, ordered, &Utc::now().to_rfc3339())
        .await
        .map_err(AppError::Internal)?;

    let description = format!("event '{}' meals ordered set by {}", event.id, claims.email);
    let meta = json!({ "event_id": event.id, "ordered": ordered });
    match state.events_kv.as_ref() {
        Some(kv) => {
            let entry = create_entry_with_meta(
                &claims.email,
                AuditAction::MealsOrderedSet,
                &event.id,
                &description,
                meta,
            );
            let _ = append_event_audit(kv, &event.id, entry, Some(db)).await;
        }
        None => {
            super::audit::audit_d1_only(
                db,
                &event.id,
                &claims.email,
                AuditAction::MealsOrderedSet,
                &event.id,
                &description,
                Some(meta),
            )
            .await;
        }
    }
    Ok(ApiOk::new(Meals { ordered }))
}
