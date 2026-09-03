//! GET /api/deposit/usdc/confirm — Poll for deposit TX confirmation

use axum::{extract::Query, extract::State};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::deposit::usdc::{
    ConfirmDepositQuery, ConfirmDepositResponse, VerifyWithSignerOutcome,
    discover_deposit_tx_on_chain, verify_tx_with_signer,
};
use crate::state::AppState;

/// Check if a USDC deposit has been confirmed on-chain.
///
/// This endpoint is polled by the frontend after the attendee sends the deposit TX.
/// It checks the KV-stored `DepositStatus` for the `verified` flag and
/// optionally calls the RPC to verify the TX landed.
#[worker::send]
pub async fn confirm_deposit_handler(
    State(state): State<AppState>,
    Query(query): Query<ConfirmDepositQuery>,
) -> Result<ApiOk<ConfirmDepositResponse>, WorkerError> {
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let event = event_store::get_event_config_with_fallback(kv, d1, &query.event_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?
        .ok_or_else(|| {
            event_checkin_domain::models::error::AppError::NotFound(format!(
                "event '{}' not found",
                query.event_id
            ))
        })?;

    // Check deposit status
    let deposit_status =
        event_store::get_deposit_status_with_fallback(kv, d1, &event.id, &query.attendee_id)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    match deposit_status {
        Some(status) if status.verified => {
            // Already verified
            Ok(ApiOk::new(ConfirmDepositResponse {
                confirmed: true,
                tx_signature: status.tx_signature.clone(),
                solana_pay_url: None,
            }))
        }
        Some(status) => {
            // Pending — check if there's a tx_signature to verify on-chain
            match &status.tx_signature {
                Some(sig) if !sig.is_empty() => {
                    // Verify the TX on-chain via RPC, cross-checking the signer
                    // against the expected attendee wallet when known. This
                    // closes the impersonation gap where a malicious user could
                    // submit someone else's TX signature to get verified, and
                    // also makes the worker resilient to missed web2 events:
                    // if the deposit record has a known wallet and the TX is
                    // confirmed and signed by that wallet, we mark it verified.
                    let rpc_url = state.config.solana.full_rpc_url();
                    let expected_wallet = status.wallet_address.as_deref();
                    let outcome = verify_tx_with_signer(&rpc_url, sig, expected_wallet).await;

                    if outcome.is_confirmed_and_matched() {
                        // Update the deposit status to verified. If the deposit
                        // record had no wallet_address (e.g., older record or a
                        // web2 hiccup at creation time), backfill it from the
                        // verified signer so future refunds/check-ins have it.
                        let mut updated = status.clone();
                        updated.verified = true;
                        if updated
                            .wallet_address
                            .as_deref()
                            .is_none_or(|w| w.is_empty())
                            && let Some(signer) = outcome.signer()
                        {
                            updated.wallet_address = Some(signer.to_string());
                        }
                        event_store::save_deposit_status_with_fallback(kv, d1, &updated)
                            .await
                            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

                        tracing::info!(
                            attendee_id = %query.attendee_id,
                            tx_signature = %sig,
                            "USDC deposit confirmed on-chain"
                        );

                        // Dual-write to D1 first (source of truth)
                        if let Some(db) = d1
                            && let Err(e) = crate::db::attendees::verify_deposit(
                                db,
                                &query.attendee_id,
                                "verified",
                                sig,
                                status.amount as i64,
                                &chrono::Utc::now().to_rfc3339(),
                                "usdc_on_chain",
                            )
                            .await
                        {
                            tracing::warn!(
                                attendee_id = %query.attendee_id,
                                error = %e,
                                "D1 USDC deposit verify failed (non-fatal)"
                            );
                        }

                        // Detach Google Sheets writes — response returns immediately (Phase 2c)
                        let deposit_amount_str = status.amount.to_string();
                        let (mapping_result, attendee_result) = futures_util::join!(
                            crate::sheets::get_column_mapping(
                                &state,
                                &event.sheet_id,
                                &event.sheet_name,
                                kv,
                            ),
                            crate::sheets::get_attendee_by_id(
                                &query.attendee_id,
                                &state,
                                &event.sheet_id,
                                &event.sheet_name,
                                kv,
                            )
                        );
                        if let (Ok(mapping), Ok(Some(attendee))) = (mapping_result, attendee_result)
                        {
                            if let Some(ctx) = &state.worker_ctx {
                                ctx.wait_until(crate::sheets::bg_sync::write_deposit_verification(
                                    state.clone(),
                                    attendee.row_index,
                                    "USDC".to_string(),
                                    deposit_amount_str.clone(),
                                    true,
                                    mapping.clone(),
                                    event.sheet_id.clone(),
                                    event.sheet_name.clone(),
                                    kv.cloned(),
                                ));

                                // Auto-generate QR if attendee doesn't have one.
                                // D1 write is inline (ticket page reads D1-first);
                                // Sheet write is detached.
                                if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                                    let server_url = &state.config.server.url;
                                    let qr_url =
                                        format!("{server_url}/staff/?scan={}", attendee.api_id);

                                    if let Some(ref d1) = state.d1
                                        && let Err(e) = crate::db::attendees::set_qr_url(
                                            d1,
                                            &attendee.api_id,
                                            &qr_url,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            attendee_id = %attendee.api_id,
                                            error = %e,
                                            "D1 set_qr_url failed on USDC confirm (non-fatal)"
                                        );
                                    }

                                    ctx.wait_until(crate::sheets::bg_sync::update_qr_urls(
                                        state.clone(),
                                        vec![(attendee.row_index, qr_url)],
                                        mapping,
                                        event.sheet_id.clone(),
                                        event.sheet_name.clone(),
                                        kv.cloned(),
                                    ));
                                }
                            } else {
                                // Fallback: blocking Sheets writes when worker_ctx unavailable (tests)
                                let ctx = crate::sheets::write::SheetContext {
                                    mapping: &mapping,
                                    state: &state,
                                    sheet_id: &event.sheet_id,
                                    sheet_name: &event.sheet_name,
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
                                    tracing::warn!(error = %e, "failed to write deposit verification to sheet (non-fatal)");
                                }

                                if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                                    let server_url = &state.config.server.url;
                                    let qr_url =
                                        format!("{server_url}/staff/?scan={}", attendee.api_id);

                                    if let Some(ref d1) = state.d1
                                        && let Err(e) = crate::db::attendees::set_qr_url(
                                            d1,
                                            &attendee.api_id,
                                            &qr_url,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            attendee_id = %attendee.api_id,
                                            error = %e,
                                            "D1 set_qr_url failed on USDC confirm (non-fatal)"
                                        );
                                    }

                                    if let Err(e) = crate::sheets::write::update_qr_urls(
                                        &[(attendee.row_index, qr_url)],
                                        &mapping,
                                        &state,
                                        &event.sheet_id,
                                        &event.sheet_name,
                                        kv,
                                    )
                                    .await
                                    {
                                        tracing::warn!(error = %e, "failed to auto-generate QR for verified attendee (non-fatal)");
                                    }
                                }
                            }
                        }

                        Ok(ApiOk::new(ConfirmDepositResponse {
                            confirmed: true,
                            tx_signature: Some(sig.clone()),
                            solana_pay_url: None,
                        }))
                    } else if outcome.is_confirmed() && !outcome.is_confirmed_and_matched() {
                        // TX is confirmed but the signer does NOT match the
                        // expected attendee wallet. This is suspicious — it
                        // could be a cross-wallet submission (e.g., attendee
                        // paid from a different wallet than declared) or an
                        // impersonation attempt. Treat it as not-verified so
                        // the attendee sees the pending state; the underlying
                        // mismatch is already logged at warn level inside
                        // `verify_tx_with_signer_impl`.
                        tracing::warn!(
                            attendee_id = %query.attendee_id,
                            tx_signature = %sig,
                            "TX confirmed on-chain but signer does not match expected wallet — refusing to verify"
                        );
                        Ok(ApiOk::new(ConfirmDepositResponse {
                            confirmed: false,
                            tx_signature: Some(sig.clone()),
                            solana_pay_url: None,
                        }))
                    } else {
                        // Not yet confirmed, keep polling. RpcError is treated
                        // identically to Pending here so the frontend retries
                        // instead of surfacing a 500; the underlying RPC failure
                        // is already logged at warn level inside `verify_tx_with_signer`.
                        if matches!(outcome, VerifyWithSignerOutcome::RpcError) {
                            tracing::warn!(
                                attendee_id = %query.attendee_id,
                                tx_signature = %sig,
                                "RPC error during deposit confirmation — returning pending for retry"
                            );
                        }
                        Ok(ApiOk::new(ConfirmDepositResponse {
                            confirmed: false,
                            tx_signature: Some(sig.clone()),
                            solana_pay_url: None,
                        }))
                    }
                }
                _ => {
                    // No TX signature recorded yet. Before falling back to the
                    // Solana Pay retry URL, try the on-chain recovery path: if
                    // the attendee has a wallet_address and the event has an
                    // escrow, derive the AttendeeDeposit PDA and look up its
                    // signature history. If the deposit TX is found on-chain
                    // (web2 missed the event but on-chain is done), save the
                    // signature so the next poll re-enters the normal
                    // verification path above with full signer cross-check +
                    // sheet write + QR generation.
                    let wallet_addr = status.wallet_address.as_deref().filter(|w| !w.is_empty());

                    let escrow_addr = if !event.escrow_address.is_empty() {
                        Some(event.escrow_address.clone())
                    } else if !event.organizer_wallet.is_empty() {
                        // Derive escrow address on the fly when not yet persisted.
                        let on_chain_event_id = if event.on_chain_event_id != 0 {
                            event.on_chain_event_id
                        } else {
                            crate::handlers::deposit::derive_on_chain_event_id(&event.id)
                        };
                        crate::solana_escrow::derive_escrow_address(
                            &event.organizer_wallet,
                            on_chain_event_id,
                        )
                        .await
                        .ok()
                    } else {
                        None
                    };

                    if let (Some(wallet), Some(escrow)) = (wallet_addr, escrow_addr) {
                        let rpc_url = state.config.solana.full_rpc_url();
                        if let Some(discovered_sig) =
                            discover_deposit_tx_on_chain(&rpc_url, &escrow, wallet).await
                        {
                            tracing::info!(
                                attendee_id = %query.attendee_id,
                                event_id = %event.id,
                                tx_signature = %discovered_sig,
                                "Recovered deposit TX signature via on-chain PDA discovery"
                            );
                            let mut updated = status.clone();
                            updated.tx_signature = Some(discovered_sig);
                            let _ =
                                event_store::save_deposit_status_with_fallback(kv, d1, &updated)
                                    .await;
                            // Return pending — the next poll will verify the
                            // now-recorded signature through the normal path
                            // (with sheet write + QR generation side effects).
                            return Ok(ApiOk::new(ConfirmDepositResponse {
                                confirmed: false,
                                tx_signature: updated.tx_signature.clone(),
                                solana_pay_url: None,
                            }));
                        }
                    }

                    // No signature found on-chain either — return Solana Pay
                    // URL so the frontend can retry the deposit flow.
                    let callback_url = format!(
                        "{}/api/deposit/usdc/tx?event_id={}&attendee_id={}&wallet=",
                        state.config.server.url,
                        urlencoding::encode(&event.id),
                        urlencoding::encode(&query.attendee_id),
                    );
                    Ok(ApiOk::new(ConfirmDepositResponse {
                        confirmed: false,
                        tx_signature: None,
                        solana_pay_url: Some(format!("solana:{callback_url}")),
                    }))
                }
            }
        }
        None => {
            // No deposit record — attendee hasn't initiated yet
            Ok(ApiOk::new(ConfirmDepositResponse {
                confirmed: false,
                tx_signature: None,
                solana_pay_url: None,
            }))
        }
    }
}
