use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{ManualRefundRequest, MarkRefundRequest};
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::state::AppState;
use event_checkin_domain::models::attendee::SheetRow;

/// A refund proof is required on every cash refund, single or batch.
///
/// Attendees click it on their ticket page (`.issues/145`): accept an uploaded
/// image (checked like a slip) or an https link, nothing else.
fn validate_refund_proof(proof: &str) -> Result<(), AppError> {
    if proof.trim().is_empty() {
        return Err(AppError::Validation(
            "refund_proof_url is required".to_string(),
        ));
    }
    if proof.starts_with("data:") {
        return super::slip_upload::validate_slip_url(proof);
    }
    match event_checkin_domain::validation::safe_document_link(proof) {
        Some(_) => Ok(()),
        None => Err(AppError::Validation(
            "refund proof must be an https link to the transfer receipt".to_string(),
        )),
    }
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
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &attendee_id, d1)
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

    // MONEY-SAFETY: a deposit already converted to rolling credit incremented the
    // attendee's contacts balance — cash-refunding it too would pay them twice.
    // Mirror the guard in hold_credit.rs (which blocks the reverse direction).
    if thb_deposit.held_as_credit {
        return Err(AppError::Validation(
            "deposit already held as rolling credit — cannot also cash-refund".to_string(),
        )
        .into());
    }

    // MONEY-SAFETY: a rolling-credit application or staff comp was never funded
    // with cash — cash-refunding it would pay out money that was never deposited.
    if thb_deposit.is_non_cash() {
        return Err(AppError::Validation(
            "this is a credit-covered / comp deposit (no cash was received) — it cannot be cash-refunded".to_string(),
        )
        .into());
    }

    if !thb_deposit.verified {
        return Err(
            AppError::Validation("deposit not verified yet — cannot refund".to_string()).into(),
        );
    }

    validate_refund_proof(&body.refund_proof_url)?;

    // Upload refund proof data URL to R2 if available
    let refund_proof_url = super::maybe_upload_to_r2(
        &state,
        &event.id,
        &attendee_id,
        &body.refund_proof_url,
        crate::storage::PREFIX_REFUNDS,
    )
    .await;

    let now = Utc::now().to_rfc3339();

    // ATOMIC: single conditional D1 UPDATE (CAS) flips refunded 0→1 only if the
    // deposit is verified, not already refunded, and NOT held as credit. Mutually
    // exclusive with the hold CAS on the same row, so a refund and a hold-as-credit
    // can never both settle (no double-payout even under concurrency).
    let settled = match d1 {
        Some(db) => crate::db::thb_deposits::try_settle_refund(
            db,
            &event.id,
            &attendee_id,
            &now,
            &refund_proof_url,
        )
        .await
        .map_err(AppError::Internal)?,
        None => true, // no D1 (tests/local) — non-atomic fallback
    };
    if !settled {
        return Err(AppError::Validation(
            "deposit already settled (refunded or held as credit)".to_string(),
        )
        .into());
    }

    // Reflect the settled state on the in-memory record — it feeds the Sheets
    // write and the response below. D1 was already flipped by the CAS; the save
    // persists the non-settlement columns (and the whole struct on the KV
    // fallback path, where there is no CAS).
    thb_deposit.refunded = true;
    thb_deposit.refunded_at = Some(now.clone());
    thb_deposit.refund_proof_url = Some(refund_proof_url.clone());
    event_store::save_thb_deposit(kv, &thb_deposit, d1)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %attendee_id,
        event_id = %event.id,
        marker_fingerprint = %state.log_fingerprint(&claims.email),
        "THB refund marked complete"
    );

    // Resolve attendee row_index & column mapping for Sheets write
    let attendee_row = crate::sheets::get_attendee_by_id(&attendee_id, &state, &event, Some(kv))
        .await
        .ok()
        .flatten();

    let mapping =
        crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await
            .unwrap_or_else(|_| event_checkin_domain::models::attendee::ColumnMapping::hardcoded());

    if let Some(ref attendee) = attendee_row
        && let Some(ctx) = &state.worker_ctx
    {
        // Detach Google Sheets writes — response returns immediately (Phase 2c)
        ctx.wait_until(crate::sheets::bg_sync::write_refund_status(
            state.clone(),
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            Some(kv.clone()),
            SheetRow::of(attendee.api_id.clone()),
            "refunded".to_string(),
            mapping.clone(),
        ));
        ctx.wait_until(crate::sheets::bg_sync::write_refund_link(
            state.clone(),
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            Some(kv.clone()),
            SheetRow::of(attendee.api_id.clone()),
            refund_proof_url.clone(),
            mapping.clone(),
        ));
    } else {
        // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
        if let Err(e) = crate::sheets::write::write_refund_status(
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
            SheetRow::of(attendee_id.clone()),
            "refunded",
        )
        .await
        {
            tracing::warn!(
                attendee_id = %attendee_id,
                error = %e,
                "failed to write refund_status to sheet (non-blocking)"
            );
        }

        if let Err(e) = crate::sheets::write::write_refund_link(
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
            SheetRow::of(attendee_id.clone()),
            &refund_proof_url,
        )
        .await
        {
            tracing::warn!(
                attendee_id = %attendee_id,
                error = %e,
                "failed to write refund_link to sheet (non-blocking)"
            );
        }
    }

    // Dual-write to D1 — refund (non-fatal, Phase 2a)
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::mark_refund(
            d1,
            &event.id,
            &attendee_id,
            "refunded",
            &refund_proof_url,
            &now,
            &claims.email,
        )
        .await
    {
        tracing::warn!(
            %attendee_id,
            error = %e,
            "D1 refund write failed (non-fatal)"
        );
    }

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
        state.d1.as_deref(),
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
    /// Proof of the bulk transfer (https receipt link or uploaded image),
    /// stored on every deposit this batch settles. Required.
    #[serde(default)]
    pub refund_proof_url: String,
}

/// Batch-refund all THB deposits for an event.
/// Marks every verified, non-refunded cash THB deposit as refunded, with the
/// same proof and D1 attendee write as the single refund (`mark_refund_handler`).
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
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    validate_refund_proof(&body.refund_proof_url)?;
    let refund_proof_url = super::maybe_upload_to_r2(
        &state,
        &event.id,
        &format!("batch-{}", Utc::now().timestamp()),
        &body.refund_proof_url,
        crate::storage::PREFIX_REFUNDS,
    )
    .await;

    let deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    let now = Utc::now().to_rfc3339();
    let mut refunded = 0u32;
    let mut skipped = 0u32;
    let mut refunded_attendee_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for mut dep in deposits {
        if dep.refunded {
            skipped += 1;
            continue;
        }
        // MONEY-SAFETY: never cash-refund a deposit already held as rolling
        // credit — the attendee already received that value as credit.
        if dep.held_as_credit {
            skipped += 1;
            continue;
        }
        // MONEY-SAFETY: never cash-refund a non-cash deposit (rolling-credit
        // application / staff comp / ฿0) — no cash was ever received.
        if dep.is_non_cash() {
            skipped += 1;
            continue;
        }
        if !dep.verified {
            skipped += 1;
            continue;
        }
        // ATOMIC per-deposit settlement (CAS): flips refunded 0→1 only if still
        // verified/unrefunded/unheld. If another request (e.g. a concurrent hold
        // or single refund) already settled this row, the CAS matches 0 rows and
        // we skip it — no double-payout even if a batch races other actions.
        let settled = match d1 {
            Some(db) => crate::db::thb_deposits::try_settle_refund(
                db,
                &event.id,
                &dep.attendee_id,
                &now,
                &refund_proof_url,
            )
            .await
            .map_err(AppError::Internal)?,
            None => true, // no D1 (tests/local) — non-atomic fallback
        };
        if !settled {
            skipped += 1;
            continue;
        }
        // Reflect on the in-memory record for the Sheets write below. D1 was
        // already flipped by the CAS; the save persists the non-settlement
        // columns (and the whole struct on the KV fallback path).
        dep.refunded = true;
        dep.refunded_at = Some(now.clone());
        dep.refund_proof_url = Some(refund_proof_url.clone());
        event_store::save_thb_deposit(kv, &dep, d1)
            .await
            .map_err(AppError::Internal)?;
        // Same D1 attendee write as the single refund (non-fatal).
        if let Some(db) = d1
            && let Err(e) = crate::db::attendees::mark_refund(
                db,
                &event.id,
                &dep.attendee_id,
                "refunded",
                &refund_proof_url,
                &now,
                &claims.email,
            )
            .await
        {
            tracing::warn!(
                attendee_id = %dep.attendee_id,
                error = %e,
                "D1 batch refund write failed (non-fatal)"
            );
        }
        refunded_attendee_ids.insert(dep.attendee_id.clone());
        refunded += 1;
    }

    tracing::info!(
        event_id = %event.id,
        refunded,
        skipped,
        marker_fingerprint = %state.log_fingerprint(&claims.email),
        "Batch THB refund completed"
    );

    // Mirror D1 state into Google Sheet — refund_status and refund_link for
    // all batch-refunded attendees in a single batch update, the same two
    // columns the single refund writes. Non-fatal.
    if !refunded_attendee_ids.is_empty() {
        let mapping =
            crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
                .await
                .unwrap_or_else(|_| {
                    event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
                });

        let attendees =
            crate::sheets::get_attendees(&state, &event.sheet_id, &event.sheet_name, Some(kv))
                .await
                .unwrap_or_default();

        let rows: Vec<usize> = attendees
            .iter()
            .filter(|a| refunded_attendee_ids.contains(&a.api_id))
            .map(|a| a.row_index)
            .collect();

        if !rows.is_empty() {
            if let Some(ctx) = &state.worker_ctx {
                ctx.wait_until(crate::sheets::bg_sync::write_refund_batch(
                    state.clone(),
                    rows,
                    refund_proof_url.clone(),
                    mapping,
                    event.sheet_id.clone(),
                    event.sheet_name.clone(),
                    Some(kv.clone()),
                ));
            } else {
                // Fallback: blocking batch write when worker_ctx unavailable (tests)
                if let Err(e) = crate::sheets::write::write_refund_batch(
                    &rows,
                    "refunded",
                    &refund_proof_url,
                    &mapping,
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                )
                .await
                {
                    tracing::warn!(
                        error = %e,
                        "failed to write batch refund to sheet (non-blocking)"
                    );
                }
            }
        }
    }

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
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "refunded": refunded,
        "skipped": skipped,
        "total_thb_deposits": refunded + skipped,
    })))
}

// ---------------------------------------------------------------------------
// Manual refund (no deposit required)
// ---------------------------------------------------------------------------

/// Manually set refund status for an attendee (e.g., VIP who didn't deposit).
/// Writes refund_status and optionally refund_link to the Google Sheet.
#[worker::send]
pub async fn mark_manual_refund_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(attendee_id): Path<String>,
    Json(body): Json<ManualRefundRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    // Verify attendee exists in sheet
    let attendee = crate::sheets::get_attendee_by_id(&attendee_id, &state, &event, Some(kv))
        .await
        .map_err(|e| AppError::Internal(format!("failed to find attendee: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("attendee '{attendee_id}' not found")))?;

    // Resolve column mapping for Sheets write
    let mapping =
        crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await
            .unwrap_or_else(|_| event_checkin_domain::models::attendee::ColumnMapping::hardcoded());

    // Detach Google Sheets writes — response returns immediately (Phase 2c)
    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::write_refund_status(
            state.clone(),
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            Some(kv.clone()),
            SheetRow::of(attendee.api_id.clone()),
            body.refund_status.clone(),
            mapping.clone(),
        ));

        // Write refund_link if provided
        if let Some(ref link) = body.refund_link
            && !link.trim().is_empty()
        {
            tracing::info!(
                %attendee_id,
                link = %link,
                "detaching refund_link write to bg_sync"
            );
            ctx.wait_until(crate::sheets::bg_sync::write_refund_link(
                state.clone(),
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                Some(kv.clone()),
                SheetRow::of(attendee.api_id.clone()),
                link.clone(),
                mapping.clone(),
            ));
        }
    } else {
        // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
        if let Err(e) = crate::sheets::write::write_refund_status(
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
            SheetRow::of(attendee_id.clone()),
            &body.refund_status,
        )
        .await
        {
            tracing::warn!(
                attendee_id = %attendee_id,
                error = %e,
                "failed to write refund_status to sheet (non-blocking)"
            );
        }

        if let Some(ref link) = body.refund_link
            && !link.trim().is_empty()
        {
            tracing::info!(
                %attendee_id,
                link = %link,
                "writing refund_link to sheet"
            );
            if let Err(e) = crate::sheets::write::write_refund_link(
                &state,
                &event.sheet_id,
                &event.sheet_name,
                Some(kv),
                SheetRow::of(attendee_id.clone()),
                link,
            )
            .await
            {
                tracing::warn!(
                    attendee_id = %attendee_id,
                    error = %e,
                    "failed to write refund_link to sheet (non-blocking)"
                );
            }
        }
    }

    // Dual-write to D1 — manual refund (non-fatal, Phase 2a)
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::mark_refund(
            d1,
            &event.id,
            &attendee_id,
            &body.refund_status,
            body.refund_link.as_deref().unwrap_or(""),
            &chrono::Utc::now().to_rfc3339(),
            &claims.email,
        )
        .await
    {
        tracing::warn!(
            %attendee_id,
            error = %e,
            "D1 manual refund write failed (non-fatal)"
        );
    }

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::RefundMarked,
            &event.id,
            &format!(
                "manual refund status '{}' set for {} ({})",
                body.refund_status, attendee.name, attendee_id
            ),
        ),
        state.d1.as_deref(),
    )
    .await;

    tracing::info!(
        %attendee_id,
        status = %body.refund_status,
        has_link = body.refund_link.is_some(),
        marker_fingerprint = %state.log_fingerprint(&claims.email),
        "Manual refund status set"
    );

    Ok(ApiOk::new(serde_json::json!({
        "attendee_id": attendee_id,
        "refund_status": body.refund_status,
        "refund_link": body.refund_link,
    })))
}
