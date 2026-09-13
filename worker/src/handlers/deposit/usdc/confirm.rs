//! Background verification of a deposit TX reported by the webhook.

use crate::event_store;
use crate::state::AppState;

use super::recover::recover_and_verify_deposit;
use super::types::UpdateDepositSignatureRequest;

/// Background task: verify a webhook-reported deposit TX and update status (H8).
///
/// Detached via `wait_until` so the webhook response returns immediately.
/// Owns all data — no borrows on the handler's stack.
///
/// **This delegates to [`recover_and_verify_deposit`] rather than running its
/// own verification.** Both are the same state transition — an unverified
/// `DepositStatus` becoming `verified` — and this path previously carried a
/// weaker copy of it: a signer cross-check and nothing else. The read path had
/// since grown two guards this one never received:
///
/// - **F1** — bind verification to the on-chain `AttendeeDeposit` PDA. The
///   signer cross-check only proves the wallet signed *a* confirmed tx, so
///   without F1 a self-transfer by the right fee-payer earned a free verified
///   ticket. Worse here: when the deposit record carried no `wallet_address`,
///   `verify_tx_with_signer` treats "no expected wallet" as a match, so *any*
///   confirmed signature on the cluster verified the deposit.
/// - **Guard 2** — refuse when the wallet or signature is already bound to a
///   different `attendee_id` (plan 003, double-registration defence).
///
/// Keeping one implementation is what stops the two from drifting apart again.
pub(crate) async fn verify_and_confirm_deposit(
    state: &AppState,
    body: &UpdateDepositSignatureRequest,
) {
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    // The event config is required, not optional: F1 needs `organizer_wallet`,
    // `on_chain_event_id` and `deposit_amount_usdc` to check the PDA. Fail
    // closed — an unloadable config means we cannot prove the deposit is real.
    let event = match event_store::get_event_config_with_fallback(kv, d1, &body.event_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
            tracing::warn!(
                attendee_id = %body.attendee_id,
                event_id = %body.event_id,
                "event config not found — refusing background deposit verification (fail-closed)"
            );
            return;
        }
        Err(e) => {
            tracing::error!(
                attendee_id = %body.attendee_id,
                event_id = %body.event_id,
                error = %e,
                "failed to load event config for background verification (fail-closed)"
            );
            return;
        }
    };

    let status = match event_store::get_deposit_status_with_fallback(
        kv,
        d1,
        &body.event_id,
        &body.attendee_id,
    )
    .await
    {
        Ok(Some(status)) => status,
        Ok(None) => {
            tracing::warn!(
                attendee_id = %body.attendee_id,
                "deposit record disappeared before background verification"
            );
            return;
        }
        Err(e) => {
            tracing::error!(
                attendee_id = %body.attendee_id,
                error = %e,
                "failed to reload deposit status for background verification"
            );
            return;
        }
    };

    let was_verified = status.verified;
    let status = recover_and_verify_deposit(state, &event, status).await;

    match (was_verified, status.verified) {
        (false, true) => tracing::info!(
            attendee_id = %body.attendee_id,
            tx_signature_fingerprint = %state.log_fingerprint(&body.tx_signature),
            "USDC deposit verified in background"
        ),
        (false, false) => tracing::info!(
            attendee_id = %body.attendee_id,
            tx_signature_fingerprint = %state.log_fingerprint(&body.tx_signature),
            "USDC deposit not verified in background — not yet confirmed, or a guard refused"
        ),
        (true, _) => tracing::debug!(
            attendee_id = %body.attendee_id,
            "deposit already verified before background check — no-op"
        ),
    }
}
