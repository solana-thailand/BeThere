//! GET / PUT /api/events/{id}/regular-rule (Super GOAT SG1b): an event's
//! opt-in regular rule, organizer-gated and audited. The answer carries how
//! many current registrants qualify. `GET ?min=N` previews a rule not yet
//! saved. `{"rule": null}` removes it.

use axum::Extension;
use axum::extract::{Path, Query, State};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::audit_store::{AuditAction, append_event_audit, create_entry_with_meta};
use crate::error::ApiOk;
use crate::state::AppState;

use super::common::{enforce_organizer, load_event};

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::record::{RegularRule, RegularRuleAdmin};

#[derive(Debug, Default, Deserialize)]
pub struct Preview {
    pub min: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PutRule {
    pub rule: Option<RegularRule>,
}

fn d1(state: &AppState) -> Result<&worker::D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 is required for regular rules".into()))
}

#[worker::send]
pub async fn get_regular_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(preview): Query<Preview>,
) -> Result<ApiOk<RegularRuleAdmin>, crate::error::WorkerError> {
    let event = load_event(&state, state.events_kv.as_ref(), &id).await?;
    enforce_organizer(&claims, &state, &event, "read the regular rule").await?;
    let db = d1(&state)?;
    let rule = crate::regular::rule(db, &event.id)
        .await
        .map_err(AppError::Internal)?;
    let min = preview
        .min
        .or(rule.map(|r| r.min_door_checkins))
        .filter(|n| *n > 0);
    let qualifying_registrants = match min {
        Some(n) => crate::regular::qualifying(db, &event.id, n)
            .await
            .map_err(AppError::Internal)?,
        None => 0,
    };
    Ok(ApiOk::new(RegularRuleAdmin {
        rule,
        qualifying_registrants,
    }))
}

#[worker::send]
pub async fn put_regular_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<PutRule>,
) -> Result<ApiOk<RegularRuleAdmin>, crate::error::WorkerError> {
    let event = load_event(&state, state.events_kv.as_ref(), &id).await?;
    enforce_organizer(&claims, &state, &event, "set the regular rule").await?;
    if let Some(problem) = req.rule.as_ref().and_then(RegularRule::problem) {
        return Err(AppError::Validation(problem.to_string()).into());
    }
    let db = d1(&state)?;
    crate::regular::save(db, &event.id, req.rule)
        .await
        .map_err(AppError::Internal)?;
    let qualifying_registrants = match req.rule {
        Some(r) => crate::regular::qualifying(db, &event.id, r.min_door_checkins)
            .await
            .map_err(AppError::Internal)?,
        None => 0,
    };

    let description = format!("event '{}' regular rule set by {}", event.id, claims.email);
    let meta = json!({ "event_id": event.id, "rule": req.rule, "at": Utc::now().to_rfc3339() });
    match state.events_kv.as_ref() {
        Some(kv) => {
            let entry = create_entry_with_meta(
                &claims.email,
                AuditAction::RegularRuleSet,
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
                AuditAction::RegularRuleSet,
                &event.id,
                &description,
                Some(meta),
            )
            .await;
        }
    }
    Ok(ApiOk::new(RegularRuleAdmin {
        rule: req.rule,
        qualifying_registrants,
    }))
}
