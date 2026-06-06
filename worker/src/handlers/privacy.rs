//! PDPA Data Deletion API (Issue #043 Phase D).
//!
//! Self-service endpoint for attendees to request erasure of their personal data.
//! Called from the frontend when a logged-in user clicks "Request Data Deletion".
//!
//! Data deletion scope:
//! - D1: Clear PII from attendees, contacts, developer_profiles, registration_responses
//! - KV: Delete deposit status, THB deposit, claim locks, quiz/adventure progress, QR cache
//! - R2: Delete THB payment slips and refund receipts
//! - Google Sheets: Clear PII cells (not the row — organizer needs slot visibility)
//! - On-chain: Cannot delete — disclosed in privacy policy as technical limitation

use axum::{
    Extension,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DeleteRequestQuery {
    /// If provided, only delete data for this specific event.
    /// If omitted, delete data across ALL events.
    pub event_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// `POST /api/privacy/delete-request`
///
/// Self-service PDPA data deletion. The user's email is taken from the JWT
/// (verified identity — cannot impersonate another user).
#[worker::send]
pub async fn delete_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<DeleteRequestQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let email = claims.email.to_lowercase();
    tracing::info!(email = %email, event_id = ?query.event_id, "PDPA data deletion request");

    let mut summary = DeletionSummary::default();
    let kv = state.events_kv.as_ref();

    // 1. Find all D1 attendee rows for this email
    if let Some(db) = state.d1.as_deref() {
        let rows = crate::db::attendees::get_attendees_by_email(db, &email)
            .await
            .map_err(|e| AppError::Internal(format!("D1 lookup failed: {e}")))?;

        for entry in &rows {
            let a = &entry.attendee;

            // Filter by event_id if specified
            if let Some(ref target_event) = query.event_id
                && entry.event_id != *target_event
            {
                continue;
            }

            let attendee_id = &a.api_id;
            let event_id = &entry.event_id;
            let claim_token = a.claim_token.as_deref().unwrap_or("");

            tracing::info!(
                attendee_id = %attendee_id,
                event_id = %event_id,
                "clearing PII for attendee"
            );

            // D1: Clear PII columns
            if let Err(e) = crate::db::attendees::clear_attendee_pii(db, attendee_id).await {
                tracing::warn!(attendee_id = %attendee_id, error = %e, "D1 clear_attendee_pii failed");
            } else {
                summary.d1_attendees_cleared += 1;
            }

            // KV: Delete all attendee-related keys
            if let Some(kv) = kv {
                // Deposit status
                let key = event_store::deposit_status_key(event_id, attendee_id);
                if kv.delete(&key).await.is_ok() {
                    summary.kv_keys_deleted += 1;
                }

                // THB deposit
                let key = event_store::thb_deposit_key(event_id, attendee_id);
                if kv.delete(&key).await.is_ok() {
                    summary.kv_keys_deleted += 1;
                }

                // Claim lock
                if !claim_token.is_empty() {
                    let key = crate::claim::claim_lock_key(event_id, claim_token);
                    if kv.delete(&key).await.is_ok() {
                        summary.kv_keys_deleted += 1;
                    }
                }

                // Quiz progress
                if !claim_token.is_empty() {
                    let key = event_store::quiz_progress_key(event_id, claim_token);
                    if kv.delete(&key).await.is_ok() {
                        summary.kv_keys_deleted += 1;
                    }
                }

                // Adventure progress
                if !claim_token.is_empty() {
                    let key = format!("event:{event_id}:adventure:progress:{claim_token}");
                    if kv.delete(&key).await.is_ok() {
                        summary.kv_keys_deleted += 1;
                    }
                }

                // QR image cache
                let key = format!("qr:{attendee_id}");
                if kv.delete(&key).await.is_ok() {
                    summary.kv_keys_deleted += 1;
                }
            }

            // R2: Delete THB slips and refund receipts
            if let Some(bucket) = state.r2.as_ref() {
                for ext in &["", ".jpg", ".png", ".webp"] {
                    let slip_key =
                        crate::storage::slip_key(event_id, &format!("{attendee_id}{ext}"));
                    if crate::storage::exists(bucket, &slip_key)
                        .await
                        .unwrap_or(false)
                    {
                        let _ = crate::storage::delete(bucket, &slip_key).await;
                        summary.r2_objects_deleted += 1;
                    }
                    let refund_key =
                        crate::storage::refund_key(event_id, &format!("{attendee_id}{ext}"));
                    if crate::storage::exists(bucket, &refund_key)
                        .await
                        .unwrap_or(false)
                    {
                        let _ = crate::storage::delete(bucket, &refund_key).await;
                        summary.r2_objects_deleted += 1;
                    }
                }
            }

            // Google Sheets: Clear PII columns (background — don't block response)
            if let Some(ctx) = &state.worker_ctx
                && a.row_index > 0
            {
                let sheet_event_id = event_id.clone();
                let state_clone = state.clone();
                let row_index = a.row_index;
                ctx.wait_until(async move {
                    clear_sheet_pii(&state_clone, &sheet_event_id, row_index).await;
                });
            }

            summary.events_affected += 1;
        }

        // 2. Clear contact PII
        if let Err(e) = crate::db::contacts::clear_contact_pii(db, &email).await {
            tracing::warn!(email = %email, error = %e, "D1 clear_contact_pii failed");
        } else {
            summary.d1_contacts_cleared += 1;
        }

        // 3. Clear developer profile PII
        if let Err(e) = crate::db::developers::clear_developer_pii(db, &email).await {
            tracing::warn!(email = %email, error = %e, "D1 clear_developer_pii failed");
        } else {
            summary.d1_developer_cleared += 1;
        }

        // 4. Delete registration responses
        if let Err(e) = crate::db::developers::delete_developer_responses(db, &email).await {
            tracing::warn!(email = %email, error = %e, "D1 delete_developer_responses failed");
        } else {
            summary.d1_responses_deleted += 1;
        }
    }

    // Audit log
    if let Some(kv) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            query.event_id.as_deref().unwrap_or("global"),
            crate::audit_store::create_entry(
                &email,
                crate::audit_store::AuditAction::DataDeletionRequested,
                &email,
                &format!(
                    "PDPA deletion: {} events, {} KV keys, {} R2 objects",
                    summary.events_affected, summary.kv_keys_deleted, summary.r2_objects_deleted
                ),
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    tracing::info!(
        email = %email,
        events = summary.events_affected,
        d1_attendees = summary.d1_attendees_cleared,
        kv_keys = summary.kv_keys_deleted,
        r2_objects = summary.r2_objects_deleted,
        "PDPA data deletion completed"
    );

    Ok(ApiOk::new(json!({
        "status": "completed",
        "email": email,
        "events_affected": summary.events_affected,
        "d1_attendees_cleared": summary.d1_attendees_cleared,
        "d1_contacts_cleared": summary.d1_contacts_cleared,
        "d1_developer_cleared": summary.d1_developer_cleared,
        "d1_responses_deleted": summary.d1_responses_deleted,
        "kv_keys_deleted": summary.kv_keys_deleted,
        "r2_objects_deleted": summary.r2_objects_deleted,
        "on_chain_note": "On-chain data (wallet addresses, transaction signatures) is immutable and cannot be deleted. This is disclosed in our privacy policy as a technical limitation per PDPA Section 37.",
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Default)]
struct DeletionSummary {
    events_affected: usize,
    d1_attendees_cleared: usize,
    d1_contacts_cleared: usize,
    d1_developer_cleared: usize,
    d1_responses_deleted: usize,
    kv_keys_deleted: usize,
    r2_objects_deleted: usize,
}

/// Clear PII columns in the Google Sheet for a specific row.
async fn clear_sheet_pii(state: &AppState, event_id: &str, row_index: usize) {
    // Resolve the event to get sheet_id/sheet_name
    let kv = match state.events_kv.as_ref() {
        Some(kv) => kv,
        None => return,
    };
    let event = match crate::event_store::get_event_config(kv, event_id).await {
        Ok(Some(e)) => e,
        _ => {
            tracing::warn!(event_id = %event_id, "cannot resolve event for sheet PII clear");
            return;
        }
    };

    if event.sheet_id.is_empty() {
        return;
    }

    // PII columns to clear (column letter + row):
    // B=name, C=first_name, D=last_name, E=email, J=phone,
    // K=contact_channel, L=contact_handle, S=checked_in_by,
    // T=solana_address, U=qr_code_url, V=claim_token,
    // Y=bank_account, Z=bank_name, AA=account_name, AC=refund_link
    let pii_columns = [
        "B", "C", "D", "E", "J", "K", "L", "S", "T", "U", "V", "Y", "Z", "AA", "AC",
    ];
    let ranges: Vec<String> = pii_columns
        .iter()
        .map(|col| format!("{col}{row_index}"))
        .collect();

    // Use batch clear — clears multiple ranges in one API call
    if let Err(e) = crate::sheets::write::clear_sheet_cells_batch(
        state,
        &event.sheet_id,
        &event.sheet_name,
        &ranges,
        Some(kv),
    )
    .await
    {
        tracing::warn!(
            event_id = %event_id,
            row_index,
            error = %e,
            "failed to clear sheet PII cells"
        );
    }
}
