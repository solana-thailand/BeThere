//! Send deposit transaction.

use leptos::prelude::*;

use crate::api::{self, UsdcDepositRequest};
use crate::components::{self as app_components, ToastType};

use crate::pages::deposit::js_interop;
use crate::pages::deposit::types::*;

// ---------------------------------------------------------------------------
// Send deposit TX
// ---------------------------------------------------------------------------

/// Create handler: initiate and send a USDC deposit transaction.
pub fn make_send_deposit(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    params: DepositParamsSignal,
) -> impl Fn(String, String) + Clone + Send + Sync + 'static {
    move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::WalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let event_id = extract_event_id_from_url();

        let pk_for_api = public_key.clone();
        let wallet_name_for_tx = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        let event_id_str = event_id.unwrap_or_default();

        leptos::task::spawn_local(async move {
            let body = UsdcDepositRequest {
                event_id: event_id_str.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_api,
            };
            let deposit_resp = match api::deposit_usdc(&body).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] USDC deposit initiate failed: {e}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Failed to initiate deposit: {e}"),
                        ToastType::Error,
                    );
                    return;
                }
            };

            let callback_url = if deposit_resp.solana_pay_url.starts_with("solana:") {
                &deposit_resp.solana_pay_url[7..]
            } else {
                &deposit_resp.solana_pay_url
            };

            let tx_b64 = match js_interop::fetch_tx_from_callback(callback_url).await {
                Some(tx) => tx,
                None => {
                    log::error!("[deposit] failed to fetch TX from callback");
                    app_components::show_toast(
                        &set_toast,
                        "Failed to build deposit transaction. Please try again.",
                        ToastType::Error,
                    );
                    return;
                }
            };

            // SEC-014: Verify wallet cluster matches expected network
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) = crate::pages::escrow_init::check_wallet_cluster(
                &wallet_name_for_tx,
                &expected_cluster,
            )
            .await
            {
                log::error!("[deposit] cluster mismatch: {cluster_err}");
                app_components::show_toast(&set_toast, &cluster_err, ToastType::Error);
                return;
            }

            match crate::pages::escrow_init::simulate_transaction_js(&wallet_name_for_tx, &tx_b64)
                .await
            {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] simulation failed: {err_msg}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Transaction would fail: {err_msg}"),
                        ToastType::Error,
                    );
                    return;
                }
                Err(e) => {
                    log::warn!("[deposit] simulate error (not blocking): {e}");
                }
            }

            match js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[deposit] TX sent, signature: {signature}");

                    let _ = api::record_deposit_tx(&event_id_str, &attendee_id, &signature).await;

                    set_state.set(DepositPageState::AwaitingConfirmation(
                        deposit_data_for_state.clone(),
                        wallet_name_for_tx.clone(),
                        signature.clone(),
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] wallet sign+send error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    log::error!("[deposit] wallet sign+send failed");
                    app_components::show_toast(
                        &set_toast,
                        "Transaction failed. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    }
}
