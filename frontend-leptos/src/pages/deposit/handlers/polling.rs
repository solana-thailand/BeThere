//! Confirmation polling (shared config, outcome and handlers).

use leptos::prelude::*;

use crate::api::{self, ConfirmDepositResponse, DepositStatusResponse};
use crate::components::{self as app_components, ToastType};

use crate::pages::deposit::types::*;

// ---------------------------------------------------------------------------
// Shared confirmation polling
// ---------------------------------------------------------------------------

/// Polling configuration for the two deposit confirmation tiers.
///
/// - **Wallet TX**: 30 attempts × 2s = ~60s total
/// - **QR payment**: 100 attempts × 3s = ~5min total
pub struct PollConfig {
    pub max_attempts: u32,
    pub interval_ok_ms: u32,
    pub interval_err_ms: u32,
}

impl PollConfig {
    /// Wallet flow: fast polling (~60s).
    pub fn wallet() -> Self {
        Self {
            max_attempts: 30,
            interval_ok_ms: 2000,
            interval_err_ms: 3000,
        }
    }

    /// QR flow: long polling (~5min) with early-exit state check.
    pub fn qr() -> Self {
        Self {
            max_attempts: 100,
            interval_ok_ms: 3000,
            interval_err_ms: 3000,
        }
    }
}

/// Shared async polling loop for deposit confirmation.
///
/// Polls `api::confirm_deposit` until confirmed, max attempts reached, or an
/// optional `should_stop` closure returns `true`.
pub async fn poll_deposit_confirmation(
    event_id: &str,
    attendee_id: &str,
    config: &PollConfig,
    set_state: &WriteSignal<DepositPageState>,
    deposit_data: &DepositStatusResponse,
    should_stop: Option<&dyn Fn() -> bool>,
) -> PollOutcome {
    let mut attempts = 0u32;
    while attempts < config.max_attempts {
        if let Some(check) = should_stop
            && check()
        {
            return PollOutcome::Cancelled;
        }
        match api::confirm_deposit(event_id, attendee_id).await {
            Ok(ConfirmDepositResponse {
                confirmed: true,
                tx_signature: Some(sig),
                ..
            }) => {
                log::info!("[deposit] confirmed on-chain: {sig}");
                set_state.set(DepositPageState::DepositConfirmed(
                    deposit_data.clone(),
                    sig.clone(),
                ));
                return PollOutcome::Confirmed(sig);
            }
            Ok(_) => {
                attempts += 1;
                if attempts < config.max_attempts {
                    gloo_timers::future::TimeoutFuture::new(config.interval_ok_ms).await;
                }
            }
            Err(e) => {
                log::warn!("[deposit] confirmation poll error: {e}");
                attempts += 1;
                if attempts < config.max_attempts {
                    gloo_timers::future::TimeoutFuture::new(config.interval_err_ms).await;
                }
            }
        }
    }
    PollOutcome::Timeout
}

/// Result of the deposit confirmation polling loop.
#[derive(Debug)]
pub enum PollOutcome {
    /// Deposit confirmed on-chain with the given signature.
    Confirmed(String),
    /// Max attempts reached without confirmation.
    Timeout,
    /// Caller requested cancellation (e.g. user navigated away).
    Cancelled,
}

// ---------------------------------------------------------------------------
// Poll for deposit confirmation (wallet flow)
// ---------------------------------------------------------------------------

/// Create handler: poll backend for on-chain deposit confirmation.
pub fn make_poll_confirmation(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String, String, String) + Clone + Send + Sync + 'static {
    move |event_id: String, attendee_id: String, _tx_sig: String| {
        let set_state = set_state;
        let set_toast = set_toast;
        leptos::task::spawn_local(async move {
            let deposit_data = match &state.get() {
                DepositPageState::AwaitingConfirmation(d, _, _) => d.clone(),
                _ => return,
            };
            let config = PollConfig::wallet();
            let outcome = poll_deposit_confirmation(
                &event_id,
                &attendee_id,
                &config,
                &set_state,
                &deposit_data,
                None,
            )
            .await;
            if matches!(outcome, PollOutcome::Timeout) {
                app_components::show_toast(
                    &set_toast,
                    "Confirmation is taking longer than expected. Your deposit may still be processing.",
                    ToastType::Warning,
                );
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Poll for deposit confirmation (QR flow)
// ---------------------------------------------------------------------------

/// Create handler: poll backend for QR-based deposit confirmation.
///
/// Similar to `make_poll_confirmation` but with longer timeout and an
/// early-exit check that stops if the page state leaves `UsdcQrReady`.
pub fn make_qr_poll_confirmation(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    params: DepositParamsSignal,
) -> impl Fn() + Clone + Send + Sync + 'static {
    move || {
        let set_state = set_state;
        let state = state;
        let deposit_data = match &state.get() {
            DepositPageState::UsdcQrReady(d, _) => d.clone(),
            _ => return,
        };
        let event_id = extract_event_id_from_url().unwrap_or_default();
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        leptos::task::spawn_local(async move {
            let config = PollConfig::qr();
            let outcome = poll_deposit_confirmation(
                &event_id,
                &attendee_id,
                &config,
                &set_state,
                &deposit_data,
                Some(&|| !matches!(&state.get(), DepositPageState::UsdcQrReady(_, _))),
            )
            .await;
            if matches!(outcome, PollOutcome::Timeout) {
                log::warn!("[deposit] QR poll timed out after 5 min");
            }
        });
    }
}
