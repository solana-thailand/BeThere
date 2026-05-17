//! Walk-in attendee registration handler.
//!
//! Staff can register walk-in attendees who show up without pre-registering.
//! Records are stored in KV with a reverse mapping for claim token lookup.
//! After KV write, the record is also auto-synced to Google Sheets (best-effort).
//! The separate `/walkin/sync` endpoint can retry any failed syncs.

use axum::{
    Extension, Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::ApiOk;
use crate::state::AppState;

use super::ext::{EventIdQuery, resolve_event_with_access};

// ---------------------------------------------------------------------------
// Sheets sync helpers
// ---------------------------------------------------------------------------

/// Best-effort sync of a single walk-in row to Google Sheets.
///
/// Extracted into a standalone async fn so it can be passed to `wait_until()`
/// without borrowing the handler's stack frame.
async fn sync_walkin_to_sheet(
    state: AppState,
    attendee: WalkinAttendee,
    event_id: String,
    sheet_id: String,
    sheet_name: String,
) {
    let kv = match state.events_kv.as_ref() {
        Some(kv) => kv.clone(),
        None => {
            tracing::warn!(event_id = %event_id, "sync: EVENTS KV not available");
            return;
        }
    };

    tracing::info!(
        event_id = %event_id,
        sheet_id = %sheet_id,
        sheet_name = %sheet_name,
        "walk-in auto-sync: resolved sheet"
    );

    match crate::sheets::get_column_mapping(&state, &sheet_id, &sheet_name, Some(&kv)).await {
        Ok(mapping) => {
            let api_id = Uuid::now_v7().to_string();
            match crate::sheets::append_walkin_row(
                &api_id,
                &attendee.name,
                &attendee.email,
                attendee.phone.as_deref(),
                &attendee.claim_token,
                &attendee.checked_in_at,
                &attendee.checked_in_by,
                attendee.wallet_address.as_deref(),
                attendee.claimed_at.as_deref(),
                &mapping,
                &state,
                &sheet_id,
                &sheet_name,
                Some(&kv),
            )
            .await
            {
                Ok(()) => {
                    tracing::info!(
                        event_id = %event_id,
                        email = %attendee.email,
                        "walk-in auto-synced to google sheet"
                    );
                    // Mark as synced so walkin_sync_handler skips it
                    let sync_key = format!("walkin_synced:{}:{}", event_id, attendee.email);
                    let sync_val = serde_json::to_string(&true).unwrap_or_default();
                    match kv
                        .put(&sync_key, &sync_val)
                        .map(|builder| builder.expiration_ttl(WALKIN_TTL_SECS))
                    {
                        Ok(builder) => {
                            if let Err(e) = builder.execute().await {
                                tracing::warn!(key = %sync_key, error = ?e, "failed to write sync marker");
                            }
                        }
                        Err(e) => {
                            tracing::warn!(key = %sync_key, error = ?e, "failed to build sync marker");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        event_id = %event_id,
                        email = %attendee.email,
                        error = %e,
                        "walk-in auto-sync to google sheet failed, will be retried by /walkin/sync"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                event_id = %event_id,
                error = %e,
                "walk-in auto-sync: failed to get column mapping, skipping sheet sync"
            );
        }
    }
}

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
/// 6. Auto-sync to Google Sheet (best-effort; falls back to /walkin/sync on failure)
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

    // 8. Auto-sync to Google Sheet (best-effort, detached via wait_until)
    let sheet_id = event.sheet_id.clone();
    let sheet_name = if event.sheet_name.is_empty() {
        state.config.sheets.sheet_name.clone()
    } else {
        event.sheet_name.clone()
    };

    // Detach the sheets sync — response returns immediately.
    // The sync future owns its data (no borrows on the handler's stack).
    if let Some(ctx) = &state.worker_ctx {
        let sync_state = state.clone();
        let sync_attendee = attendee.clone();
        let sync_event_id = event.id.clone();
        ctx.wait_until(sync_walkin_to_sheet(
            sync_state,
            sync_attendee,
            sync_event_id,
            sheet_id,
            sheet_name,
        ));
    } else {
        tracing::warn!(
            event_id = %event.id,
            "no worker_ctx available — skipping detached sheets sync"
        );
    }

    // Audit log
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::WalkinRegistered,
                &email_lower,
                "walk-in attendee registered",
            ),
        )
        .await;
    }

    Ok(ApiOk::new(WalkinRegisterResponse {
        claim_token,
        claim_url,
        attendee,
    }))
}

// ---------------------------------------------------------------------------
// List walk-in attendees (KV scan helper)
// ---------------------------------------------------------------------------

/// List all walk-in attendees for an event from KV.
/// Uses cursor-based pagination to scan `walkin:{event_id}:*` prefix.
pub async fn list_walkin_attendees(
    kv: &worker::KvStore,
    event_id: &str,
) -> Result<Vec<WalkinAttendee>, AppError> {
    let prefix = format!("walkin:{event_id}:");
    let mut attendees = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut builder = kv.list().prefix(prefix.clone());
        if let Some(c) = cursor.take() {
            builder = builder.cursor(c);
        }

        let resp = builder.execute().await.map_err(|e| {
            tracing::error!(event_id = %event_id, error = ?e, "failed to list walk-in keys");
            AppError::Internal(format!("failed to list walk-in keys: {e:?}"))
        })?;

        for key in &resp.keys {
            let raw: Option<String> = kv.get(&key.name).text().await.map_err(|e| {
                tracing::error!(key = %key.name, error = ?e, "failed to read walk-in record");
                AppError::Internal(format!("KV read failed: {e:?}"))
            })?;

            if let Some(json) = raw {
                match serde_json::from_str::<WalkinAttendee>(&json) {
                    Ok(a) => attendees.push(a),
                    Err(e) => {
                        tracing::warn!(key = %key.name, error = %e, "skipping malformed walk-in record");
                    }
                }
            }
        }

        if resp.list_complete {
            break;
        }
        cursor = resp.cursor;
    }

    Ok(attendees)
}

// ---------------------------------------------------------------------------
// GET /api/walkin/list — list walk-in attendees
// ---------------------------------------------------------------------------

/// Response for listing walk-in attendees.
#[derive(Debug, Serialize)]
pub struct WalkinListResponse {
    pub attendees: Vec<WalkinAttendee>,
    pub count: usize,
}

/// GET /api/walkin/list?event_id=xxx — list all walk-in attendees for an event.
#[worker::send]
pub async fn list_walkin_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<WalkinListResponse>, crate::error::WorkerError> {
    let event_id = query
        .event_id
        .as_deref()
        .ok_or_else(|| AppError::Validation("missing event_id parameter".to_string()))?;

    let event = resolve_event_with_access(&state, &claims, Some(event_id)).await?;

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        tracing::error!("EVENTS KV namespace not configured");
        AppError::Internal("EVENTS KV namespace not configured".to_string())
    })?;

    let attendees = list_walkin_attendees(kv, &event.id).await?;
    let count = attendees.len();

    tracing::info!(
        event_id = %event.id,
        count,
        staff = %claims.email,
        "listed walk-in attendees"
    );

    Ok(ApiOk::new(WalkinListResponse { attendees, count }))
}

// ---------------------------------------------------------------------------
// GET /api/walkin/export — CSV export
// ---------------------------------------------------------------------------

/// Response for walk-in CSV export.
#[derive(Debug, Serialize)]
pub struct WalkinExportResponse {
    pub csv: String,
    pub filename: String,
    pub count: usize,
}

/// Escape a CSV field containing commas, quotes, or newlines.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// GET /api/walkin/export?event_id=xxx — export all walk-in attendees as CSV.
#[worker::send]
pub async fn walkin_export_csv_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<WalkinExportResponse>, crate::error::WorkerError> {
    let event_id = query
        .event_id
        .as_deref()
        .ok_or_else(|| AppError::Validation("missing event_id parameter".to_string()))?;

    let event = resolve_event_with_access(&state, &claims, Some(event_id)).await?;

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        tracing::error!("EVENTS KV namespace not configured");
        AppError::Internal("EVENTS KV namespace not configured".to_string())
    })?;

    let attendees = list_walkin_attendees(kv, &event.id).await?;

    // Build CSV
    let mut csv =
        String::from("Name,Email,Phone,Check-in Time,Registered By,Wallet Address,NFT Claimed\n");
    for a in &attendees {
        let phone = a.phone.as_deref().unwrap_or("");
        let wallet = a.wallet_address.as_deref().unwrap_or("");
        let claimed = a.claimed_at.as_deref().unwrap_or("No");
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            escape_csv(&a.name),
            escape_csv(&a.email),
            escape_csv(phone),
            escape_csv(&a.checked_in_at),
            escape_csv(&a.checked_in_by),
            escape_csv(wallet),
            escape_csv(claimed),
        ));
    }

    let count = attendees.len();
    let filename = format!("walkin-attendees-{}.csv", event.id);

    tracing::info!(
        event_id = %event.id,
        count,
        staff = %claims.email,
        "walk-in CSV exported"
    );

    // Audit log
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::WalkinExported,
                &format!("{} attendees", count),
                "walk-in CSV exported",
            ),
        )
        .await;
    }

    Ok(ApiOk::new(WalkinExportResponse {
        csv,
        filename,
        count,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/walkin/sync — Google Sheet sync
// ---------------------------------------------------------------------------

/// Request body for walk-in sync.
#[derive(Debug, Deserialize)]
pub struct WalkinSyncRequest {
    pub event_id: String,
}

/// Response for walk-in sync.
#[derive(Debug, Serialize)]
pub struct WalkinSyncResponse {
    pub synced: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub total_walkins: usize,
}

/// POST /api/walkin/sync — sync walk-in attendees to Google Sheet.
#[worker::send]
pub async fn walkin_sync_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<WalkinSyncRequest>,
) -> Result<ApiOk<WalkinSyncResponse>, crate::error::WorkerError> {
    let event = resolve_event_with_access(&state, &claims, Some(&body.event_id)).await?;

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        tracing::error!("EVENTS KV namespace not configured");
        AppError::Internal("EVENTS KV namespace not configured".to_string())
    })?;

    let attendees = list_walkin_attendees(kv, &event.id).await?;

    let sheet_id = &event.sheet_id;
    let sheet_name = if event.sheet_name.is_empty() {
        &state.config.sheets.sheet_name
    } else {
        &event.sheet_name
    };

    // Get column mapping for the sheet
    let mapping = crate::sheets::get_column_mapping(&state, sheet_id, sheet_name, Some(kv))
        .await
        .map_err(|e| AppError::Internal(format!("failed to get column mapping: {e}")))?;

    let mut synced = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for a in &attendees {
        // Idempotency check: skip if already synced
        let sync_key = format!("walkin_synced:{}:{}", event.id, a.email.to_lowercase());
        let already_synced: Option<bool> = kv.get(&sync_key).json().await.ok().flatten();

        if already_synced == Some(true) {
            skipped += 1;
            continue;
        }

        // Generate a unique API ID for this walk-in in the sheet
        let api_id = Uuid::now_v7().to_string();

        match crate::sheets::append_walkin_row(
            &api_id,
            &a.name,
            &a.email,
            a.phone.as_deref(),
            &a.claim_token,
            &a.checked_in_at,
            &a.checked_in_by,
            a.wallet_address.as_deref(),
            a.claimed_at.as_deref(),
            &mapping,
            &state,
            sheet_id,
            sheet_name,
            Some(kv),
        )
        .await
        {
            Ok(()) => {
                // Mark as synced in KV (90-day TTL)
                let sync_val = serde_json::to_string(&true).unwrap_or_default();
                match kv
                    .put(&sync_key, &sync_val)
                    .map(|builder| builder.expiration_ttl(WALKIN_TTL_SECS))
                {
                    Ok(builder) => {
                        if let Err(e) = builder.execute().await {
                            tracing::warn!(key = %sync_key, error = ?e, "failed to write sync marker");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(key = %sync_key, error = ?e, "failed to build sync marker");
                    }
                }
                synced += 1;
            }
            Err(e) => {
                tracing::warn!(
                    email = %a.email,
                    error = %e,
                    "failed to sync walk-in to sheet"
                );
                errors.push(format!("{}: {e}", a.email));
            }
        }
    }

    // Invalidate attendee cache so the next GET /api/attendees picks up new rows
    let cache_key = format!("attendees_cache:{sheet_id}:{sheet_name}");
    let _ = kv.delete(&cache_key).await;

    tracing::info!(
        event_id = %event.id,
        synced,
        skipped,
        errors = errors.len(),
        total = attendees.len(),
        staff = %claims.email,
        "walk-in sheet sync completed"
    );

    // Audit log
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::WalkinSynced,
                &format!("synced={synced} skipped={skipped} errors={}", errors.len()),
                "walk-in attendees synced to Google Sheet",
            ),
        )
        .await;
    }

    Ok(ApiOk::new(WalkinSyncResponse {
        synced,
        skipped,
        errors,
        total_walkins: attendees.len(),
    }))
}
