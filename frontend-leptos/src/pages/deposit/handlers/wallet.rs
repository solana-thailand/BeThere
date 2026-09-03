//! Connect wallet (deposit flow).

use leptos::prelude::*;

use crate::components::{self as app_components, ToastType};

use crate::pages::deposit::js_interop;
use crate::pages::deposit::types::*;


// ---------------------------------------------------------------------------
// Connect wallet (deposit flow)
// ---------------------------------------------------------------------------

/// Create handler: connect a wallet for the deposit payment flow.
pub fn make_connect_wallet(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::ChoosePayment(d) => d.clone(),
            _ => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match js_interop::connect_wallet(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!(
                        "[deposit] wallet connected: {} ({})",
                        wallet_name_clone,
                        pubkey
                    );
                    set_state.set(DepositPageState::WalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] wallet connect error: code={:?} msg={}",
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
