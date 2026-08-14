//! Admin-side hold-as-credit handlers — sibling of the attendee `hold_credit.rs`.
//!
//! Three endpoints (all admin/staff-authed via `resolve_event_with_access`):
//!
//!   GET  /api/refund/held                  — list deposits held as rolling credit
//!   POST /api/refund/hold/{attendee_id}    — admin marks a deposit as held
//!   GET  /api/deposit/credit-liability     — total credit held across all contacts
//!
//! ## Why a separate admin path?
//!
//! The attendee endpoint (`POST /api/deposit/hold`) is attendee-JWT-gated: it
//! checks `attendee.email == claims.email`, so only the attendee themselves can
//! trigger a hold. In practice an organizer often reaches an attendee over chat
//! / in-person and the attendee confirms they want to hold — but never actually
//! taps the button. Without an admin path that deposit sits in the Refund Queue
//! forever (or gets wrongly refunded). These handlers close that operational
//! gap (Issue #061 Phase 2 scoping decision (a) — admin credit visibility).
//!
//! ## Safety invariants (must match the attendee handler exactly)
//!
//! 1. **Settle BEFORE incrementing credit.** If settle succeeds but the credit
//!    write fails, no money is created and the deposit is marked held — an
//!    admin can reconcile. The reverse order permits infinite credit via retry.
//! 2. **Idempotency guards** on `refunded` and `held_as_credit` — a re-call
//!    returns `Validation(...)` without double-incrementing.
//! 3. **Credit the attendee's email, not the admin's.** The admin is acting on
//!    behalf of the attendee; the contact row that gets the credit is the
//!    attendee's, looked up from the event sheet.
//! 4. **THB-only by construction.** This loads `ThbDeposit` directly; USDC
//!    state lives on-chain (escrow) and has no `ThbDeposit` row. The attendee
//!    handler's explicit `DepositMethod::Thb` check is implicitly satisfied.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::ThbDeposit;
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// GET /api/refund/held (admin)
// ---------------------------------------------------------------------------

/// Response for the held-as-credit list endpoint.
/// Mirrors `RefundedListResponse` (slip_list.rs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeldListResponse {
    pub held: Vec<ThbDeposit>,
}

/// List all THB deposits held as rolling credit for an event.
///
/// Mirrors `refunded_list_handler` but filters on `held_as_credit = true`.
/// Used by the admin "Held as Credit" sub-tab in the deposit dashboard.
#[worker::send]
pub async fn held_list_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<HeldListResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    let mut held: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| d.held_as_credit)
        .collect();

    // Migrate any inline base64 slip/refund URLs to R2 (keeps payload small).
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut held).await;

    // Enrich with attendee names from Google Sheets.
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &held).await;
    let enriched: Vec<ThbDeposit> = held
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(HeldListResponse { held: enriched }))
}

// ---------------------------------------------------------------------------
// POST /api/refund/hold/{attendee_id} (admin)
// ---------------------------------------------------------------------------

/// Request body for the admin hold endpoint. Only `event_id` is needed — the
/// attendee is identified by the path parameter.
#[derive(Debug, Clone, Deserialize)]
pub struct AdminHoldRequest {
    pub event_id: String,
}

/// Admin marks a verified THB deposit as held-as-rolling-credit on behalf of
/// an attendee (attendee confirmed verbally / over chat but didn't tap the
/// button themselves).
///
/// Mirrors the attendee `hold_deposit_handler` (hold_credit.rs) with two
/// differences: admin auth (not attendee-JWT), and credits the attendee's
/// looked-up email (not `claims.email`). All financial invariants are
/// preserved — see the module-level doc.
#[worker::send]
pub async fn admin_hold_deposit_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(attendee_id): Path<String>,
    Json(body): Json<AdminHoldRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        attendee_id = %attendee_id,
        event_id = %body.event_id,
        admin_email = %claims.email,
        "admin hold deposit initiated"
    );

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    // 1. Admin access + event resolution (staff auth via resolve_event_with_access).
    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    // 2. Load attendee FIRST — the credit must be applied to the attendee's
    //    contact row, not the admin's. An attendee with no email cannot be
    //    credited (defensive — sheets rows always have an email column).
    let attendee = crate::sheets::get_attendee_by_id(
        &attendee_id,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        Some(kv),
    )
    .await
    .map_err(AppError::Internal)?
    .ok_or_else(|| AppError::NotFound(format!("attendee '{attendee_id}' not found")))?;

    let attendee_email = attendee.email.trim().to_lowercase();
    if attendee_email.is_empty() {
        return Err(AppError::Validation(
            "attendee has no email — cannot credit contact".to_string(),
        )
        .into());
    }

    // 3. Load the settleable THB record. THB-only by construction — USDC
    //    deposits have no ThbDeposit row (state lives on-chain in escrow).
    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("THB deposit record not found".to_string()))?;

    // 4. Idempotency guards — exact mirror of the attendee handler. A re-call
    //    (admin double-click, or admin holds after attendee already held)
    //    returns Validation without double-incrementing credit.
    if !thb_deposit.verified {
        return Err(
            AppError::Validation("deposit must be verified before holding".to_string()).into(),
        );
    }
    if thb_deposit.refunded {
        return Err(AppError::Validation("deposit already refunded".to_string()).into());
    }
    if thb_deposit.held_as_credit {
        return Err(AppError::Validation("deposit already held as credit".to_string()).into());
    }
    // MONEY-SAFETY: a rolling-credit application / staff comp was never funded
    // with cash. Holding it as credit would MINT credit from a deposit that was
    // itself created from credit (free ticket + restored balance).
    if thb_deposit.is_non_cash() {
        return Err(AppError::Validation(
            "this is a credit-covered / comp deposit — it cannot be held as rolling credit".to_string(),
        )
        .into());
    }

    // 5. Resolve per-org contacts sheet (same logic as the attendee handler).
    let resolved = if let Some(db) = state.d1.as_deref() {
        crate::org_store::resolve_contacts_sheet(db, &event, &state.config.sheets).await
    } else {
        event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: state.config.sheets.contacts_sheet_id.clone(),
            contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
            events_sheet_name: state.config.sheets.events_sheet_name.clone(),
        }
    };

    if resolved.sheet_id.is_empty() {
        return Err(AppError::Internal("contacts sheet not configured".to_string()).into());
    }

    // 6. Settle the source deposit BEFORE incrementing credit. Failure ordering
    //    is load-bearing: if settle succeeds but the credit increment fails, no
    //    money is created and the deposit is marked held — an admin can
    //    reconcile manually. The reverse order would permit infinite credit via
    //    retry. Mirrors mark_refund_handler + attendee hold_deposit_handler.
    //
    //    ATOMIC: single conditional D1 UPDATE (CAS) so an admin hold racing with
    //    the attendee's own hold (or another admin) can't double-increment credit.
    let held_amount = thb_deposit.amount_thb;
    let now = Utc::now().to_rfc3339();

    let settled = match d1 {
        Some(db) => {
            crate::db::thb_deposits::try_settle_hold_credit(db, &event.id, &attendee_id, &now)
                .await
                .map_err(AppError::Internal)?
        }
        None => true, // no D1 (tests/local) — non-atomic fallback
    };
    if !settled {
        return Err(AppError::Validation(
            "deposit already settled (held as credit or refunded)".to_string(),
        )
        .into());
    }

    thb_deposit.held_as_credit = true;
    thb_deposit.held_as_credit_at = Some(now);
    event_store::save_thb_deposit(kv, &thb_deposit, d1)
        .await
        .map_err(AppError::Internal)?;

    // 7. Increment credit on the ATTENDEE's contact row (not the admin's).
    crate::sheets::contacts::increment_credit(
        &state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        Some(kv),
        &attendee_email,
        "thb",
        held_amount,
    )
    .await
    .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %attendee_id,
        event_id = %event.id,
        amount = held_amount,
        attendee_email = %attendee_email,
        admin_email = %claims.email,
        "admin held deposit as credit"
    );

    // Audit (sibling of RefundMarked / attendee DepositHeldAsCredit). Non-fatal.
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::DepositHeldAsCredit,
            &attendee_id,
            &format!("admin held {held_amount} THB held as rolling credit for attendee"),
        ),
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "held_amount": held_amount,
        "message": format!("{held_amount} THB held as credit"),
    })))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/credit-liability (admin)
// ---------------------------------------------------------------------------

/// Total deposit-credit liability across all contacts — the organizer's
/// "Total credit held: X THB across N contacts" header chip (Issue #061 Phase 2
/// option a2). One D1 SUM/COUNT round-trip; degrades to zeros if D1 is
/// unavailable so the deposits view always renders.
#[worker::send]
pub async fn credit_liability_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<crate::db::contacts::CreditLiability>, WorkerError> {
    tracing::info!(admin_email = %claims.email, "credit liability requested");

    // Source of truth is the org-scoped credit ledger (the old path summed a D1
    // contacts column that hold never wrote, so it always read zero — masking a
    // ฿5,000 real liability). Fold the per-(org,currency) rows into the existing
    // CreditLiability shape so the admin chip's response is unchanged.
    let liability = match state.d1.as_deref() {
        Some(db) => match crate::db::credit_ledger::liability(db).await {
            Ok(rows) => {
                let mut out = crate::db::contacts::CreditLiability::default();
                for r in &rows {
                    if r.currency == "usdc" {
                        out.total_usdc += r.balance;
                    } else {
                        out.total_thb += r.balance;
                    }
                }
                out.contact_count = rows.iter().map(|r| r.holders).max().unwrap_or(0);
                out
            }
            Err(_) => crate::db::contacts::CreditLiability::default(),
        },
        // No D1 → cannot read credit state; degrade to zero rather than block
        // the deposits view.
        None => crate::db::contacts::CreditLiability::default(),
    };

    Ok(ApiOk::new(liability))
}

/// Request body for the admin apply-credit endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct AdminApplyCreditRequest {
    pub event_id: String,
}

/// Admin applies an attendee's rolling credit to COMPLETE a registration stuck
/// at the deposit step — a credit-holder who registered but never uploaded a
/// slip (e.g. registered before the auto-apply shipped). Server-side equivalent
/// of the registration auto-apply: atomically spends the credit from the ledger
/// (`try_spend`) and writes a credit-covered deposit so the attendee proceeds to
/// the ticket. Never creates money — spends only if the balance covers the
/// event's deposit, and is idempotent per `(event, email)`.
#[worker::send]
pub async fn admin_apply_credit_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(attendee_id): Path<String>,
    Json(body): Json<AdminApplyCreditRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        attendee_id = %attendee_id,
        event_id = %body.event_id,
        admin_email = %claims.email,
        "admin apply-credit initiated"
    );

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".to_string()))?;

    // Admin access + event resolution (staff auth via resolve_event_with_access).
    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;
    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    // Load attendee — credit applies to the attendee's email, not the admin's.
    let attendee =
        crate::sheets::get_attendee_by_id(&attendee_id, &state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("attendee '{attendee_id}' not found")))?;
    let email = attendee.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(AppError::Validation("attendee has no email".to_string()).into());
    }

    // Only for STUCK registrations: if a deposit already exists, there is nothing
    // to complete (and we must not create a second one).
    if event_store::get_thb_deposit(kv, &event.id, &attendee_id, Some(d1))
        .await
        .map_err(AppError::Internal)?
        .is_some()
    {
        return Err(
            AppError::Validation("attendee already has a deposit for this event".to_string()).into(),
        );
    }

    let required = event.deposit_amount_thb;
    if required == 0 {
        return Err(AppError::Validation("event has no THB deposit amount".to_string()).into());
    }

    // Atomic ledger spend (idempotent per apply:event:email). Spends only if the
    // balance covers the deposit — never creates money.
    let apply_key = format!("apply:{}:{}", event.id, email);
    let spent = crate::db::credit_ledger::try_spend(
        d1,
        &email,
        &event.organization_id,
        "thb",
        required as i64,
        &event.id,
        &apply_key,
    )
    .await
    .map_err(AppError::Internal)?;

    if !spent {
        let bal = crate::db::credit_ledger::balance(d1, &email, &event.organization_id, "thb")
            .await
            .unwrap_or(0)
            .max(0);
        return Err(AppError::Validation(format!(
            "insufficient rolling credit: balance ฿{bal} < required ฿{required}"
        ))
        .into());
    }

    // Write the credit-covered deposit (server-side) so the registration completes.
    let now = Utc::now().to_rfc3339();
    let covered = ThbDeposit {
        event_id: event.id.clone(),
        attendee_id: attendee_id.clone(),
        amount_thb: required,
        slip_url: Some("ROLLING_CREDIT_AUTO_APPLIED".to_string()),
        verified: true,
        verified_at: Some(now.clone()),
        verified_by: Some("SYSTEM_ROLLING_CREDIT".to_string()),
        uploaded_at: now,
        refunded: false,
        refunded_at: None,
        held_as_credit: false,
        held_as_credit_at: None,
        attendee_name: Some(attendee.name.clone()),
        bank_account: None,
        bank_name: None,
        account_name: None,
        refund_proof_url: None,
    };
    event_store::save_thb_deposit(kv, &covered, Some(d1))
        .await
        .map_err(AppError::Internal)?;

    let remaining = crate::db::credit_ledger::balance(d1, &email, &event.organization_id, "thb")
        .await
        .unwrap_or(0)
        .max(0);
    tracing::info!(%email, event_id = %event.id, applied = required, remaining, "admin applied rolling credit to complete registration");
    Ok(ApiOk::new(serde_json::json!({
        "applied_thb": required,
        "email": email,
        "remaining_credit_thb": remaining,
        "next_step": "ticket",
    })))
}
