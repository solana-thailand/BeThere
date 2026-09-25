use axum::{Extension, Json, extract::State};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::VerifySlipRequest;
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::state::AppState;
use event_checkin_domain::models::attendee::SheetRow;

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
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    // Get existing THB deposit
    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "THB deposit not found for attendee '{}' in event '{}'",
                body.attendee_id, event.id
            ))
        })?;

    // Guard: a terminal-settled deposit (refunded / held-as-credit) or a non-cash
    // deposit (credit application / staff comp) must not be re-verified —
    // re-approving one could resurrect it into the refund queue or muddle its
    // state. Cash slips awaiting review are the only verifiable target.
    if thb_deposit.refunded {
        return Err(AppError::Validation(
            "deposit already refunded — cannot re-verify".to_string(),
        )
        .into());
    }
    if thb_deposit.held_as_credit {
        return Err(AppError::Validation(
            "deposit already held as rolling credit — cannot re-verify".to_string(),
        )
        .into());
    }
    if thb_deposit.is_non_cash() {
        return Err(AppError::Validation(
            "credit-covered / comp deposit — there is no cash slip to verify".to_string(),
        )
        .into());
    }

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

    event_store::save_thb_deposit(kv, &thb_deposit, d1)
        .await
        .map_err(AppError::Internal)?;

    // Update deposit status
    if let Some(mut status) = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
    {
        status.verified = body.approved;
        status.rejected = !body.approved;
        event_store::save_deposit_status(kv, &status, d1)
            .await
            .map_err(AppError::Internal)?;
    }

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        approved = body.approved,
        verifier_fingerprint = %state.log_fingerprint(&claims.email),
        "THB deposit slip verified"
    );

    // Write deposit columns (N=method, O=amount, Q=verified) to Google Sheet.
    // Mirrors D1 for BOTH approval and rejection so the sheet stays in sync.
    // QR auto-generation only fires on approval.
    let deposit_amount_thb = thb_deposit.amount_thb.to_string();
    if let (Ok(mapping), Ok(Some(attendee))) = (
        crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await,
        crate::sheets::get_attendee_by_id(
            &body.attendee_id,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
        )
        .await,
    ) {
        if let Some(wctx) = &state.worker_ctx {
            // Detach deposit verification write (fires for approve AND reject)
            wctx.wait_until(crate::sheets::bg_sync::write_deposit_verification(
                state.clone(),
                SheetRow::of(attendee.api_id.clone()),
                "THB".to_string(),
                deposit_amount_thb.clone(),
                body.approved,
                mapping.clone(),
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                Some(kv.clone()),
            ));
        } else {
            // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
            let ctx = crate::sheets::write::SheetContext {
                mapping: &mapping,
                state: &state,
                sheet_id: &event.sheet_id,
                sheet_name: &event.sheet_name,
                kv: Some(kv),
            };

            if let Err(e) = crate::sheets::write::write_deposit_verification(
                SheetRow::of(attendee.api_id.clone()),
                "THB",
                &deposit_amount_thb,
                body.approved,
                &ctx,
            )
            .await
            {
                tracing::warn!(error = %e, "failed to write deposit verification to sheet (non-fatal)");
            }
        }

        // Approval is what admits the attendee: the ticket QR is issued here and
        // nowhere else on this path. One call for both branches above — the
        // helper picks `wait_until` or a blocking write itself, which is why the
        // QR block is no longer written out twice (see `admit.rs`).
        //
        // The comp action calls the SAME helper, so an attendee let in without a
        // refund being owed gets an identical ticket (`.issues/129` Gap 1).
        if body.approved {
            super::admit::issue_ticket_qr_if_absent(&state, kv, &event, &attendee, &mapping).await;
        }
    }

    let msg = if body.approved {
        "deposit verified"
    } else {
        "deposit rejected"
    };

    // Dual-write to D1 — deposit verification (non-fatal, Phase 2a)
    if body.approved
        && let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::verify_deposit(
            d1,
            &body.attendee_id,
            "verified",
            "THB",
            0, // THB amount tracked in KV, not USDC
            &Utc::now().to_rfc3339(),
            &claims.email,
        )
        .await
    {
        tracing::warn!(
            attendee_id = %body.attendee_id,
            error = %e,
            "D1 deposit verify failed (non-fatal)"
        );
    }

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
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "message": msg
    })))
}
