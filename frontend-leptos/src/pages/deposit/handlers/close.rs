//! Close-deposit wallet connect and close.

use leptos::prelude::*;

use crate::api::{
    self, CloseDepositRequest,
};
use crate::components::{self as app_components, ToastType};

use crate::pages::deposit::js_interop;
use crate::pages::deposit::types::*;


// ---------------------------------------------------------------------------
// Close deposit: connect wallet
// ---------------------------------------------------------------------------

/// Create handler: connect wallet for the close-deposit (reclaim rent) flow.
pub fn make_close_deposit_connect_wallet(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::CloseDepositChooseWallet(d) => Some(d.clone()),
            _ => None,
        };
        let deposit_data = match deposit_data {
            Some(d) => d,
            None => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match js_interop::connect_wallet(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!(
                        "[deposit] close-deposit wallet connected: {} ({})",
                        wallet_name_clone,
                        pubkey
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] close-deposit wallet connect error: code={:?} msg={}",
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
                    app_components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Close deposit (reclaim rent)
// ---------------------------------------------------------------------------

/// Create handler: build, sign, and send a close-deposit transaction.
pub fn make_close_deposit(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    params: DepositParamsSignal,
) -> impl Fn(String, String) + Clone + Send + Sync + 'static {
    move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::CloseDepositWalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let event_id = extract_event_id_from_url().unwrap_or_default();

        let wallet_name_for_tx = wallet_name.clone();
        let pk_for_tx = public_key.clone();
        let deposit_data_for_state = deposit_data.clone();

        set_state.set(DepositPageState::CloseDepositSigning(
            deposit_data.clone(),
            wallet_name.clone(),
            public_key.clone(),
        ));

        leptos::task::spawn_local(async move {
            let req = CloseDepositRequest {
                event_id: event_id.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_tx.clone(),
            };
            let close_resp = match api::close_deposit(&req).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] close-deposit TX build failed: {e}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Failed to build close-deposit transaction: {e}"),
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                    return;
                }
            };

            let tx_b64 = close_resp.transaction;
            if tx_b64.is_empty() {
                log::error!("[deposit] close-deposit TX is empty");
                app_components::show_toast(
                    &set_toast,
                    "Close-deposit transaction was empty. Please try again later.",
                    ToastType::Error,
                );
                set_state.set(DepositPageState::CloseDepositWalletConnected(
                    deposit_data,
                    wallet_name_for_tx,
                    pk_for_tx,
                ));
                return;
            }

            // SEC-014
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) =
                crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await
            {
                log::error!("[deposit] cluster mismatch (close): {cluster_err}");
                app_components::show_toast(&set_toast, &cluster_err, ToastType::Error);
                return;
            }

            match crate::pages::escrow_init::simulate_transaction_js(
                &wallet_name_for_tx,
                &tx_b64,
            )
            .await
            {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim
                        .error
                        .unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] close simulation failed: {err_msg}");
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
                    log::info!(
                        "[deposit] close-deposit TX sent, signature: {signature}",
                    );
                    set_state.set(DepositPageState::CloseDepositConfirmed(
                        deposit_data,
                        signature,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] close-deposit wallet sign+send error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    log::error!("[deposit] close-deposit wallet sign+send failed");
                    app_components::show_toast(
                        &set_toast,
                        "Close-deposit transaction failed. Please try again.",
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                }
            }
        });
    }
}
