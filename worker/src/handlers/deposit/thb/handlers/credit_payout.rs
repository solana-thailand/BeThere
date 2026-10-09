//! Helpers for paying held credit back out (`.issues/190`): who may see and
//! clear a payout request, where the organizer's transfer slip is stored, and
//! the audit entry that records who paid.
//!
//! The handlers live in `hold_refund_request.rs` (the attendee's request and
//! the organizer clearing it) and `organizer_credit_payout.rs` (the organizer
//! paying to the deposit account without a request, `.issues/192`). Both pay
//! through [`settle_payout`], so the guard, the slip and the audit entry are
//! the same whichever side started it.

use event_checkin_domain::models::credit_payout::{
    PaidAmounts, PayoutScope, payout_mismatch_message,
};
use event_checkin_domain::models::error::AppError;
use serde::Serialize;

use crate::auth::UserRole;
use crate::db::credit_ledger::{CreditBucket, RefundOutcome};
use crate::state::AppState;

/// Who started a payout. Recorded in the audit entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PayoutInitiator {
    /// The attendee asked (`request-credit-refund`); the organizer cleared it.
    Attendee,
    /// The organizer paid to the account from the attendee's deposit with no
    /// request open (`organizer-credit-payout`).
    Organizer,
}

/// Ledger note on every payout reversal row.
const PAYOUT_NOTE: &str = "held-credit payout processed by organizer";

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
    Ok(crate::storage::credit_payout_url(&key))
}

/// Record who paid a held-credit payout, how much, and the slip — the
/// "audit entry deferred" of `request_credit_refund_handler`, now written at
/// the moment that moves money. Global audit log (`__global__`): the payout is
/// cross-event. The actor is the staff email, as in every other audit entry;
/// the audit log is the access-controlled record that holds identities.
#[allow(clippy::too_many_arguments)]
pub(super) async fn audit_payout(
    state: &AppState,
    staff_email: &str,
    contact_email: &str,
    paid: PaidAmounts,
    requested_at: &str,
    proof: Option<&str>,
    initiator: PayoutInitiator,
    organizations: &[&str],
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
            "initiated_by": initiator,
            // Who may see the payout in the history (`credit_payout_history`).
            "organizations": organizations,
        }),
    );
    crate::audit_store::append_global_audit(kv, entry, state.d1.as_deref()).await
}

/// What is payable now: the person's positive buckets summed per currency.
pub(super) fn payable_of(buckets: &[CreditBucket]) -> PaidAmounts {
    let mut payable = PaidAmounts::default();
    for bucket in buckets {
        payable.add(&bucket.currency, bucket.balance);
    }
    payable
}

/// Every bucket the person holds, across orgs and currencies (the payout is
/// against the whole rolling balance, plan 022 §6), refused unless the
/// caller's scope covers every organization in it.
pub(super) async fn scoped_buckets(
    db: &worker::D1Database,
    scope: &PayoutScope,
    email: &str,
) -> Result<Vec<CreditBucket>, AppError> {
    let buckets = crate::db::credit_ledger::positive_balances(db, email)
        .await
        .map_err(AppError::Internal)?;
    match scope.covers(buckets.iter().map(|b| b.organization_id.as_str())) {
        true => Ok(buckets),
        false => Err(AppError::Forbidden(
            "this credit belongs to an organization you do not run".to_string(),
        )),
    }
}

/// One payout as the organizer confirmed it: `paid` against what is payable
/// in `buckets`, then — when there is money to move — the slip, the guarded
/// reversal and the audit entry, in that order.
///
/// - The read comparison is the fast path for the message; the guarded write
///   (`credit_ledger::try_refund`) re-checks it in the same statement as the
///   reversal, which is what makes it safe. A mismatch at either point is a
///   409 and writes nothing to the ledger.
/// - `key_at` keys the reversal (`refund:{email}:{key_at}:…`): a retry with
///   the same key is a no-op once the reversal has landed.
/// - Any failure returns before the caller clears the request or deletes the
///   account: a closed request with an unreversed ledger is a double payout.
#[allow(clippy::too_many_arguments)]
pub(super) async fn settle_payout(
    state: &AppState,
    db: &worker::D1Database,
    staff_email: &str,
    email: &str,
    key_at: &str,
    paid: PaidAmounts,
    proof: Option<&str>,
    buckets: &[CreditBucket],
    initiator: PayoutInitiator,
) -> Result<(), AppError> {
    let target_fingerprint = state.log_fingerprint(email);
    let payable = payable_of(buckets);
    if paid != payable {
        tracing::warn!(
            staff_fingerprint = %state.log_fingerprint(staff_email),
            target_fingerprint = %target_fingerprint,
            confirmed = %paid,
            payable = %payable,
            initiator = ?initiator,
            "refused credit payout — confirmed amount is stale"
        );
        return Err(AppError::Conflict(payout_mismatch_message(paid, payable)));
    }
    if paid.is_zero() {
        return Ok(());
    }

    // The slip names the org of the first bucket (sorted); a payout spanning
    // orgs is rare and the slip covers all of it.
    let org = match buckets.first() {
        Some(bucket) => bucket.organization_id.as_str(),
        None => "",
    };
    let proof = match proof.map(str::trim) {
        Some(data_url) if !data_url.is_empty() => {
            Some(store_payout_proof(state, org, email, key_at, data_url).await?)
        }
        _ => None,
    };

    let outcome = crate::db::credit_ledger::try_refund(db, email, key_at, paid, PAYOUT_NOTE)
        .await
        .map_err(AppError::Internal)?;
    tracing::info!(
        target_fingerprint = %target_fingerprint,
        thb = paid.thb,
        usdc = paid.usdc,
        outcome = ?outcome,
        initiator = ?initiator,
        "held credit payout reversal"
    );
    if outcome == RefundOutcome::Mismatch {
        // The balance moved between the read and the write.
        let now = crate::db::credit_ledger::positive_balances(db, email)
            .await
            .map_err(AppError::Internal)?;
        return Err(AppError::Conflict(payout_mismatch_message(
            paid,
            payable_of(&now),
        )));
    }

    // Who paid is part of the payout: a failure aborts before the request
    // clears, and the retry re-audits (the reversal is already a no-op).
    let mut organizations: Vec<&str> = buckets.iter().map(|b| b.organization_id.as_str()).collect();
    organizations.sort_unstable();
    organizations.dedup();
    audit_payout(
        state,
        staff_email,
        email,
        paid,
        key_at,
        proof.as_deref(),
        initiator,
        &organizations,
    )
    .await
    .map_err(AppError::Internal)
}
