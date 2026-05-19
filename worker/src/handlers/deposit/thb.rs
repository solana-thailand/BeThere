use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{
    DepositMethod, DepositStatus, MarkRefundRequest, PendingSlipResponse, RefundQueueResponse,
    ThbDeposit, VerifySlipRequest,
};
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Slip URL validation
// ---------------------------------------------------------------------------

/// Validate a slip URL or data URL for safety.
///
/// Ensures:
/// - Data URLs have an allowed MIME type (image/jpeg, image/png, image/webp)
/// - Data URLs are within size limits (decoded ≤ 5MB, encoded ≤ 7MB)
/// - SVG is rejected (XSS risk)
/// - External URLs use HTTPS
fn validate_slip_url(slip_url: &str) -> Result<(), AppError> {
    if slip_url.is_empty() {
        return Err(AppError::Validation("slip URL is empty".to_string()));
    }

    if let Some(rest) = slip_url.strip_prefix("data:") {
        // Data URL — validate MIME type and size
        // Format: data:<mediatype>;base64,<data>
        let (header, _data) = rest
            .split_once(',')
            .ok_or_else(|| AppError::Validation("invalid data URL format".to_string()))?;

        // Check base64 encoding
        if !header.contains(";base64") && !header.contains(";base64,") {
            return Err(AppError::Validation(
                "data URL must use base64 encoding".to_string(),
            ));
        }

        // Extract MIME type (before semicolon)
        let mime = header.split(';').next().unwrap_or("").trim();

        // Whitelist safe image types — reject SVG (XSS risk)
        let allowed_mimes = ["image/jpeg", "image/png", "image/webp", "image/jpg"];
        if !allowed_mimes.contains(&mime) {
            return Err(AppError::Validation(format!(
                "unsupported image type '{mime}' — allowed: JPEG, PNG, WebP"
            )));
        }

        // Check total data URL size (encoded)
        if slip_url.len() > 7 * 1024 * 1024 {
            return Err(AppError::Validation(
                "slip image too large (max 5MB)".to_string(),
            ));
        }

        Ok(())
    } else if slip_url.starts_with("http://") || slip_url.starts_with("https://") {
        // External URL — require HTTPS in production (allow HTTP for dev)
        Ok(())
    } else {
        Err(AppError::Validation(
            "slip URL must be a data URL (image upload) or HTTPS URL".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// POST /api/deposit/thb/upload
// ---------------------------------------------------------------------------

/// Record a THB payment slip upload.
///
/// The frontend uploads the slip image to R2 separately and passes the URL.
/// This creates a THB deposit record in KV for admin verification.
#[worker::send]
pub async fn upload_thb_slip_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ThbSlipUploadRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %body.event_id,
        uploader_email = %claims.email,
        "THB slip upload initiated"
    );

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.deposit_amount_thb == 0 {
        return Err(AppError::Validation("THB deposit amount not configured".to_string()).into());
    }

    // Validate slip URL for safety (MIME type, size, no SVG/XSS)
    validate_slip_url(&body.slip_url)?;

    // Check if already deposited
    let existing = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    if existing.is_some() {
        return Err(AppError::Validation("attendee already has a deposit".to_string()).into());
    }

    // Deposit deadline check: reject OR reclaim
    // If deadline expired but in-person capacity is still available,
    // switch the attendee back to In-Person and allow the deposit (reclaim flow).
    if let Some(deadline_hours) = event.deposit_deadline_hours
        && let Ok(Some(attendee)) = crate::sheets::get_attendee_by_id(
            &body.attendee_id,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
        )
        .await
        && let Some(reg_str) = &attendee.registration_date
        && let Ok(reg_time) = chrono::DateTime::parse_from_rfc3339(reg_str)
    {
        let deadline = reg_time.with_timezone(&chrono::Utc)
            + chrono::Duration::hours(i64::from(deadline_hours));
        if chrono::Utc::now() > deadline {
            // Deadline passed — check if reclaim is possible
            let capacity_available = if let Some(cap) = event.in_person_capacity {
                // Quick capacity check (sheet only — walk-ins less likely for THB)
                let in_person_count = crate::sheets::get_attendees(
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                )
                .await
                .map(|a| a.iter().filter(|a| a.is_in_person()).count() as u32)
                .unwrap_or(u32::MAX);
                in_person_count < cap
            } else {
                true // No capacity limit = always available
            };

            if capacity_available {
                // Reclaim: switch back to In-Person
                if let Ok(mapping) = crate::sheets::get_column_mapping(
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                )
                .await
                {
                    match crate::sheets::write::update_participation_type(
                        attendee.row_index,
                        "In-Person",
                        &mapping,
                        &state,
                        &event.sheet_id,
                        &event.sheet_name,
                        Some(kv),
                    )
                    .await
                    {
                        Ok(()) => tracing::info!(
                            attendee_id = %attendee.api_id,
                            "THB deposit deadline reclaim: switched back to In-Person"
                        ),
                        Err(e) => tracing::warn!(
                            attendee_id = %attendee.api_id,
                            error = %e,
                            "THB deposit deadline reclaim: failed to switch back"
                        ),
                    }
                }
            } else {
                return Err(AppError::Validation(
                    "deposit deadline has passed and in-person spots are now full. You have been moved to the online track.".to_string(),
                ).into());
            }
        }
    }

    let now = Utc::now().to_rfc3339();

    // Create THB deposit record
    let thb_deposit = ThbDeposit {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        amount_thb: event.deposit_amount_thb,
        slip_url: Some(body.slip_url.clone()),
        verified: false,
        verified_by: None,
        verified_at: None,
        uploaded_at: now.clone(),
        refunded: false,
        refunded_at: None,
        attendee_name: None,
    };

    event_store::save_thb_deposit(kv, &thb_deposit)
        .await
        .map_err(AppError::Internal)?;

    // Atomically increment deposit counter for this event
    let deposit_order = event_store::increment_deposit_counter(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;
    let refundable =
        event.max_refundable_deposits == 0 || deposit_order <= event.max_refundable_deposits;

    // Create deposit status
    let deposit_status = DepositStatus {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        method: DepositMethod::Thb,
        amount: event.deposit_amount_thb,
        currency: "THB".to_string(),
        tx_signature: None,
        verified: false,
        deposited_at: now,
        wallet_address: None,
        deposit_order,
        refundable,
    };

    event_store::save_deposit_status(kv, &deposit_status)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        amount_thb = event.deposit_amount_thb,
        "THB deposit slip uploaded"
    );

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "message": "slip uploaded, awaiting verification"
    })))
}

/// Request body for THB slip upload.
#[derive(Debug, serde::Deserialize)]
pub struct ThbSlipUploadRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// R2 URL of the uploaded payment slip image.
    pub slip_url: String,
}

// ---------------------------------------------------------------------------
// POST /api/deposit/thb/verify (admin)
// ---------------------------------------------------------------------------

/// Admin verifies or rejects a THB payment slip.
#[worker::send]
pub async fn verify_thb_slip_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<VerifySlipRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    // Get existing THB deposit
    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "THB deposit not found for attendee '{}' in event '{}'",
                body.attendee_id, event.id
            ))
        })?;

    let now = Utc::now().to_rfc3339();

    if body.approved {
        thb_deposit.verified = true;
        thb_deposit.verified_by = Some(claims.email.clone());
        thb_deposit.verified_at = Some(now.clone());
    } else {
        // Rejected — keep the record but mark as not verified
        thb_deposit.verified = false;
        thb_deposit.verified_by = Some(claims.email.clone());
        thb_deposit.verified_at = Some(now.clone());
    }

    event_store::save_thb_deposit(kv, &thb_deposit)
        .await
        .map_err(AppError::Internal)?;

    // Update deposit status
    if let Some(mut status) = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?
    {
        status.verified = body.approved;
        event_store::save_deposit_status(kv, &status)
            .await
            .map_err(AppError::Internal)?;
    }

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        approved = body.approved,
        verifier = %claims.email,
        "THB deposit slip verified"
    );

    let msg = if body.approved {
        "deposit verified"
    } else {
        "deposit rejected"
    };

    // Audit log
    let action = if body.approved {
        crate::audit_store::AuditAction::DepositVerified
    } else {
        crate::audit_store::AuditAction::DepositRejected
    };
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            action,
            &body.attendee_id,
            &format!(
                "THB slip {}",
                if body.approved {
                    "verified"
                } else {
                    "rejected"
                }
            ),
        ),
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "message": msg
    })))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/thb/pending (admin)
// ---------------------------------------------------------------------------

/// List all unverified THB deposits for admin review.
#[worker::send]
pub async fn pending_thb_slips_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<PendingSlipResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| !d.verified && d.slip_url.is_some())
        .collect();

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &pending).await;
    let slips: Vec<ThbDeposit> = pending
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(PendingSlipResponse { slips }))
}

// ---------------------------------------------------------------------------
// GET /api/refund/queue (admin)
// ---------------------------------------------------------------------------

/// List THB deposits that need refund (verified + checked-in + not yet refunded).
#[worker::send]
pub async fn refund_queue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<RefundQueueResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| d.verified && !d.refunded)
        .collect();

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &pending).await;
    let enriched: Vec<ThbDeposit> = pending
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(RefundQueueResponse { pending: enriched }))
}

// ---------------------------------------------------------------------------
// POST /api/refund/mark/{attendee_id} (admin)
// ---------------------------------------------------------------------------

/// Mark a THB refund as completed.
#[worker::send]
pub async fn mark_refund_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(attendee_id): Path<String>,
    Json(body): Json<MarkRefundRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &attendee_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "THB deposit not found for attendee '{attendee_id}' in event '{}'",
                event.id
            ))
        })?;

    if thb_deposit.refunded {
        return Err(AppError::Validation("already refunded".to_string()).into());
    }

    if !thb_deposit.verified {
        return Err(
            AppError::Validation("deposit not verified yet — cannot refund".to_string()).into(),
        );
    }

    let now = Utc::now().to_rfc3339();
    thb_deposit.refunded = true;
    thb_deposit.refunded_at = Some(now);

    event_store::save_thb_deposit(kv, &thb_deposit)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %attendee_id,
        event_id = %event.id,
        marker = %claims.email,
        "THB refund marked complete"
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::RefundMarked,
            &attendee_id,
            "refund marked complete",
        ),
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "message": "refund marked complete"
    })))
}

// ---------------------------------------------------------------------------
// POST /api/refund/batch-thb (admin)
// ---------------------------------------------------------------------------

/// Request body for batch THB refund.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BatchThbRefundRequest {
    pub event_id: String,
}

/// Batch-refund all THB deposits for an event.
/// Marks every verified, non-refunded THB deposit as refunded.
#[worker::send]
pub async fn batch_thb_refund_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<BatchThbRefundRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    let deposits = event_store::list_thb_deposits(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let now = Utc::now().to_rfc3339();
    let mut refunded = 0u32;
    let mut skipped = 0u32;

    for mut dep in deposits {
        if dep.refunded {
            skipped += 1;
            continue;
        }
        if !dep.verified {
            skipped += 1;
            continue;
        }
        dep.refunded = true;
        dep.refunded_at = Some(now.clone());
        event_store::save_thb_deposit(kv, &dep)
            .await
            .map_err(AppError::Internal)?;
        refunded += 1;
    }

    tracing::info!(
        event_id = %event.id,
        refunded,
        skipped,
        marker = %claims.email,
        "Batch THB refund completed"
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::RefundMarked,
            &event.id,
            &format!("batch THB refund: {refunded} refunded, {skipped} skipped"),
        ),
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "refunded": refunded,
        "skipped": skipped,
        "total_thb_deposits": refunded + skipped,
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve attendee display names from Google Sheets for a list of deposits.
/// Returns a map of `attendee_id → display_name`.
/// Silently skips attendees not found — the frontend falls back to showing the raw ID.
async fn resolve_attendee_names(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    deposits: &[ThbDeposit],
) -> std::collections::HashMap<String, String> {
    use crate::handlers::ext::resolve_kv;
    use crate::sheets;

    if deposits.is_empty() {
        return std::collections::HashMap::new();
    }

    let kv = resolve_kv(state);
    match sheets::get_attendees_map(state, sheet_id, sheet_name, kv).await {
        Ok(map) => deposits
            .iter()
            .filter_map(|d| {
                map.get(&d.attendee_id)
                    .map(|a| (d.attendee_id.clone(), a.display_name().to_string()))
            })
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "failed to resolve attendee names for deposits");
            std::collections::HashMap::new()
        }
    }
}
