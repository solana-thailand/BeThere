//! Pay-by-QR (USDC).

use leptos::prelude::*;

use crate::api::{
    self, UsdcDepositRequest,
};
use crate::components::{self as app_components, ToastType};

use crate::pages::deposit::types::*;


// ---------------------------------------------------------------------------
// Pay USDC via QR
// ---------------------------------------------------------------------------

/// Create handler: initiate USDC QR payment (no wallet needed).
pub fn make_pay_usdc_qr(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    wallet_input: ReadSignal<String>,
    params: DepositParamsSignal,
) -> impl Fn() + Clone + Send + Sync + 'static {
    move || {
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                let aid = match params.get() {
                    Ok(p) => p.attendee_id.unwrap_or_default(),
                    Err(_) => return,
                };
                let eid = extract_event_id_from_url();
                (d.clone(), aid, eid)
            }
            _ => return,
        };

        let wallet = wallet_input.get();
        if wallet.trim().is_empty() {
            app_components::show_toast(
                &set_toast,
                "Please enter your Solana wallet address.",
                ToastType::Warning,
            );
            return;
        }

        let deposit_data_for_set = deposit_data.clone();
        leptos::task::spawn_local(async move {
            let body = UsdcDepositRequest {
                event_id: event_id.unwrap_or_default(),
                attendee_id,
                wallet_address: wallet,
            };
            match api::deposit_usdc(&body).await {
                Ok(resp) => {
                    log::info!("[deposit] USDC QR deposit initiated");
                    set_state.set(DepositPageState::UsdcQrReady(
                        deposit_data_for_set,
                        resp.solana_pay_url,
                    ));
                }
                Err(e) => {
                    log::error!("[deposit] USDC deposit failed: {e}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Failed to initiate USDC payment: {e}"),
                        ToastType::Error,
                    );
                }
            }
        });
    }
}
