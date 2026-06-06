//! Walk-in attendee registration handler.
//!
//! Staff can register walk-in attendees who show up without pre-registering.
//! Records are stored in D1 as the sole primary store. After registration,
//! records are auto-synced to Google Sheets (best-effort) via `wait_until()`.
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
    original_sheet_id: String,
    original_sheet_name: String,
) {
    let kv = match state.events_kv.as_ref() {
        Some(kv) => kv.clone(),
        None => {
            tracing::warn!(event_id = %event_id, "sync: EVENTS KV not available");
            return;
        }
    };

    // Guard: attendee's event_id must match the resolved event
    if attendee.event_id != event_id {
        tracing::error!(
            attendee_event_id = %attendee.event_id,
            resolved_event_id = %event_id,
            email = %attendee.email,
            "walk-in auto-sync ABORTED: attendee event_id mismatch"
        );
        return;
    }

    // Resolve sheet_id and sheet_name from the FRESH event config in KV.
    // The handler runs detached via wait_until(), so the config may have changed
    // since the registration request was processed. Always use the current config
    // to ensure we write to the correct sheet.
    let (sheet_id, sheet_name) = match crate::event_store::get_event_config(&kv, &event_id).await {
        Ok(Some(fresh_config)) => {
            let sid = fresh_config.sheet_id.clone();
            let sname = if fresh_config.sheet_name.is_empty() {
                state.config.sheets.sheet_name.clone()
            } else {
                fresh_config.sheet_name.clone()
            };

            if sid != original_sheet_id || sname != original_sheet_name {
                tracing::warn!(
                    event_id = %event_id,
                    original_sheet_id = %original_sheet_id,
                    current_sheet_id = %sid,
                    original_sheet_name = %original_sheet_name,
                    current_sheet_name = %sname,
                    "walk-in auto-sync: event config changed since registration, using current config"
                );
            }
            (sid, sname)
        }
        Ok(None) => {
            tracing::error!(event_id = %event_id, "walk-in auto-sync ABORTED: event no longer exists in KV");
            return;
        }
        Err(e) => {
            tracing::warn!(
                event_id = %event_id,
                error = %e,
                original_sheet_id = %original_sheet_id,
                "walk-in auto-sync: could not re-verify event config, proceeding with original"
            );
            (original_sheet_id, original_sheet_name)
        }
    };

    if sheet_id.is_empty() {
        tracing::warn!(
            event_id = %event_id,
            "walk-in auto-sync skipped: resolved sheet_id is empty"
        );
        return;
    }

    tracing::info!(
        event_id = %event_id,
        email = %attendee.email,
        sheet_id = %sheet_id,
        sheet_name = %sheet_name,
        "walk-in auto-sync: starting"
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

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct WalkinRegisterRequest {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    /// Staff override: register walk-in even when in-person capacity is reached.
    #[serde(default)]
    pub override_capacity: bool,
}

#[derive(Debug, Serialize)]
pub struct WalkinRegisterResponse {
    pub claim_token: String,
    pub claim_url: String,
    pub attendee: WalkinAttendee,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// POST /api/walkin/register — register a walk-in attendee on-the-spot.
///
/// Flow:
/// 1. Validate input (name, email, phone)
/// 2. Check for duplicate in D1
/// 3. Generate claim token (UUID v7)
/// 4. Write walk-in record to D1
/// 5. Return claim URL and attendee data
/// 6. Auto-sync to Google Sheet (best-effort; falls back to /walkin/sync on failure)
#[worker::send]
pub async fn register_walkin(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::Json(body): axum::Json<WalkinRegisterRequest>,
) -> Result<ApiOk<WalkinRegisterResponse>, crate::error::WorkerError> {
    // 1. Validate input
    if body.event_id.trim().is_empty() {
        return Err(AppError::Validation(
            "event_id is required — cannot register walk-in without an event".to_string(),
        )
        .into());
    }

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
    let requested_event_id = body.event_id.clone();
    let event = resolve_event_with_access(&state, &claims, Some(&body.event_id)).await?;

    tracing::info!(
        requested_event_id = %requested_event_id,
        resolved_event_id = %event.id,
        event_name = %event.name,
        sheet_id = %event.sheet_id,
        sheet_name = %event.sheet_name,
        event_format = %event.event_format,
        "walk-in: resolved event config"
    );

    // 2a. Block walk-in for online-only events
    if !event.event_format.has_in_person() {
        return Err(AppError::Validation(
            "Walk-in registration is not available for online-only events.".to_string(),
        )
        .into());
    }

    let email_lower = email.to_lowercase();

    // 2. Check for duplicate (D1)
    let db = state.d1.as_deref().ok_or_else(|| {
        AppError::Internal("D1 database not available for walkin registration".to_string())
    })?;

    let is_duplicate = crate::db::attendees::check_walkin_duplicate(db, &event.id, &email_lower)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %event.id, email = %email_lower, error = %e, "D1 walkin duplicate check failed");
            AppError::Internal(format!("D1 duplicate check failed: {e}"))
        })?;

    if is_duplicate {
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

    // 3. Enforce in-person capacity (walk-ins are always in-person)
    if !body.override_capacity {
        enforce_walkin_capacity(&state, &event).await?;
    } else {
        tracing::info!(
            event_id = %event.id,
            "walk-in capacity override: staff bypassing capacity check"
        );
    }

    // 4. Generate claim token
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

    // 5. Persist walk-in record to D1 (sole primary store)
    let inserted = crate::db::attendees::try_insert_walkin(
        db,
        &claim_token,
        &event.id,
        &email_lower,
        name,
        attendee.phone.as_deref(),
        &attendee.checked_in_at,
        &claims.email,
        &claim_token,
    )
    .await
    .map_err(|e| {
        tracing::error!(event_id = %event.id, email = %email_lower, error = %e, "D1 walkin write failed");
        AppError::Internal(format!("D1 walkin write failed: {e}"))
    })?;
    if !inserted {
        tracing::warn!(
            event_id = %event.id,
            email = %email_lower,
            "walk-in duplicate detected at insert time (race condition)"
        );
        return Err(AppError::Validation(
            "a walk-in attendee with this email is already registered for this event".to_string(),
        )
        .into());
    }
    tracing::info!(event_id = %event.id, email = %email_lower, "walkin registered to D1");

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

    if sheet_id.is_empty() {
        tracing::warn!(
            event_id = %event.id,
            event_name = %event.name,
            "walk-in auto-sync skipped: event has no sheet_id configured"
        );
    } else if let Some(ctx) = &state.worker_ctx {
        tracing::info!(
            event_id = %event.id,
            event_name = %event.name,
            sheet_id = %sheet_id,
            sheet_name = %sheet_name,
            "walk-in auto-sync: dispatching to sheet"
        );
        // Detach the sheets sync — response returns immediately.
        // The sync future owns its data (no borrows on the handler's stack).
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
            state.d1.as_deref(),
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
// List walk-in attendees
// ---------------------------------------------------------------------------

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

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let attendees = crate::db::attendees::get_walkin_attendees(db, &event.id)
        .await
        .map_err(|e| AppError::Internal(format!("D1 walkin list failed: {e}")))?;
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

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let attendees = crate::db::attendees::get_walkin_attendees(db, &event.id)
        .await
        .map_err(|e| AppError::Internal(format!("D1 walkin list failed: {e}")))?;

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
            state.d1.as_deref(),
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

    let kv = state.events_kv.as_ref();

    // Fetch walk-in attendees from D1
    let db = match state.d1.as_deref() {
        Some(db) => db,
        None => {
            tracing::info!(event_id = %event.id, "walkin sync skipped: D1 not available");
            return Ok(ApiOk::new(WalkinSyncResponse {
                synced: 0,
                skipped: 0,
                errors: vec![],
                total_walkins: 0,
            }));
        }
    };

    let attendees = crate::db::attendees::get_walkin_attendees(db, &event.id)
        .await
        .map_err(|e| AppError::Internal(format!("D1 walkin list failed: {e}")))?;

    if event.sheet_id.is_empty() {
        return Err(AppError::Validation(format!(
            "event '{}' has no sheet_id configured — cannot sync walk-ins",
            event.name
        ))
        .into());
    }

    let sheet_id = &event.sheet_id;
    let sheet_name = if event.sheet_name.is_empty() {
        &state.config.sheets.sheet_name
    } else {
        &event.sheet_name
    };

    tracing::info!(
        event_id = %event.id,
        event_name = %event.name,
        sheet_id = %sheet_id,
        sheet_name = %sheet_name,
        walkin_count = attendees.len(),
        "walk-in batch sync: starting"
    );

    // Get column mapping for the sheet
    let mapping = crate::sheets::get_column_mapping(&state, sheet_id, sheet_name, kv)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get column mapping: {e}")))?;

    let mut synced = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for a in &attendees {
        // Skip already-claimed walk-ins (no need to sync to sheet)
        if a.claimed_at.is_some() {
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
            kv,
        )
        .await
        {
            Ok(()) => {
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
            state.d1.as_deref(),
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

// ---------------------------------------------------------------------------
// Capacity enforcement
// ---------------------------------------------------------------------------

/// Count current in-person attendees (sheet + walk-in) and reject if capacity is reached.
async fn enforce_walkin_capacity(
    state: &AppState,
    config: &event_checkin_domain::models::event::EventConfig,
) -> Result<(), AppError> {
    // Only enforce when a capacity limit is set
    let cap = match config.in_person_capacity {
        Some(c) => c,
        None => return Ok(()),
    };

    let kv = state.events_kv.as_ref();

    // Count sheet-based in-person attendees
    let attendees = crate::sheets::get_attendees_for_event(
        state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to check capacity: {e}")))?;

    let mut in_person_count: u32 = attendees.iter().filter(|a| a.is_in_person()).count() as u32;

    // Count walk-in attendees from D1
    if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::count_walkin_attendees(db, &config.id).await {
            Ok(count) => {
                in_person_count += count;
            }
            Err(e) => {
                tracing::warn!(error = %e, "D1 walkin count for capacity failed, skipping");
            }
        }
    }

    tracing::info!(
        event_id = %config.id,
        in_person_count,
        in_person_capacity = cap,
        "walk-in capacity check"
    );

    if in_person_count >= cap {
        return Err(AppError::Validation(
            "CAPACITY_REACHED: In-person spots are full. Override to register anyway.".to_string(),
        ));
    }

    Ok(())
}
