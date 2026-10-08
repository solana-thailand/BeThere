//! Helpers for paying held credit back out (`.issues/190`): who may see and
//! clear a payout request, where the organizer's transfer slip is stored, and
//! the audit entry that records who paid.
//!
//! The handlers live in `hold_refund_request.rs`; this module keeps them under
//! the file-size limit and keeps the I/O shapes in one place.

use event_checkin_domain::models::credit_payout::{PaidAmounts, PayoutScope};
use event_checkin_domain::models::error::AppError;

use crate::auth::UserRole;
use crate::state::AppState;

/// Which organizations' credit `email` may see in the payout queue and pay
/// out.
///
/// - super-admin: every organization;
/// - global admin/organizer (the staff sheet's role): the default organization
///   (`""`, which has no owners) plus every organization they own;
/// - anyone else: only organizations they own (`organizations.owner_emails`).
///
/// A per-event scanner owns nothing, so their scope is empty and the queue —
/// which carries attendees' account numbers — is refused. Before this the
/// queue was readable by every authenticated staff member.
pub(super) async fn payout_scope(state: &AppState, email: &str) -> Result<PayoutScope, AppError> {
    let role = crate::auth::resolve_user_role(email, state, None).await;
    if role == UserRole::SuperAdmin {
        return Ok(PayoutScope::All);
    }
    let mut orgs = std::collections::BTreeSet::new();
    if role == UserRole::Organizer {
        orgs.insert(String::new());
    }
    if let Some(db) = state.d1.as_deref() {
        // Fail closed: an unreadable org table must not widen anything, and
        // it must not silently narrow an owner to nothing either.
        let owned = crate::db::organizations::list_orgs(db)
            .await
            .map_err(AppError::Internal)?;
        orgs.extend(
            owned
                .into_iter()
                .filter(|org| {
                    org.owner_emails
                        .iter()
                        .any(|e| e.eq_ignore_ascii_case(email))
                })
                .map(|org| org.id),
        );
    }
    Ok(PayoutScope::Orgs(orgs))
}

/// Refuse a caller whose scope is empty with the one message both payout
/// endpoints use.
pub(super) fn require_payout_operator(scope: &PayoutScope) -> Result<(), AppError> {
    match scope.is_empty() {
        true => Err(AppError::Forbidden(
            "credit payouts are handled by the organizer".to_string(),
        )),
        false => Ok(()),
    }
}

/// Validate and store the organizer's transfer slip for one payout. Returns
/// the staff-only serving path.
///
/// Same rules as every other slip: an uploaded image (`data:` URL), JPEG / PNG
/// / WebP by its leading bytes, at most ~3 MB (`validate_slip_url`). Unlike
/// the deposit slip it is never kept as a data URL: no R2, or a failed put,
/// fails the clear before anything is recorded, so a payout is never audited
/// against a slip that was not stored.
pub(super) async fn store_payout_proof(
    state: &AppState,
    organization_id: &str,
    email: &str,
    requested_at: &str,
    data_url: &str,
) -> Result<String, AppError> {
    if !data_url.starts_with("data:") {
        return Err(AppError::Validation(
            "the transfer slip must be an uploaded image".to_string(),
        ));
    }
    super::slip_upload::validate_slip_url(data_url)?;
    let bucket = state.r2.as_ref().ok_or_else(|| {
        AppError::Internal("slip storage is not configured — nothing was recorded".to_string())
    })?;
    let payload = data_url
        .split_once(',')
        .map(|(_, data)| data.trim())
        .unwrap_or_default();
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| AppError::Validation("the transfer slip could not be read".to_string()))?;
    let kind = event_checkin_domain::image_kind::ImageKind::sniff(&bytes).ok_or_else(|| {
        AppError::Validation("the uploaded file is not a JPEG, PNG or WebP image".to_string())
    })?;
    let key = crate::storage::credit_payout_key(organization_id, email, requested_at);
    crate::storage::put_bytes(
        bucket,
        &format!("{key}.{}", kind.extension()),
        bytes,
        kind.mime(),
    )
    .await
    .map_err(|e| AppError::Internal(format!("transfer slip upload failed: {e}")))?;
    Ok(format!("/api/{}", key))
}

/// Record who paid a held-credit payout, how much, and the slip — the
/// "audit entry deferred" of `request_credit_refund_handler`, now written at
/// the moment that moves money. Global audit log (`__global__`): the payout is
/// cross-event. The actor is the staff email, as in every other audit entry;
/// the audit log is the access-controlled record that holds identities.
pub(super) async fn audit_payout(
    state: &AppState,
    staff_email: &str,
    contact_email: &str,
    paid: PaidAmounts,
    requested_at: &str,
    proof: Option<&str>,
) -> Result<(), String> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| "EVENTS KV not configured — payout not audited".to_string())?;
    let entry = crate::audit_store::create_entry_with_meta(
        staff_email,
        crate::audit_store::AuditAction::CreditRefundPaidOut,
        contact_email,
        &format!("held credit paid out: {paid}"),
        serde_json::json!({
            "thb": paid.thb,
            "usdc": paid.usdc,
            "requested_at": requested_at,
            "proof": proof,
        }),
    );
    crate::audit_store::append_global_audit(kv, entry, state.d1.as_deref()).await
}
