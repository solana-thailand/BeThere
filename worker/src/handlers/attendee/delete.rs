//! `DELETE /api/attendee/{id}` — delete attendee from system + sheet.

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{EventIdQuery, resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// DELETE /api/attendee/{id} — delete attendee from system + sheet
// ---------------------------------------------------------------------------

/// Delete attendee request (supports both sheet-based and walk-in attendees).
///
/// Cleans up:
/// - D1 attendee record (primary store)
/// - Google Sheet row (async, best-effort)
/// - Claim locks
/// - QR image cache
#[worker::send]
pub async fn delete_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(attendee_id = %id, staff_email = %claims.email, "delete attendee request");

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    let mut deleted_keys = Vec::new();
    let source;

    // 1. Try to find as walk-in attendee in D1 (primary store)
    //    Walk-ins have participation_type='walkin' in D1.
    //    The `id` param may be a claim_token, email, or name.
    let mut walkin: Option<event_checkin_domain::models::attendee::WalkinAttendee> = None;
    if let Some(db) = state.d1.as_deref() {
        // Try claim_token lookup first
        if let Ok(Some(a)) = crate::db::attendees::get_attendee_by_claim_token(db, &id).await
            && a.participation_type == "walkin"
        {
            walkin = Some(event_checkin_domain::models::attendee::WalkinAttendee {
                event_id: event.id.clone(),
                name: a.name.clone(),
                email: a.email.clone(),
                phone: a.phone.clone(),
                claim_token: a.claim_token.clone().unwrap_or_default(),
                checked_in_at: a.checked_in_at.clone().unwrap_or_default(),
                checked_in_by: a.checked_in_by.clone().unwrap_or_default(),
                wallet_address: None,
                claimed_at: a.claimed_at.clone(),
            });
        }
    }

    if let Some(walkin_attendee) = walkin {
        source = "walk-in".to_string();
        let email_lower = walkin_attendee.email.to_lowercase();
        let claim_token = walkin_attendee.claim_token.clone();

        // Delete from D1 (primary)
        if let Some(db) = state.d1.as_deref()
            && let Err(e) = crate::db::attendees::delete_attendee(db, &event.id, &email_lower).await
        {
            tracing::warn!(
                event_id = %event.id,
                email = %email_lower,
                error = %e,
                "D1 walk-in delete failed"
            );
        }

        // Clean up claim lock
        if let Some(kv) = kv {
            let lkey = crate::claim::claim_lock_key(&event.id, &claim_token);
            let _ = kv.delete(&lkey).await;
            deleted_keys.push(lkey);
        }

        tracing::info!(
            event_id = %event.id,
            email = %email_lower,
            name = %walkin_attendee.name,
            "walk-in attendee deleted"
        );
    } else {
        // 2. Try to find as regular attendee.
        //
        //    Lookup order (matches the dashboard's data source so that what the
        //    admin sees is what gets deleted):
        //      a. D1 by api_id  — authoritative record; gives us email,
        //         claim_token, and a (possibly stale) sheet_row_index.
        //      b. Google Sheet   — read fresh to resolve the CURRENT row_index.
        //         Match by api_id first; fall back to email (case-insensitive)
        //         because some sheets don't populate the api_id column, which
        //         would otherwise cause the sheet row to be missed.
        //
        //    Earlier this code only consulted the Sheet (get_attendees_map with
        //    no event_id skips D1). When the Sheet lookup missed (empty api_id
        //    column, shifted rows, transient Sheets API error), the handler
        //    fell into the "d1-orphan" branch and ONLY deleted from D1 — so the
        //    Google Sheet row survived and the attendee reappeared after any
        //    re-sync. We now delete from BOTH stores whenever a row can be
        //    located in either, preferring the Sheet's fresh row_index over
        //    D1's possibly-stale sheet_row_index.

        // (a) D1 authoritative lookup
        let d1_attendee = if let Some(db) = state.d1.as_deref() {
            crate::db::attendees::get_attendee_by_id(db, &id)
                .await
                .map_err(|e| AppError::Internal(format!("D1 lookup for delete: {e}")))?
        } else {
            None
        };

        // (b) Sheet lookup — try api_id, then email
        let sheet_map = sheets::get_attendees_map(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
            .map_err(|e| AppError::Internal(format!("failed to read attendees sheet: {e}")))?;
        let sheet_attendee = sheet_map.get(&id).cloned().or_else(|| {
            // api_id miss — retry by email (lowercased). Email is reliably
            // populated by registration forms even when api_id is blank.
            let email = d1_attendee.as_ref().map(|a| a.email.to_lowercase())?;
            sheet_map
                .values()
                .find(|a| a.email.eq_ignore_ascii_case(&email))
                .cloned()
        });

        if sheet_attendee.is_none() && d1_attendee.is_none() {
            // Truly unknown — nothing to delete anywhere.
            return Err(AppError::NotFound(format!(
                "attendee '{id}' not found in walk-in records, event sheet, or D1"
            ))
            .into());
        }

        // Reconcile into one view for the cleanup below.
        // Prefer the Sheet row (it carries the live row_index + the email as
        // stored in the sheet); fall back to the D1 record.
        let attendee = sheet_attendee.clone().or(d1_attendee.clone());
        let attendee = match attendee {
            Some(a) => a,
            None => {
                return Err(
                    AppError::NotFound(format!("attendee '{id}' could not be resolved")).into(),
                );
            }
        };

        // Choose the sheet row_index to delete:
        //   1. Sheet's live row_index (authoritative) when available.
        //   2. D1's sheet_row_index as a last resort — but only if non-zero,
        //      since 0 is the "unset" sentinel and would delete the header row.
        let sheet_row_to_delete: Option<usize> = sheet_attendee
            .as_ref()
            .map(|a| a.row_index)
            .filter(|&r| r >= 2) // row 1 is the header; data starts at row 2
            .or_else(|| {
                let r = d1_attendee.as_ref().map(|a| a.row_index).unwrap_or(0);
                if r >= 2 {
                    tracing::warn!(
                        event_id = %event.id,
                        attendee_id = %id,
                        row_index = r,
                        "sheet lookup missed; deleting by D1 sheet_row_index (may be stale)"
                    );
                    Some(r)
                } else {
                    None
                }
            });

        source = if sheet_attendee.is_some() {
            "sheet"
        } else {
            "d1-orphan"
        }
        .to_string();

        // Delete the Google Sheet row (best-effort). If we couldn't resolve a
        // row_index at all, skip the sheet delete but still clean D1/KV so the
        // attendee leaves the dashboard.
        if let Some(row_index) = sheet_row_to_delete {
            let mapping =
                sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
                    .await
                    .unwrap_or_else(|_| {
                        event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
                    });

            if let Some(ctx) = &state.worker_ctx {
                ctx.wait_until(crate::sheets::bg_sync::delete_sheet_row(
                    state.clone(),
                    row_index,
                    mapping.clone(),
                    event.sheet_id.clone(),
                    event.sheet_name.clone(),
                    kv.cloned(),
                ));
            } else {
                crate::sheets::write::delete_sheet_row(
                    row_index,
                    &mapping,
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                .map_err(|e| AppError::Internal(format!("failed to delete sheet row: {e}")))?;
            };
        } else {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %id,
                "no resolvable sheet row_index; skipping sheet delete (D1/KV still cleaned)"
            );
        }

        // Delete from D1 (primary store). Use email when we have it (covers
        // duplicate-id edge cases), else delete by id.
        let email_lower = attendee.email.to_lowercase();
        if !email_lower.is_empty() {
            if let Some(db) = state.d1.as_deref()
                && let Err(e) =
                    crate::db::attendees::delete_attendee(db, &event.id, &email_lower).await
            {
                tracing::warn!(
                    event_id = %event.id,
                    email = %email_lower,
                    error = %e,
                    "D1 attendee delete (by email) failed"
                );
            }
        } else if let Some(db) = state.d1.as_deref()
            && let Err(e) = crate::db::attendees::delete_attendee_by_id(db, &id).await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %id,
                error = %e,
                "D1 attendee delete (by id) failed"
            );
        }

        // Clean up KV keys for this attendee
        if let Some(kv) = kv {
            // Deposit status
            let dkey = crate::event_store::deposit_status_key(&event.id, &attendee.api_id);
            let _ = kv.delete(&dkey).await;
            deleted_keys.push(dkey);

            // THB deposit (KV cleanup — D1 also cleaned below)
            let tkey = crate::event_store::thb_deposit_key(&event.id, &attendee.api_id);
            let _ = kv.delete(&tkey).await;
            deleted_keys.push(tkey);

            // Claim lock (if has claim_token)
            if let Some(ref token) = attendee.claim_token
                && !token.is_empty()
            {
                let lkey = crate::claim::claim_lock_key(&event.id, token);
                let _ = kv.delete(&lkey).await;
                deleted_keys.push(lkey);
            }

            // QR image cache
            let qrkey = format!("qr:{}", attendee.api_id);
            let _ = kv.delete(&qrkey).await;
            deleted_keys.push(qrkey);
        }

        if let Some(db) = state.d1.as_deref()
            && let Err(e) =
                crate::db::thb_deposits::delete_thb_deposit(db, &event.id, &attendee.api_id).await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %attendee.api_id,
                error = %e,
                "D1 THB deposit delete failed"
            );
        }

        // Delete deposit status from D1
        if let Some(db) = state.d1.as_deref()
            && let Err(e) =
                crate::db::deposit_statuses::delete_deposit_status(db, &event.id, &attendee.api_id)
                    .await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %attendee.api_id,
                error = %e,
                "D1 deposit status delete failed"
            );
        }

        tracing::info!(
            event_id = %event.id,
            attendee_id = %attendee.api_id,
            name = %attendee.display_name(),
            row_index = ?sheet_row_to_delete,
            source = %source,
            "attendee deleted"
        );
    }

    // Audit log
    if let Some(kv) = &state.events_kv {
        let action = if source == "walk-in" {
            crate::audit_store::AuditAction::WalkinDeleted
        } else {
            crate::audit_store::AuditAction::AttendeeDeleted
        };
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                action,
                &id,
                &format!("attendee deleted (source={source})"),
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    Ok(ApiOk::new(json!({
        "deleted": true,
        "attendee_id": id,
        "event_id": event.id,
        "source": source,
        "kv_keys_removed": deleted_keys.len(),
    })))
}
