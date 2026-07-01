//! Organization API handlers — CRUD for organizations.
//!
//! Access policy:
//!   - Read (list) is available to any authenticated admin/organizer so they
//!     can populate org dropdowns (e.g. the campaign creation form).
//!   - All mutations (create/update/delete) and single-org detail remain
//!     SuperAdmin-only.
//!
//! Endpoints:
//!   GET    /api/orgs           — list all organizations   (admin read)
//!   POST   /api/orgs           — create organization      (super admin)
//!   GET    /api/orgs/{id}      — get organization details (super admin)
//!   PUT    /api/orgs/{id}      — update organization      (super admin)
//!   DELETE /api/orgs/{id}      — delete organization      (super admin)

use axum::{
    Extension,
    extract::{Path, State},
    response::Json,
};
use serde::Serialize;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::org::{CreateOrgRequest, OrganizationConfig, UpdateOrgRequest};

// ---------------------------------------------------------------------------
// GET /api/orgs — list all organizations
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct OrgListResponse {
    pub orgs: Vec<OrganizationConfig>,
}

#[worker::send]
pub async fn list_orgs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<OrgListResponse>, crate::error::WorkerError> {
    // Read access is intentionally open to any authenticated admin/organizer
    // (the route sits behind the admin-authed router, so `claims` proves a
    // valid staff session). Mutating endpoints below remain SuperAdmin-only.
    let db = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not available".into()))?;

    let orgs = crate::org_store::list_orgs(db)
        .await
        .map_err(AppError::Internal)?;

    tracing::debug!(
        staff_email = %claims.email,
        org_count = orgs.len(),
        "org list read"
    );

    Ok(ApiOk::new(OrgListResponse { orgs }))
}

// ---------------------------------------------------------------------------
// POST /api/orgs — create organization
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CreateOrgResponse {
    pub id: String,
    pub name: String,
}

#[worker::send]
pub async fn create_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateOrgRequest>,
) -> Result<ApiOk<CreateOrgResponse>, crate::error::WorkerError> {
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(
            AppError::Forbidden("only super admins can create organizations".into()).into(),
        );
    }

    let db = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not available".into()))?;

    let config = crate::org_store::create_org(db, &req).await.map_err(|e| {
        tracing::error!(error = %e, "failed to create org");
        AppError::Validation(e)
    })?;

    tracing::info!(
        org_id = %config.id,
        name = %config.name,
        staff_email = %claims.email,
        "org created"
    );

    Ok(ApiOk::new(CreateOrgResponse {
        id: config.id,
        name: config.name,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/orgs/{id} — get organization
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct OrgDetailResponse {
    pub org: OrganizationConfig,
}

#[worker::send]
pub async fn get_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<OrgDetailResponse>, crate::error::WorkerError> {
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can view organizations".into()).into());
    }

    let db = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not available".into()))?;

    let org = crate::org_store::get_org_config(db, &id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("organization '{id}' not found")))?;

    Ok(ApiOk::new(OrgDetailResponse { org }))
}

// ---------------------------------------------------------------------------
// PUT /api/orgs/{id} — update organization
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct UpdateOrgResponse {
    pub id: String,
    pub updated_at: String,
}

#[worker::send]
pub async fn update_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(req): Json<UpdateOrgRequest>,
) -> Result<ApiOk<UpdateOrgResponse>, crate::error::WorkerError> {
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(
            AppError::Forbidden("only super admins can update organizations".into()).into(),
        );
    }

    let db = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not available".into()))?;

    let config = crate::org_store::update_org(db, &id, &req)
        .await
        .map_err(|e| {
            tracing::error!(org_id = %id, error = %e, "failed to update org");
            AppError::Validation(e)
        })?;

    tracing::info!(
        org_id = %id,
        staff_email = %claims.email,
        "org updated"
    );

    Ok(ApiOk::new(UpdateOrgResponse {
        id: config.id,
        updated_at: config.updated_at,
    }))
}

// ---------------------------------------------------------------------------
// DELETE /api/orgs/{id} — delete organization
// ---------------------------------------------------------------------------

#[worker::send]
pub async fn delete_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(
            AppError::Forbidden("only super admins can delete organizations".into()).into(),
        );
    }

    let db = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not available".into()))?;

    crate::org_store::delete_org(db, &id).await.map_err(|e| {
        tracing::error!(org_id = %id, error = %e, "failed to delete org");
        AppError::Validation(e)
    })?;

    tracing::info!(
        org_id = %id,
        staff_email = %claims.email,
        "org deleted"
    );

    Ok(ApiOk::new(json!({
        "id": id,
        "status": "deleted",
    })))
}
