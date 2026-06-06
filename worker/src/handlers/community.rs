//! Community insights API (Issue #049 Phase 1).
//!
//! Provides aggregated developer profile data for organizer dashboards.
//! All endpoints require staff auth.

use axum::{Extension, extract::State};
use serde::Serialize;

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CommunityInsightsResponse {
    pub total_developers: i64,
    pub experience_distribution: Vec<LabelCount>,
    pub tech_stack_popularity: Vec<LabelCount>,
    pub role_distribution: Vec<LabelCount>,
    pub interest_distribution: Vec<LabelCount>,
    pub outreach_opt_in: i64,
}

#[derive(Serialize, Clone)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct DeveloperListResponse {
    pub developers: Vec<DeveloperSummary>,
    pub total: i64,
}

#[derive(Serialize)]
pub struct DeveloperSummary {
    pub email: String,
    pub display_name: String,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub tech_stack: String,
    pub interests: String,
    pub total_events: i64,
    pub last_active_at: String,
    pub consent_outreach: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/community/insights
///
/// Returns aggregated community insights for the organizer dashboard.
#[worker::send]
pub async fn get_community_insights(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<ApiOk<CommunityInsightsResponse>, WorkerError> {
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))?;

    let total = crate::db::developers::developer_count(d1)
        .await
        .unwrap_or(0);
    let experience = crate::db::developers::experience_distribution(d1)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    let tech_stack = crate::db::developers::tech_stack_popularity(d1, 20)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    let role_distribution = crate::db::developers::role_distribution(d1)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    let interest_distribution = crate::db::developers::interest_distribution(d1, 20)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    let outreach_opt_in = crate::db::developers::outreach_opt_in_count(d1)
        .await
        .unwrap_or(0);

    Ok(ApiOk::new(CommunityInsightsResponse {
        total_developers: total,
        experience_distribution: experience,
        tech_stack_popularity: tech_stack,
        role_distribution,
        interest_distribution,
        outreach_opt_in,
    }))
}

/// GET /api/community/developers
///
/// Returns a paginated list of developer profiles.
#[worker::send]
pub async fn list_developers(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    axum::extract::Query(params): axum::extract::Query<DeveloperListParams>,
) -> Result<ApiOk<DeveloperListResponse>, WorkerError> {
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))?;

    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let (profiles, total) = crate::db::developers::list_developers_paginated(d1, limit, offset)
        .await
        .unwrap_or((vec![], 0));

    let developers = profiles
        .into_iter()
        .map(|p| DeveloperSummary {
            email: p.email,
            display_name: p.display_name,
            experience_level: p.experience_level,
            primary_role: p.primary_role,
            tech_stack: p.tech_stack,
            interests: p.interests,
            total_events: p.total_events,
            last_active_at: p.last_active_at,
            consent_outreach: p.consent_outreach == 1,
        })
        .collect();

    Ok(ApiOk::new(DeveloperListResponse { developers, total }))
}

#[derive(serde::Deserialize)]
pub struct DeveloperListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
