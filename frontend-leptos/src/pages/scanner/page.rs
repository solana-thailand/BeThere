//! The staff scanner page component.

use leptos::prelude::*;

use crate::api;
use crate::auth;
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

use super::event_bar::event_bar;
use super::interop::*;
use super::logic::*;
use super::state::*;
use super::views::*;
use super::walk_in::*;

/// Upper bound on one wait for a scan event (see `wait_for_scan_event_js`).
const SCAN_WAIT_TIMEOUT_MS: u32 = 1000;

// ===== Scanner Component =====

/// Staff scanner page component.
#[component]
pub fn Scanner() -> impl IntoView {
    // Get user email and role from ProtectedRoute context
    let user_email = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[scanner] no user_email in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });
    let user_role = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[scanner] no user_role in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });

    // Reactive state
    let (manual_mode, set_manual_mode) = signal(false);
    let (manual_input, set_manual_input) = signal(String::new());
    let (check_in_state, set_check_in_state) = signal(CheckInState::Idle);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);
    let (camera_error, set_camera_error) = signal(None::<String>);
    // Incremented on reset to restart the polling loop without leaving the tab.
    let (scan_round, set_scan_round) = signal(0u32);
    let (flash_enabled, set_flash_enabled) = signal(true);
    let (audio_enabled, set_audio_enabled) = signal(is_audio_enabled_js());
    let (settings_open, set_settings_open) = signal(false);

    // Event selector state — signals declared early because handlers below reference active_event_id.
    let (events_list, set_events_list) = signal(Vec::<api::EventMeta>::new());
    let (active_event_id, set_active_event_id) = signal(None::<String>);
    let (escrow_enabled, set_escrow_enabled) = signal(false);
    let (events_loading, set_events_loading) = signal(false);

    // Event format + capacity from EventDetail (used for walk-in gating).
    let (active_event_format, set_active_event_format) = signal(api::EventFormat::InPerson);
    let (_active_in_person_capacity, set_active_in_person_capacity) = signal(None::<u32>);

    // Wallet detection — pre-poll on mount like events_page.rs.
    // Phantom injects async after page load; a single sync call returns [].
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    leptos::task::spawn_local(async move {
        let mut wallets = get_detected_wallets_js();
        if wallets.is_empty() {
            for _ in 0..10 {
                gloo_timers::future::TimeoutFuture::new(300).await;
                wallets = get_detected_wallets_js();
                if !wallets.is_empty() {
                    break;
                }
            }
        }
        log::info!("[scanner] detected wallets: {:?}", wallets);
        set_detected_wallets.set(wallets);
    });

    // Session tracking signals
    let (session_total, set_session_total) = signal(0u32);
    let (session_success, set_session_success) = signal(0u32);
    let (_session_started_at, _set_session_started_at) = signal(Some(js_sys::Date::now()));

    // Undo check-in signals — two-click confirmation with 30s availability window.
    let (undo_confirm, set_undo_confirm) = signal(false);
    let (undo_timer_secs, set_undo_timer_secs) = signal(30u32);
    let (undo_expired, set_undo_expired) = signal(false);

    // Stop camera when component unmounts (e.g. navigating to /admin).
    // Without this, window.__scannerActive remains true and startCamera()
    // skips on remount, leaving the camera broken until page refresh.
    on_cleanup(move || {
        log::info!("[scanner] component unmounting — stopping camera");
        stop_camera_js();
    });

    // Camera lifecycle: start when Idle, stop when showing results.
    // Camera runs whenever check_in_state is Idle (regardless of manual_mode).
    // Stops on: non-Idle state (attendee info shown), or unmount.
    // Re-triggers on scan_round (reset) or check_in_state changes.
    Effect::new(move |_| {
        let round = scan_round.get(); // generation counter for polling loop
        let should_scan = matches!(check_in_state.get(), CheckInState::Idle);

        if should_scan {
            // Only start camera if not already running (avoids rapid stop/start)
            if !is_scanner_active_js() {
                set_camera_error.set(None);
                start_camera_js();
            }

            let set_cam_err = set_camera_error;
            let set_state = set_check_in_state;
            let set_t = set_toast;
            let set_s_total = set_session_total;

            leptos::task::spawn_local(async move {
                loop {
                    // Woken by the JS side as soon as there is a result, an
                    // error or a stop; the timeout only bounds how long a
                    // superseded round lingers.
                    let _ = wasm_bindgen_futures::JsFuture::from(wait_for_scan_event_js(
                        SCAN_WAIT_TIMEOUT_MS,
                    ))
                    .await;

                    // Stop polling when superseded by a new round — or when
                    // the page unmounted: `.get()` on a disposed signal
                    // panics (a wasm trap under panic=abort), and this loop
                    // outlives the component on a client-side navigation.
                    if scan_round.try_get() != Some(round) {
                        break;
                    }

                    // Stop polling when scanner is deactivated (unmount)
                    if !is_scanner_active_js() {
                        break;
                    }

                    // Check for camera errors (set asynchronously by JS)
                    if let Some(err) = check_camera_error_js() {
                        set_cam_err.set(Some(err));
                        break;
                    }

                    // Check for QR detection results
                    if let Some(qr_data) = check_qr_result_js() {
                        log::info!("[scanner] QR code detected: {qr_data}");
                        match extract_attendee_id(&qr_data) {
                            Some(id) => process_attendee_id(
                                &id,
                                active_event_id.get_untracked(),
                                set_state,
                                set_t,
                                set_s_total,
                            ),
                            None => components::show_toast(
                                &set_t,
                                "Invalid QR code format",
                                ToastType::Error,
                            ),
                        }
                        break;
                    }
                }
            });
        } else {
            stop_camera_js();
        }
    });

    // On mount: check for `?scan=` URL parameter from QR code redirect
    Effect::new(move |_| {
        let window = web_sys::window().expect("no window");
        if let Ok(url_str) = window.location().href()
            && let Ok(url) = web_sys::Url::new(&url_str)
            && let Some(scan_id) = url.search_params().get("scan")
        {
            // Clean up URL
            url.search_params().delete("scan");
            let clean_path = url.pathname();
            let _ = window.history().and_then(|h| {
                h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&clean_path))
            });
            process_attendee_id(
                &scan_id,
                active_event_id.get_untracked(),
                set_check_in_state,
                set_toast,
                set_session_total,
            );
        }
    });

    // Handle manual form submission
    let i18n = crate::i18n::use_i18n();
    let handle_manual_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let value = manual_input.get().trim().to_string();
        if value.is_empty() {
            components::show_toast(
                &set_toast,
                "Please enter an attendee ID",
                ToastType::Warning,
            );
            return;
        }
        match classify_manual_entry(&value) {
            Some(ManualEntry::Code(code)) => process_display_code(
                code,
                active_event_id.get_untracked(),
                i18n.get_locale_untracked(),
                set_check_in_state,
                set_toast,
                set_session_total,
            ),
            Some(ManualEntry::AttendeeId(id)) => process_attendee_id(
                &id,
                active_event_id.get_untracked(),
                set_check_in_state,
                set_toast,
                set_session_total,
            ),
            None => {
                components::show_toast(&set_toast, "Invalid attendee ID format", ToastType::Error)
            }
        }
    };

    // Handle check-in confirmation (only in Found state)
    let handle_check_in = move |_: web_sys::MouseEvent| {
        let state = check_in_state.get();
        if let CheckInState::Found(data) = &state {
            let id = data.attendee.api_id.clone();
            let name = data.attendee.name.clone();
            set_check_in_state.set(CheckInState::CheckingIn {
                name: name.clone(),
                _id: id.clone(),
            });
            let eid = active_event_id.get();
            let set_state = set_check_in_state;
            let set_t = set_toast;
            let set_s_success = set_session_success;
            leptos::task::spawn_local(async move {
                match api::check_in(&id, eid.as_deref(), false).await {
                    Ok(result) => {
                        log::info!("[scanner] check-in successful: {}", result.name);
                        feedback_success_js(); // Success — vibration + beep
                        set_state.set(CheckInState::Success(Box::new(result)));
                        set_s_success.update(|c| *c += 1);
                        api::invalidate_attendee_cache();
                        components::show_toast(
                            &set_t,
                            &format!("{name} checked in successfully!"),
                            ToastType::Success,
                        );
                    }
                    Err(err) => {
                        log::error!("[scanner] check-in failed: {err}");
                        feedback_error_js(); // Check-in failed — error tone
                        set_state.set(CheckInState::Error);
                        components::show_toast(
                            &set_t,
                            "Check-in failed. Please try again.",
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    // Handle virtual check-in for online attendees (hybrid events).
    // Uses ?online=true to bypass the in-person check on the backend.
    let handle_online_check_in = move |_: web_sys::MouseEvent| {
        let state = check_in_state.get();
        if let CheckInState::NotInPerson(data) = &state {
            let id = data.attendee.api_id.clone();
            let name = data.attendee.name.clone();
            set_check_in_state.set(CheckInState::CheckingIn {
                name: name.clone(),
                _id: id.clone(),
            });
            let eid = active_event_id.get();
            let set_state = set_check_in_state;
            let set_t = set_toast;
            let set_s_success = set_session_success;
            leptos::task::spawn_local(async move {
                match api::check_in(&id, eid.as_deref(), true).await {
                    Ok(result) => {
                        log::info!("[scanner] virtual check-in successful: {}", result.name);
                        feedback_success_js();
                        set_state.set(CheckInState::Success(Box::new(result)));
                        set_s_success.update(|c| *c += 1);
                        api::invalidate_attendee_cache();
                        components::show_toast(
                            &set_t,
                            &format!("{name} virtually checked in!"),
                            ToastType::Success,
                        );
                    }
                    Err(err) => {
                        log::error!("[scanner] virtual check-in failed: {err}");
                        feedback_error_js();
                        set_state.set(CheckInState::Error);
                        components::show_toast(
                            &set_t,
                            "Virtual check-in failed. Please try again.",
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    // Reset scanner to idle state and re-trigger camera via Effect.
    // The Effect tracks check_in_state: setting Idle + incrementing scan_round
    // causes it to re-evaluate should_scan=true → start camera fresh.
    let handle_reset = move |_: web_sys::MouseEvent| {
        let _ = check_qr_result_js(); // drain stale result
        let _ = check_camera_error_js(); // drain stale error
        set_camera_error.set(None);
        set_check_in_state.set(CheckInState::Idle);
        set_manual_input.set(String::new());
        set_manual_mode.set(false);
        set_scan_round.update(|r| *r += 1);
    };

    // Undo timer: countdown from 30s when Success state is entered.
    // Resets confirm + expired signals when leaving Success.
    Effect::new(move |_| {
        if matches!(check_in_state.get(), CheckInState::Success(_)) {
            set_undo_confirm.set(false);
            set_undo_expired.set(false);
            set_undo_timer_secs.set(30);

            let set_secs = set_undo_timer_secs;
            let set_exp = set_undo_expired;
            let state_check = check_in_state;
            let secs_reader = undo_timer_secs;
            leptos::task::spawn_local(async move {
                loop {
                    gloo_timers::future::TimeoutFuture::new(1000).await;
                    // Stop if we're no longer in Success (e.g. user clicked
                    // Scan Next) or the page unmounted (disposed → None;
                    // `.get()` would panic there).
                    if !matches!(state_check.try_get(), Some(CheckInState::Success(_))) {
                        break;
                    }
                    let Some(remaining) = secs_reader.try_get() else {
                        break;
                    };
                    if remaining == 0 {
                        set_exp.set(true);
                        break;
                    }
                    set_secs.set(remaining - 1);
                }
            });

            // When leaving Success, the interval will eventually notice
            // the state change (it doesn't re-check, but the expired
            // flag will prevent the button from showing).
        }
    });

    // Handle undo check-in (called from Success state).
    // First click: show confirmation prompt. Second click: execute undo.
    let handle_undo = move |_: web_sys::MouseEvent| {
        if undo_expired.get() {
            return;
        }
        if !undo_confirm.get() {
            // First click — ask for confirmation
            set_undo_confirm.set(true);
            return;
        }

        // Second click — execute undo
        let state = check_in_state.get();
        if let CheckInState::Success(data) = &state {
            let attendee_id = data.api_id.clone();
            let name = data.name.clone();
            let eid = active_event_id.get();
            let set_state = set_check_in_state;
            let set_t = set_toast;
            let set_s_success = set_session_success;

            set_check_in_state.set(CheckInState::CheckingIn {
                name: format!("Undoing {name}..."),
                _id: attendee_id.clone(),
            });

            leptos::task::spawn_local(async move {
                match api::undo_check_in(&attendee_id, eid.as_deref()).await {
                    Ok(()) => {
                        log::info!("[scanner] check-in undone for: {attendee_id}");
                        feedback_warning_js();
                        set_state.set(CheckInState::Idle);
                        set_scan_round.update(|r| *r += 1);
                        set_s_success.update(|c| *c = c.saturating_sub(1));
                        api::invalidate_attendee_cache();
                        components::show_toast(
                            &set_t,
                            &format!("Check-in undone for {name}"),
                            ToastType::Warning,
                        );
                    }
                    Err(err) => {
                        log::warn!("[scanner] undo failed: {err}");
                        // Restore Success state so staff can see the result
                        // Re-lookup is too complex; show error and let them scan again
                        set_state.set(CheckInState::Error);
                        let msg = if err.status == 404 {
                            "Undo not available on this server".to_string()
                        } else {
                            format!("Undo failed: {err}")
                        };
                        components::show_toast(&set_t, &msg, ToastType::Error);
                    }
                }
            });
        }
    };

    // ===== Event selector Effects =====

    // Load events on mount — populate events_list and auto-select the first
    // active event. The escrow/format detail comes from the active_event_id
    // effect below; fetching it here too requested it twice per mount.
    Effect::new(move |_| {
        set_events_loading.set(true);
        let set_eid = set_active_event_id;
        let set_el = set_events_list;
        let set_el_loading = set_events_loading;
        leptos::task::spawn_local(async move {
            let data = match api::list_events().await {
                Ok(data) => data,
                Err(e) => {
                    log::warn!("[scanner] failed to load events: {e}");
                    set_el_loading.set(false);
                    return;
                }
            };
            let events = data.events;
            // Auto-select the first active event
            let first_active = events.iter().find(|e| e.status == api::EventStatus::Active);
            let selected_id = first_active.map(|e| e.id.clone());
            set_el.set(events);
            set_eid.set(selected_id);
            set_el_loading.set(false);
        });
    });

    // When active_event_id changes, reload escrow status for the new event.
    Effect::new(move |_| {
        let eid = active_event_id.get();
        let set_ee = set_escrow_enabled;
        let set_ef = set_active_event_format;
        let set_cap = set_active_in_person_capacity;
        leptos::task::spawn_local(async move {
            if let Some(ref event_id) = eid {
                if event_id.is_empty() {
                    return;
                }
                let detail = api::get_event_detail(event_id).await;
                // A slower response for an event the operator already switched
                // away from must not overwrite the current event's settings.
                if active_event_id.try_get_untracked().flatten().as_deref() != Some(event_id) {
                    return;
                }
                match detail {
                    Ok(detail) => {
                        let enabled =
                            detail.event.deposit_enabled && !detail.event.escrow_address.is_empty();
                        log::info!(
                            "[scanner] event '{}' escrow_enabled={} format={:?}",
                            event_id,
                            enabled,
                            detail.event.event_format,
                        );
                        set_ee.set(enabled);
                        set_ef.set(detail.event.event_format);
                        set_cap.set(detail.event.in_person_capacity);
                    }
                    Err(e) => {
                        log::warn!("[scanner] failed to load event detail for escrow: {e}");
                    }
                }
            }
        });
    });

    // Handler: start escrow check-in (from Success state)
    let handle_escrow_check_in = move |_: web_sys::MouseEvent| {
        let state = check_in_state.get();
        if let CheckInState::Success(data) = &state {
            let attendee_id = data.api_id.clone();
            let event_id = active_event_id.get().unwrap_or_default();
            if event_id.is_empty() {
                components::show_toast(
                    &set_toast,
                    "No event selected for on-chain check-in",
                    ToastType::Warning,
                );
                return;
            }
            set_check_in_state.set(CheckInState::EscrowChooseWallet {
                check_in_data: Box::new(data.as_ref().clone()),
                attendee_id,
                event_id,
            });
        }
    };

    // Handler: connect organizer wallet (from EscrowChooseWallet state)
    let handle_escrow_wallet_connect = move |wallet_name: String| {
        let state = check_in_state.get();
        if let CheckInState::EscrowChooseWallet {
            check_in_data,
            attendee_id,
            event_id,
        } = &state
        {
            let check_in_data = check_in_data.clone();
            let attendee_id = attendee_id.clone();
            let event_id = event_id.clone();
            let wn = wallet_name.clone();
            let set_state = set_check_in_state;
            let set_t = set_toast;
            leptos::task::spawn_local(async move {
                match connect_wallet_js(&wn).await {
                    crate::wallet_error::WalletResult::Success(pk) => {
                        log::info!("[scanner] organizer wallet connected: {} ({})", wn, pk);
                        set_state.set(CheckInState::EscrowWalletConnected {
                            check_in_data,
                            attendee_id,
                            event_id,
                            wallet_name: wn,
                            public_key: pk,
                        });
                    }
                    crate::wallet_error::WalletResult::Error(e) => {
                        components::show_toast(
                            &set_t,
                            &crate::wallet_error::user_friendly_message(
                                &e,
                                crate::i18n::Locale::en,
                            ),
                            ToastType::Error,
                        );
                    }
                    crate::wallet_error::WalletResult::UnknownFailure => {
                        components::show_toast(
                            &set_t,
                            "Failed to connect wallet",
                            ToastType::Error,
                        );
                    }
                }
            });
        }
    };

    // Handler: sign and send on-chain mark_checked_in TX
    let handle_escrow_sign = move |_: web_sys::MouseEvent| {
        let state = check_in_state.get();
        if let CheckInState::EscrowWalletConnected {
            check_in_data,
            attendee_id,
            event_id,
            wallet_name,
            public_key: _,
        } = &state
        {
            let attendee_id = attendee_id.clone();
            let event_id = event_id.clone();
            let wallet_name = wallet_name.clone();
            let check_in_data = check_in_data.clone();
            let set_state = set_check_in_state;
            let set_t = set_toast;

            set_state.set(CheckInState::EscrowSigning {
                wallet_name: wallet_name.clone(),
            });

            leptos::task::spawn_local(async move {
                // Step 1: Build mark_checked_in TX
                let body = api::MarkCheckedInRequest {
                    event_id: event_id.clone(),
                    attendee_id: attendee_id.clone(),
                };
                let tx_resp = match api::mark_checked_in(&body).await {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("[scanner] mark_checked_in API failed: {e}");
                        set_state.set(CheckInState::EscrowError {
                            check_in_data,
                            message: format!("API error: {e}"),
                        });
                        return;
                    }
                };

                // SEC-014: Verify wallet cluster matches expected network.
                let expected_cluster = crate::utils::get_cluster();
                if let Err(cluster_err) =
                    crate::pages::escrow_init::check_wallet_cluster(&wallet_name, &expected_cluster)
                        .await
                {
                    let cluster_err = cluster_err.message(crate::i18n::Locale::en);
                    log::error!("[scanner] cluster mismatch: {cluster_err}");
                    set_state.set(CheckInState::EscrowError {
                        check_in_data,
                        message: cluster_err,
                    });
                    return;
                }

                // Pre-sign simulation.
                match crate::pages::escrow_init::simulate_transaction_js(
                    &wallet_name,
                    &tx_resp.transaction,
                )
                .await
                {
                    Ok(sim) if sim.ok => {}
                    Ok(sim) => {
                        let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                        log::error!("[scanner] check-in simulation failed: {err_msg}");
                        set_state.set(CheckInState::EscrowError {
                            check_in_data,
                            message: format!("Transaction would fail: {err_msg}"),
                        });
                        return;
                    }
                    Err(e) => {
                        log::warn!("[scanner] simulate error (not blocking): {e}");
                    }
                }

                // Step 2: Sign and send the TX via the wallet
                match sign_and_send_tx_js(&wallet_name, &tx_resp.transaction).await {
                    crate::wallet_error::WalletResult::Success(signature) => {
                        log::info!("[scanner] on-chain check-in TX sent: {}", signature);
                        feedback_success_js(); // On-chain confirmed — vibration + beep
                        set_state.set(CheckInState::EscrowConfirmed {
                            check_in_data,
                            signature,
                        });
                        components::show_toast(
                            &set_t,
                            "On-chain check-in confirmed!",
                            ToastType::Success,
                        );
                    }
                    crate::wallet_error::WalletResult::Error(e) => {
                        let msg =
                            crate::wallet_error::user_friendly_message(&e, crate::i18n::Locale::en);
                        log::error!(
                            "[scanner] wallet sign+send error: code={:?} msg={}",
                            e.code,
                            e.raw_message
                        );
                        set_state.set(CheckInState::EscrowError {
                            check_in_data,
                            message: msg,
                        });
                    }
                    crate::wallet_error::WalletResult::UnknownFailure => {
                        log::error!("[scanner] wallet sign+send failed for on-chain check-in");
                        set_state.set(CheckInState::EscrowError {
                            check_in_data,
                            message: "Transaction failed".to_string(),
                        });
                    }
                }
            });
        }
    };

    // ===== Walk-in registration =====

    // Walk-in form fields (the handlers and screens live in `walk_in.rs`).
    let walkin_fields = WalkinFields::new();
    let WalkinFields {
        set_walkin_name,
        set_walkin_email,
        set_walkin_phone,
        ..
    } = walkin_fields;

    // Handler: open walk-in form
    let handle_walkin_open = move |_: web_sys::MouseEvent| {
        set_walkin_name.set(String::new());
        set_walkin_email.set(String::new());
        set_walkin_phone.set(String::new());
        set_check_in_state.set(CheckInState::WalkinForm);
    };

    // Handle sign out
    let handle_sign_out = move |_: web_sys::MouseEvent| {
        auth::logout();
    };

    view! {
        <div>
            <components::AppHeader
                title="Scanner"
                user_email=user_email
                user_role=user_role
                on_sign_out=handle_sign_out
            />

            {event_bar(events_list, events_loading, active_event_id, set_active_event_id)}

            // Fullscreen camera — always in DOM, never conditionally rendered.
            <div class="scanner-fullscreen">
                <video
                    id="scanner-video"
                    autoplay=true
                    playsinline=true
                    muted=true
                />
                // Scanning frame overlay
                <div class="scanner-frame-overlay">
                    <div style=move || {
                        if camera_error.get().is_none()
                            && matches!(check_in_state.get(), CheckInState::Idle)
                        {
                            "width:180px;height:180px;border:3px solid rgba(99,102,241,0.7);border-radius:12px;box-shadow:0 0 0 2000px rgba(0,0,0,0.3);"
                        } else {
                            "display:none;"
                        }
                    } />
                </div>
                // Scan hint
                <Show
                    when=move || {
                        camera_error.get().is_none()
                            && matches!(check_in_state.get(), CheckInState::Idle)
                    }
                    fallback=|| view! { <div></div> }
                >
                    <div class="scanner-scan-hint">"Point camera at QR code"</div>
                </Show>
                // Camera error overlay
                <Show
                    when=move || camera_error.get().is_some()
                    fallback=|| view! { <div></div> }
                >
                    <div
                        class="scanner-scan-hint scanner-scan-hint--danger"
                    >
                        {move || camera_error.get().unwrap_or_default()}
                    </div>
                </Show>
            </div>

            // Success flash animation
            <Show
                when=move || matches!(check_in_state.get(), CheckInState::Success(_)) && flash_enabled.get()
                fallback=|| view! { <div></div> }
            >
                <div class="scanner-success-flash"></div>
            </Show>

            // Result overlay (glass panel) when not Idle
            <Show
                when=move || !matches!(check_in_state.get(), CheckInState::Idle)
                fallback=|| view! { <div></div> }
            >
                <div class="scanner-result-overlay">
                    <div class="scanner-glass-card">
                        {walkin_overlay(
                            walkin_fields,
                            check_in_state,
                            set_check_in_state,
                            set_scan_round,
                            set_toast,
                            active_event_id,
                            handle_reset,
                        )}
                        // Non-walk-in states: delegate to render_check_in_state
                        <Show
                            when=move || !matches!(
                                check_in_state.get(),
                                CheckInState::WalkinForm
                                    | CheckInState::WalkinRegistering
                                    | CheckInState::WalkinSuccess { .. }
                                    | CheckInState::WalkinCapacityWarning { .. }
                            )
                            fallback=|| view! { <div></div> }
                        >
                            {move || {
                                let state = check_in_state.get();
                                render_check_in_state(
                                    state,
                                    handle_check_in,
                                    handle_reset,
                                    handle_escrow_check_in,
                                    handle_escrow_wallet_connect,
                                    handle_escrow_sign,
                                    escrow_enabled.get(),
                                    detected_wallets.get(),
                                    handle_online_check_in,
                                    handle_undo,
                                    undo_confirm,
                                    undo_timer_secs,
                                    undo_expired,
                                )
                            }}
                        </Show>
                    </div>
                </div>
            </Show>

            // Bottom sheet (only when Idle)
            <Show
                when=move || matches!(check_in_state.get(), CheckInState::Idle)
                fallback=|| view! { <div></div> }
            >
                <div class="scanner-bottom-sheet">
                    // Drag handle
                    <div class="scanner-bottom-handle"></div>
                    // Session info
                    <div class="scanner-bottom-session">
                        <div class="scanner-bottom-session-info">
                            <div class="scanner-bottom-session-title">"Scanner"</div>
                            <div class="scanner-bottom-session-sub">
                                {move || {
                                    let total = session_total.get();
                                    let success = session_success.get();
                                    if total == 0 {
                                        "Ready to scan".to_string()
                                    } else {
                                        format!("{success}/{total} checked in")
                                    }
                                }}
                            </div>
                        </div>
                        <div class="scanner-session-actions">
                            <button
                                class="scanner-manual-toggle"
                                on:click=move |_| set_manual_mode.update(|m| *m = !*m)
                            >
                                {move || if manual_mode.get() { "Cancel" } else { "Enter manually" }}
                            </button>
                            <div class="scanner-settings-wrap">
                                <button
                                    class="scanner-settings-btn"
                                    class:scanner-settings-btn-active=move || settings_open.get()
                                    on:click=move |_| set_settings_open.update(|s| *s = !*s)
                                    title="Settings"
                                >
                                    <Icon icon=IconName::Settings class="icon-sm" />
                                </button>
                                <Show
                                    when=move || settings_open.get()
                                    fallback=|| view! { <div></div> }
                                >
                                    <div class="scanner-settings-popover">
                                        <button
                                            class="scanner-settings-toggle"
                                            class:is-on=move || flash_enabled.get()
                                            on:click=move |_| set_flash_enabled.update(|e| *e = !*e)
                                        >
                                            <Icon icon=IconName::Flash class="icon-sm" />
                                            {move || if flash_enabled.get() { "Flash On" } else { "Flash Off" }}
                                        </button>
                                        <button
                                            class="scanner-settings-toggle"
                                            class:is-on=move || audio_enabled.get()
                                            on:click=move |_| {
                                                let new_val = !audio_enabled.get();
                                                if new_val {
                                                    enable_audio_js();
                                                } else {
                                                    disable_audio_js();
                                                }
                                                set_audio_enabled.set(new_val);
                                            }
                                        >
                                            <Icon icon=IconName::Sound class="icon-sm" />
                                            {move || if audio_enabled.get() { "Sound On" } else { "Sound Off" }}
                                        </button>
                                    </div>
                                </Show>
                            </div>
                        </div>
                    </div>
                    // Session stats (shown when scans > 0)
                    <Show
                        when=move || { session_total.get() > 0 }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="scanner-session-stats">
                            <div class="scanner-session-stat">
                                <span class="scanner-session-stat-value">{move || session_total.get()}</span>
                                <span class="scanner-session-stat-label">"Scanned"</span>
                            </div>
                            <div class="scanner-session-stat">
                                <span class="scanner-session-stat-value scanner-stat-value--success">{move || session_success.get()}</span>
                                <span class="scanner-session-stat-label">"Checked In"</span>
                            </div>
                            <div class="scanner-session-stat">
                                <span class="scanner-session-stat-value scanner-stat-value--warning">{move || session_total.get() - session_success.get()}</span>
                                <span class="scanner-session-stat-label">"Other"</span>
                            </div>
                        </div>
                    </Show>
                    // Manual input form (toggled inline)
                    <Show
                        when=move || manual_mode.get()
                        fallback=|| view! { <div></div> }
                    >
                        <div class="scanner-manual-form">
                            <form on:submit=handle_manual_submit>
                                <div class="manual-input-group">
                                    <input
                                        type="text"
                                        placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.code.scanner_placeholder))
                                        autocapitalize="characters"
                                        autocomplete="off"
                                        prop:value=move || manual_input.get()
                                        on:input=move |ev| {
                                            let val = event_target_value(&ev);
                                            set_manual_input.set(val);
                                        }
                                    />
                                    <button
                                        class="btn btn-primary"
                                        type="submit"
                                        disabled=move || matches!(
                                            check_in_state.get(),
                                            CheckInState::LookingUp | CheckInState::CheckingIn { .. }
                                        )
                                    >
                                        "Look Up"
                                    </button>
                                </div>
                            </form>
                        </div>
                    </Show>
                    // Register Walk-in button
                    <div class="scanner-walkin-register-wrap">
                        <button
                            class="btn btn-primary btn-block"
                            on:click=handle_walkin_open
                            disabled=move || !active_event_format.get().has_in_person()
                        >
                            {move || {
                                if active_event_format.get().has_in_person() {
                                    view! { "Register Walk-in Attendee" }.into_any()
                                } else {
                                    view! { "Walk-in Not Available (Online Event)" }.into_any()
                                }
                            }}
                        </button>
                    </div>
                </div>
            </Show>

            <components::Toast toast_signal=toast />
        </div>
    }
}
