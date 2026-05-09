//! Walk-in attendee registration handler.
//!
//! Staff can register walk-in attendees who show up without pre-registering.
//! Records are stored in KV only (no Google Sheets) with a reverse mapping
//! for claim token lookup.

use axum::{Extension, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::ApiOk;
use crate::state::AppState;

use super::ext::resolve_event_with_access;

// ---------------------------------------------------------------------------
// TTL constants
// ---------------------------------------------------------------------------

/// TTL for walk-in records — 90 days in seconds.
const WALKIN_TTL_SECS: u64 = 86400 * 90;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct WalkinRegisterRequest {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WalkinRegisterResponse {
    pub claim_token: String,
    pub claim_url: String,
    pub attendee: WalkinAttendee,
}

// ---------------------------------------------------------------------------
// KV key helpers
// ---------------------------------------------------------------------------

/// KV key for walk-in attendee record.
/// Pattern: `walkin:{event_id}:{email_lower}`
fn walkin_key(event_id: &str, email_lower: &str) -> String {
    format!("walkin:{event_id}:{email_lower}")
}

/// KV key for reverse mapping (claim token → event + email).
/// Pattern: `claim_walkin:{claim_token}`
fn claim_walkin_key(claim_token: &str) -> String {
    format!("claim_walkin:{claim_token}")
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// POST /api/walkin/register — register a walk-in attendee on-the-spot.
///
/// Flow:
/// 1. Validate input (name, email, phone)
/// 2. Check for duplicate in KV
/// 3. Generate claim token (UUID v7)
/// 4. Write walk-in record + reverse mapping to KV with 90-day TTL
/// 5. Return claim URL and attendee data
#[worker::send]
pub async fn register_walkin(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::Json(body): axum::Json<WalkinRegisterRequest>,
) -> Result<ApiOk<WalkinRegisterResponse>, crate::error::WorkerError> {
    // 1. Validate input
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation("name is required (max 100 chars)".to_string()).into());
    }

    let email = body.email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::Validation(
            "email is required and must be a valid address".to_string(),
        )
        .into());
    }

    if let Some(ref phone) = body.phone
        && phone.trim().len() > 20
    {
        return Err(AppError::Validation("phone must be at most 20 characters".to_string()).into());
    }

    // Resolve event and check access
    let event = resolve_event_with_access(&state, &claims, Some(&body.event_id)).await?;

    let email_lower = email.to_lowercase();
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        tracing::error!("EVENTS KV namespace not configured");
        AppError::Internal("EVENTS KV namespace not configured".to_string())
    })?;

    // 2. Check for duplicate
    let wkey = walkin_key(&event.id, &email_lower);
    let existing: Option<String> = kv.get(&wkey).text().await.map_err(|e| {
        tracing::error!(key = %wkey, error = ?e, "failed to check walkin duplicate");
        AppError::Internal(format!("KV read failed: {e:?}"))
    })?;

    if existing.is_some() {
        tracing::warn!(
            event_id = %event.id,
            email = %email_lower,
            "walk-in duplicate: already registered"
        );
        return Err(AppError::Validation(
            "a walk-in attendee with this email is already registered for this event".to_string(),
        )
        .into());
    }

    // 3. Generate claim token
    let claim_token = Uuid::now_v7().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // 4. Build walk-in attendee
    let phone_clean = body
        .phone
        .as_ref()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    let attendee = WalkinAttendee {
        event_id: event.id.clone(),
        name: name.to_string(),
        email: email_lower.clone(),
        phone: phone_clean,
        claim_token: claim_token.clone(),
        checked_in_at: now,
        checked_in_by: claims.email.clone(),
        wallet_address: None,
        claimed_at: None,
    };

    // 5. Write walk-in record to KV
    let json = serde_json::to_string(&attendee).map_err(|e| {
        tracing::error!(error = %e, "failed to serialize walkin attendee");
        AppError::Internal(format!("serialization failed: {e}"))
    })?;

    kv.put(&wkey, &json)
        .map_err(|e| {
            tracing::error!(key = %wkey, error = ?e, "failed to build walkin put");
            AppError::Internal(format!("KV put failed: {e:?}"))
        })?
        .expiration_ttl(WALKIN_TTL_SECS)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(key = %wkey, error = ?e, "failed to write walkin record");
            AppError::Internal(format!("KV write failed: {e:?}"))
        })?;

    // 6. Write reverse mapping: claim_walkin:{token} → {event_id}:{email_lower}
    let reverse_value = format!("{}:{}", event.id, email_lower);
    let rkey = claim_walkin_key(&claim_token);
    kv.put(&rkey, &reverse_value)
        .map_err(|e| {
            tracing::error!(key = %rkey, error = ?e, "failed to build reverse mapping put");
            AppError::Internal(format!("KV put failed: {e:?}"))
        })?
        .expiration_ttl(WALKIN_TTL_SECS)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(key = %rkey, error = ?e, "failed to write reverse mapping");
            AppError::Internal(format!("KV write failed: {e:?}"))
        })?;

    // 7. Build claim URL
    let claim_url = format!("{}/{}", state.config.server.claim_base_url, claim_token);

    tracing::info!(
        event_id = %event.id,
        email = %email_lower,
        name = %name,
        staff = %claims.email,
        claim_token = %claim_token,
        "walk-in registered"
    );

    Ok(ApiOk::new(WalkinRegisterResponse {
        claim_token,
        claim_url,
        attendee,
    }))
}
