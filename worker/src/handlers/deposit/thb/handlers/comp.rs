//! Admitting an attendee without owing them a refund (`.issues/129` Gap 1).
//!
//! Approving a payment slip is the only way an attendee receives their ticket
//! QR, and approving is simultaneously the promise to refund their ฿500. An
//! organizer who knows somebody did not really pay therefore had two moves:
//! admit them and owe ฿500, or refuse them entry. Neither is what they want, so
//! in practice they approved everyone and kept a mental list of who not to pay
//! back — which is why refunds still require them to be present, working down
//! the list from memory.
//!
//! This is the third move. It admits the attendee — same ticket QR, issued by
//! the same helper approval uses — and records the deposit as
//! [`DepositSource::Comp`], which every refund, hold and roll rule already
//! honours through `is_non_cash()`. Nothing is paid out, because nothing was
//! taken in.
//!
//! It is deliberately NOT a rejection. Rejecting tells the attendee their slip
//! was refused and leaves them outside the event. Comping is for the case where
//! the organizer would rather let somebody in than argue about ฿500 at the
//! door, and wants the system — not their memory — to hold the consequence.

use axum::{Extension, Json, extract::State};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::DepositSource;
use event_checkin_domain::models::error::AppError;
use serde::Deserialize;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::state::AppState;

/// Request body for `POST /api/deposit/thb/comp`.
#[derive(Debug, Deserialize)]
pub struct CompDepositRequest {
    pub event_id: String,
    pub attendee_id: String,
    /// Why this deposit is being written off. Free text, stored only in the
    /// audit entry — never shown to the attendee, who is told nothing beyond
    /// receiving their ticket.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Admit an attendee and write off their deposit as a comp.
#[worker::send]
pub async fn comp_thb_deposit_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CompDepositRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    let mut deposit = event_store::get_thb_deposit(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "no THB deposit for attendee '{}' in event '{}' — a comp reclassifies an \
                 existing deposit; an attendee who never submitted one is comped at signup",
                body.attendee_id, event.id
            ))
        })?;

    // ── Guards ──────────────────────────────────────────────────────────────
    // Each of these is a case where comping would make an existing money fact
    // untrue, rather than merely deciding a pending one.

    if deposit.refunded {
        return Err(AppError::Validation(
            "this deposit has already been refunded — the cash has gone back out, so there \
             is nothing to write off"
                .to_string(),
        )
        .into());
    }
    if deposit.held_as_credit {
        return Err(AppError::Validation(
            "this deposit is already held as rolling credit — comping it would delete credit \
             the attendee can already spend. Settle the credit first."
                .to_string(),
        )
        .into());
    }
    match deposit.source() {
        // Already written off. Idempotent rather than an error: the organizer
        // pressing the button twice must not produce a scary message about an
        // outcome that is exactly what they wanted.
        DepositSource::Comp => {
            return Ok(ApiOk::new(serde_json::json!({
                "success": true,
                "message": "already comped — no refund is owed",
                "already": true,
            })));
        }
        // A credit-covered deposit is ALREADY non-cash: the ฿500 came out of the
        // attendee's rolling balance, which was debited when it was applied.
        // Comping it would silently destroy that balance entry — the attendee
        // would have paid for this event out of credit they no longer have.
        DepositSource::Credit => {
            return Err(AppError::Validation(
                "this deposit was covered by the attendee's rolling credit, not cash — there \
                 is no payment to write off, and comping it would consume credit they are \
                 still owed"
                    .to_string(),
            )
            .into());
        }
        DepositSource::Cash => {}
    }

    // An approved deposit is ฿ the organizer has promised back, and it sits in
    // the refund queue. Writing that off is a decision about real money after
    // the fact, so it must say why (the audit entry carries the reason). A
    // pending slip's comp is the admit-at-the-door case and stays optional.
    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    if deposit.verified && reason.is_none() {
        return Err(AppError::Validation(
            "a reason is required to write off an approved deposit".to_string(),
        )
        .into());
    }

    // ── Write ───────────────────────────────────────────────────────────────
    let now = Utc::now().to_rfc3339();
    deposit.deposit_source = Some(DepositSource::Comp);
    // Verified so every attendee-facing reader treats them as admitted; the
    // source is what keeps them out of the refund queue. `slip_url` and
    // `amount_thb` are left exactly as uploaded — the record of what was
    // claimed is evidence, and the whole reason for the `deposit_source`
    // column is that writing this off no longer requires destroying it.
    deposit.verified = true;
    deposit.verified_by = Some(claims.email.clone());
    deposit.verified_at = Some(now.clone());

    event_store::save_thb_deposit(kv, &deposit, d1)
        .await
        .map_err(AppError::Internal)?;

    if let Some(mut status) = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
    {
        status.verified = true;
        status.rejected = false;
        // The refund queue reads this. A comp must never appear in it.
        status.refundable = false;
        event_store::save_deposit_status(kv, &status, d1)
            .await
            .map_err(AppError::Internal)?;
    }

    // ── Admit ───────────────────────────────────────────────────────────────
    // The same helper the approval path calls, so a comped attendee's ticket is
    // indistinguishable from anyone else's. Being let in is the point; the
    // deposit classification is the organizer's business, not the attendee's.
    if let (Ok(mapping), Ok(Some(attendee))) = (
        crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await,
        crate::sheets::get_attendee_by_id(&body.attendee_id, &state, &event, Some(kv)).await,
    ) {
        super::admit::issue_ticket_qr_if_absent(&state, kv, &event, &attendee, &mapping).await;
    } else {
        tracing::warn!(
            attendee_id = %body.attendee_id,
            event_id = %event.id,
            "comp recorded but the attendee row could not be read — no ticket QR was issued"
        );
    }

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        amount_thb = deposit.amount_thb,
        admin_fingerprint = %state.log_fingerprint(&claims.email),
        "THB deposit comped — attendee admitted, no refund owed"
    );

    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::DepositCompedByAdmin,
            &body.attendee_id,
            &format!(
                "admitted without a refundable deposit ({} THB written off){}",
                deposit.amount_thb,
                match reason {
                    Some(reason) => format!(": {reason}"),
                    None => String::new(),
                }
            ),
        ),
        d1,
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "message": "attendee admitted — no refund owed",
        "already": false,
    })))
}
