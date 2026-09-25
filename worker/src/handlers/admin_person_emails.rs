//! Super-admin linking of one person's emails (plan 025 §6.1 / §7.4,
//! `.issues/122`).
//!
//! Self-service linking (`handlers::email_link`) proves both emails with Google
//! sign-ins, so it cannot help a person whose second email is not a Google
//! account, or who will not do it themselves before the event. These endpoints
//! let a super-admin vouch for the link instead:
//!
//!   GET  /api/admin/person-emails?email=…  — the person's linked emails
//!   POST /api/admin/person-emails/link     — link two emails (`proof = admin`)
//!   POST /api/admin/person-emails/unlink   — take one email back out
//!
//! Linking shares rolling credit across the emails (owner choice 2026-09-18),
//! so every change needs a reason, and the reason goes into the global audit
//! log with who made it. Unlinking is refused while either side has spent or
//! locked credit the other side holds (`db::person::UNLINK_SQL`).

use axum::{
    Extension, Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::json;
use worker::D1Database;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::validation::is_plausible_email;

use crate::audit_store::{self, AuditAction};
use crate::db::person::{self, LinkOutcome, LinkProof, UnlinkOutcome};
use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

/// Longest reason accepted, in characters. It is stored in the audit log.
const MAX_REASON_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
pub struct PersonEmailsQuery {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminLinkRequest {
    pub email: String,
    pub other_email: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminUnlinkRequest {
    pub email: String,
    pub reason: String,
}

async fn require_super_admin(state: &AppState, claims: &Claims) -> Result<(), AppError> {
    match crate::auth::resolve_user_role(&claims.email, state, None).await {
        crate::auth::UserRole::SuperAdmin => Ok(()),
        _ => Err(AppError::Forbidden(
            "only super admins can link or unlink a person's emails".into(),
        )),
    }
}

fn database(state: &AppState) -> Result<&D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".into()))
}

/// `field` names the request field in the error. The value itself stays out:
/// error messages are logged, and an email is an identifier.
fn email_arg(raw: &str, field: &'static str) -> Result<String, AppError> {
    let email = raw.trim().to_lowercase();
    match is_plausible_email(&email) {
        true => Ok(email),
        false => Err(AppError::Validation(format!(
            "{field} is not a valid email address"
        ))),
    }
}

fn reason_arg(raw: &str) -> Result<&str, AppError> {
    let reason = raw.trim();
    match (reason.is_empty(), reason.chars().count() > MAX_REASON_CHARS) {
        (true, _) => Err(AppError::Validation(
            "a reason is required — it is kept in the audit log".into(),
        )),
        (false, true) => Err(AppError::Validation(format!(
            "the reason is too long (at most {MAX_REASON_CHARS} characters)"
        ))),
        (false, false) => Ok(reason),
    }
}

/// Record a link change in the global audit log. Best-effort, like every other
/// audit append: the change itself has already been written.
async fn audit(
    state: &AppState,
    actor: &str,
    action: AuditAction,
    target: &str,
    description: &str,
) {
    let Some(kv) = state.events_kv.as_ref() else {
        tracing::warn!("person-email change not audited: EVENTS KV not configured");
        return;
    };
    let entry = audit_store::create_entry(actor, action, target, description);
    if let Err(e) = audit_store::append_global_audit(kv, entry, state.d1.as_deref()).await {
        tracing::warn!(error = %e, "person-email audit append failed");
    }
}

/// `GET /api/admin/person-emails?email=…` — every email of that person, or just
/// the email itself when it is not linked.
#[worker::send]
pub async fn list_person_emails(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<PersonEmailsQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    require_super_admin(&state, &claims).await?;
    let email = email_arg(&query.email, "email")?;
    let emails = person::emails_of(database(&state)?, &email)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(json!({ "email": email, "emails": emails })))
}

/// `POST /api/admin/person-emails/link` — make two emails one person.
#[worker::send]
pub async fn link_person_emails(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AdminLinkRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    require_super_admin(&state, &claims).await?;
    let email = email_arg(&body.email, "email")?;
    let other = email_arg(&body.other_email, "other_email")?;
    let reason = reason_arg(&body.reason)?;
    let db = database(&state)?;

    let status = match person::link(db, &email, &other, LinkProof::Admin)
        .await
        .map_err(AppError::Internal)?
    {
        LinkOutcome::Linked => {
            audit(
                &state,
                &claims.email,
                AuditAction::PersonEmailsLinkedByAdmin,
                &email,
                &format!("linked {email} and {other} as one person: {reason}"),
            )
            .await;
            tracing::info!(
                admin_fingerprint = %state.log_fingerprint(&claims.email),
                email_fingerprint = %state.log_fingerprint(&email),
                other_fingerprint = %state.log_fingerprint(&other),
                "person emails linked by admin"
            );
            "linked"
        }
        LinkOutcome::AlreadyLinked => "already",
        LinkOutcome::SameEmail => {
            return Err(AppError::Validation("the two emails are the same".into()).into());
        }
        LinkOutcome::Conflict => {
            return Err(AppError::Validation(
                "each email already belongs to a different person. Merging two linked \
                 people is not supported: unlink one of the emails first"
                    .into(),
            )
            .into());
        }
    };

    let emails = person::emails_of(db, &email)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(json!({ "status": status, "emails": emails })))
}

/// `POST /api/admin/person-emails/unlink` — make one email its own person
/// again. Refused while the split would leave either side owing credit.
#[worker::send]
pub async fn unlink_person_email(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AdminUnlinkRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    require_super_admin(&state, &claims).await?;
    let email = email_arg(&body.email, "email")?;
    let reason = reason_arg(&body.reason)?;
    let db = database(&state)?;

    // Read the set before the unlink, so the audit entry can name who it left.
    let before = person::emails_of(db, &email)
        .await
        .map_err(AppError::Internal)?;
    let status = match person::unlink(db, &email)
        .await
        .map_err(AppError::Internal)?
    {
        UnlinkOutcome::Unlinked => {
            let others: Vec<&str> = before
                .iter()
                .map(String::as_str)
                .filter(|e| *e != email)
                .collect();
            audit(
                &state,
                &claims.email,
                AuditAction::PersonEmailUnlinkedByAdmin,
                &email,
                &format!("unlinked {email} from {}: {reason}", others.join(", ")),
            )
            .await;
            tracing::info!(
                admin_fingerprint = %state.log_fingerprint(&claims.email),
                email_fingerprint = %state.log_fingerprint(&email),
                "person email unlinked by admin"
            );
            "unlinked"
        }
        UnlinkOutcome::NotLinked => "not_linked",
        UnlinkOutcome::CreditWouldGoNegative => {
            return Err(AppError::Validation(
                "cannot unlink yet: one of these emails has spent or locked credit that \
                 another one holds. It comes back at check-in or when the event ends; \
                 retry after that"
                    .into(),
            )
            .into());
        }
    };

    Ok(ApiOk::new(json!({ "status": status })))
}
