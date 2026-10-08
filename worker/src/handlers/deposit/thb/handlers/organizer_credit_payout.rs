//! Organizer-initiated held-credit payout (`.issues/192`).
//!
//!   GET  /api/deposit/credit-payout-candidates — payable credit holders whose
//!                                                 deposit account is on file
//!   POST /api/deposit/organizer-credit-payout   — pay one of them out
//!
//! Before this, held credit came back only when the attendee asked
//! (`request-credit-refund`). The attendee already gave a refund account with
//! their THB deposit (copied to `credit_refund_accounts` when the deposit was
//! held as credit), so the organizer can return the money without waiting for
//! a request. Three rules keep that safe:
//!
//! - **Only to the deposit account.** The account that speaks for the person
//!   must be the deposit-sourced one. An account the attendee typed (or a
//!   replacement of the deposit one) only exists with a request, and a request
//!   is paid from the queue.
//! - **Never over an open request.** If any email of the person has one, the
//!   attendee chose where to be paid; this endpoint refuses and points to the
//!   queue.
//! - **The slip is required.** No attendee asked, so the organizer's transfer
//!   slip is the attendee-facing record that the money was sent.
//!
//! The money moves through the same core as the queue
//! (`credit_payout::settle_payout`): the guarded reversal writes only if the
//! payable balance still equals what the organizer confirmed, so a double
//! click or a racing spend is a 409 with nothing reversed. No request flag is
//! opened, so there is nothing to roll back when it refuses; the reversal is
//! keyed `organizer-{epoch ms}` instead of a request timestamp.

use axum::{Extension, Json, extract::State};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::credit_payout::{AccountSource, PaidAmounts, PayoutScope};
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use super::credit_payout::{PayoutInitiator, payout_scope, require_payout_operator};
use crate::db::contacts::CreditRefundRequest;
use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

/// Response for the candidates list. Same row shape as the request queue
/// (`requested_at` empty, `age_hours` 0), so the staff page renders both with
/// one row component.
#[derive(Debug, Clone, Serialize)]
pub struct CreditPayoutCandidatesResponse {
    pub candidates: Vec<CreditRefundRequest>,
}

/// Lists people the caller may pay out without a request: payable credit,
/// deposit account on file, no open request. Organizers only, org-scoped like
/// the queue — a row is shown only when every organization the person's
/// credit belongs to is in the caller's scope, because it carries the full
/// account number.
#[worker::send]
pub async fn credit_payout_candidates_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<CreditPayoutCandidatesResponse>, WorkerError> {
    let scope = payout_scope(&state, &claims.email).await?;
    require_payout_operator(&scope)?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".to_string()))?;
    let candidates = crate::db::credit_refund_accounts::deposit_account_holders(db)
        .await
        .map_err(AppError::Internal)?
        .into_iter()
        .filter(|row| {
            scope == PayoutScope::All
                || row
                    .organization_ids()
                    .is_some_and(|orgs| scope.covers(orgs.iter().map(String::as_str)))
        })
        .collect();
    Ok(ApiOk::new(CreditPayoutCandidatesResponse { candidates }))
}

/// Body of an organizer-initiated payout.
#[derive(Debug, Clone, Deserialize)]
pub struct OrganizerCreditPayoutRequest {
    pub email: String,
    /// What the organizer transferred, per currency. Must equal the payable
    /// balance at the moment of the write.
    pub paid: PaidAmounts,
    /// The organizer's transfer slip as an uploaded image (`data:` URL).
    /// Required here: no attendee asked, so the slip is the record.
    #[serde(default)]
    pub proof: Option<String>,
}

/// Response for an organizer-initiated payout.
#[derive(Debug, Serialize)]
pub struct OrganizerCreditPayoutResponse {
    /// Always `true` on success — a consistent JSON shape.
    pub paid_out: bool,
    pub message: String,
}

/// The organizer pays a person's held credit to the account from their THB
/// deposit, without a request (see the module docs for the rules).
#[worker::send]
pub async fn organizer_credit_payout_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<OrganizerCreditPayoutRequest>,
) -> Result<ApiOk<OrganizerCreditPayoutResponse>, WorkerError> {
    let target_fingerprint = state.log_fingerprint(&body.email);
    tracing::info!(
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        target_fingerprint = %target_fingerprint,
        "organizer-initiated credit payout"
    );

    let scope = payout_scope(&state, &claims.email).await?;
    require_payout_operator(&scope)?;
    body.paid
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    if body.paid.is_zero() {
        return Err(AppError::Validation("there is no amount to pay out".to_string()).into());
    }
    let proof = match body.proof.as_deref().map(str::trim) {
        Some(data_url) if !data_url.is_empty() => data_url,
        _ => {
            return Err(AppError::Validation(
                "attach the transfer slip — the attendee did not ask for this payout".to_string(),
            )
            .into());
        }
    };

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".to_string()))?;

    if crate::db::credit_refund_accounts::open_request_for_person(db, &body.email)
        .await
        .map_err(AppError::Internal)?
    {
        return Err(AppError::Conflict(
            "this person has asked for their credit back — pay it from the request queue"
                .to_string(),
        )
        .into());
    }
    match crate::db::credit_refund_accounts::chosen_for_person(db, &body.email)
        .await
        .map_err(AppError::Internal)?
    {
        Some((_, AccountSource::Deposit, _)) => {}
        _ => {
            return Err(AppError::Conflict(
                "no deposit refund account on file for this person — they can request a \
                 refund from their ticket page"
                    .to_string(),
            )
            .into());
        }
    }

    let paid = body.paid;
    let buckets = super::credit_payout::scoped_buckets(db, &scope, &body.email).await?;
    let key_at = format!("organizer-{}", js_sys::Date::now() as i64);
    super::credit_payout::settle_payout(
        &state,
        db,
        &claims.email,
        &body.email,
        &key_at,
        paid,
        Some(proof),
        &buckets,
        PayoutInitiator::Organizer,
    )
    .await?;

    // Paid: nothing needs the account number any more. Best-effort — the
    // nightly purge deletes it once the person holds no credit.
    let email_lower = body.email.trim().to_lowercase();
    let deleted =
        match crate::db::credit_refund_accounts::delete_for_person_statement(db, &email_lower) {
            Ok(stmt) => stmt
                .run()
                .await
                .map(|_| ())
                .map_err(|e| format!("D1 credit_refund_accounts delete run: {e:?}")),
            Err(e) => Err(e),
        };
    if let Err(e) = deleted {
        tracing::warn!(
            target_fingerprint = %target_fingerprint,
            error = %e,
            "payout account delete failed — the nightly purge will remove it"
        );
    }

    Ok(ApiOk::new(OrganizerCreditPayoutResponse {
        paid_out: true,
        message: format!("Paid out {paid}."),
    }))
}
