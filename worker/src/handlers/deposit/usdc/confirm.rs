//! Background verification of a deposit TX reported by the webhook.

use crate::event_store;
use crate::state::AppState;

use super::rpc::verify_tx_with_signer;
use super::types::UpdateDepositSignatureRequest;

/// Background task: verify deposit TX on-chain and update status (H8).
///
/// Detached via `wait_until` so the webhook response returns immediately.
/// Owns all data — no borrows on the handler's stack.
pub(crate) async fn verify_and_confirm_deposit(
    state: &AppState,
    body: &UpdateDepositSignatureRequest,
) {
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let rpc_url = state.config.solana.full_rpc_url();

    // Load deposit status first to get the expected wallet for signer cross-check.
    // A single `verify_tx_with_signer` call (getTransaction) replaces the previous
    // two-step pattern (getSignatureStatuses pre-check + getTransaction) — same
    // security, half the RPC calls on the success path.
    match event_store::get_deposit_status_with_fallback(
        kv,
        d1,
        &body.event_id,
        &body.attendee_id,
    )
    .await
    {
        Ok(Some(mut deposit_status)) => {
            // Single RPC call via getTransaction: confirms the TX on-chain
            // AND cross-checks the signer against the expected attendee
            // wallet. Closes the impersonation gap (someone submitting
            // another user's TX signature) and makes the worker resilient
            // to missed web2 events: if the deposit record has a known
            // wallet and the on-chain TX is signed by that wallet, we
            // confidently flip the deposit to verified.
            let expected_wallet = deposit_status.wallet_address.as_deref();
            let signer_outcome =
                verify_tx_with_signer(&rpc_url, &body.tx_signature, expected_wallet).await;
            if !signer_outcome.is_confirmed_and_matched() {
                if signer_outcome.is_confirmed() {
                    tracing::warn!(
                        attendee_id = %body.attendee_id,
                        tx_signature = %body.tx_signature,
                        "USDC deposit TX confirmed on-chain but signer does not match expected wallet (background)"
                    );
                } else {
                    tracing::info!(
                        attendee_id = %body.attendee_id,
                        tx_signature = %body.tx_signature,
                        "USDC deposit not yet confirmed on-chain (background check)"
                    );
                }
                return;
            }

            // Backfill the wallet_address if missing (older record or web2
            // hiccup at deposit creation time). Future refunds/check-ins
            // depend on having the depositor's wallet on hand.
            if deposit_status.wallet_address.as_deref().is_none_or(|w| w.is_empty())
                && let Some(signer) = signer_outcome.signer()
            {
                deposit_status.wallet_address = Some(signer.to_string());
            }

            deposit_status.verified = true;
            if let Err(e) =
                event_store::save_deposit_status_with_fallback(kv, d1, &deposit_status).await
            {
                tracing::error!(
                    attendee_id = %body.attendee_id,
                    error = %e,
                    "failed to save verified deposit status in background"
                );
                return;
            }

            tracing::info!(
                attendee_id = %body.attendee_id,
                tx_signature = %body.tx_signature,
                "USDC deposit verified in background"
            );

            // D1 dual-write for verified deposit
            if let Some(db) = d1
                && let Err(e) = crate::db::attendees::verify_deposit(
                    db,
                    &body.attendee_id,
                    "verified",
                    &body.tx_signature,
                    deposit_status.amount as i64,
                    &chrono::Utc::now().to_rfc3339(),
                    "usdc_on_chain_bg",
                )
                .await
            {
                tracing::warn!(
                    attendee_id = %body.attendee_id,
                    error = %e,
                    "D1 USDC deposit verify failed in background (non-fatal)"
                );
            }

            // Audit log
            if let Some(kv) = kv {
                let _ = crate::audit_store::append_event_audit(
                    kv,
                    &body.event_id,
                    crate::audit_store::create_entry_with_meta(
                        "system",
                        crate::audit_store::AuditAction::DepositConfirmed,
                        &body.attendee_id,
                        "USDC deposit confirmed on-chain (background)",
                        serde_json::json!({
                            "tx_signature": body.tx_signature,
                            "confirmed": true,
                        }),
                    ),
                    state.d1.as_deref(),
                )
                .await;
            }

            // Write deposit columns to sheet + auto-generate QR (non-blocking)
            let deposit_amount_str = deposit_status.amount.to_string();
            // Can't collapse: inner lookups depend on event_config bound by this pattern
            #[allow(clippy::collapsible_if)]
            if let Ok(Some(event_config)) =
                event_store::get_event_config_with_fallback(kv, d1, &body.event_id).await
            {
                let (mapping_result, attendee_result) = futures_util::join!(
                    crate::sheets::get_column_mapping(
                        state,
                        &event_config.sheet_id,
                        &event_config.sheet_name,
                        kv,
                    ),
                    crate::sheets::get_attendee_by_id(
                        &body.attendee_id,
                        state,
                        &event_config.sheet_id,
                        &event_config.sheet_name,
                        kv,
                    )
                );
                if let (Ok(mapping), Ok(Some(attendee))) = (mapping_result, attendee_result) {
                    let ctx = crate::sheets::write::SheetContext {
                        mapping: &mapping,
                        state,
                        sheet_id: &event_config.sheet_id,
                        sheet_name: &event_config.sheet_name,
                        kv,
                    };

                    if let Err(e) = crate::sheets::write::write_deposit_verification(
                        attendee.row_index,
                        "USDC",
                        &deposit_amount_str,
                        true,
                        &ctx,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "failed to write deposit verification to sheet in background");
                    }
                    if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                        let server_url = &state.config.server.url;
                        let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);

                        // D1 write — inline so the ticket page sees the QR immediately.
                        if let Some(ref d1) = state.d1
                            && let Err(e) =
                                crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
                        {
                            tracing::warn!(
                                attendee_id = %attendee.api_id,
                                error = %e,
                                "D1 set_qr_url failed on USDC verify_and_confirm (non-fatal)"
                            );
                        }

                        if let Err(e) = crate::sheets::write::update_qr_urls(
                            &[(attendee.row_index, qr_url)],
                            &mapping,
                            state,
                            &event_config.sheet_id,
                            &event_config.sheet_name,
                            kv,
                        )
                        .await
                        {
                            tracing::warn!(error = %e, "failed to auto-generate QR in background");
                        }
                    }
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                attendee_id = %body.attendee_id,
                "deposit record disappeared before background verification"
            );
        }
        Err(e) => {
            tracing::error!(
                attendee_id = %body.attendee_id,
                error = %e,
                "failed to reload deposit status for background verification"
            );
        }
    }
}
