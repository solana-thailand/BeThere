//! Staff scanner page — fullscreen camera QR scanning with slide-up bottom sheet.
//!
//! The camera fills the entire screen. A bottom sheet slides up with session info
//! and a manual entry toggle. Scan results appear as glass panel overlays on top
//! of the camera view.
//!
//! The video element is always present in the DOM (never conditionally rendered)
//! to avoid race conditions between the reactive Effect and DOM mounting.
//!
//! Requires being wrapped in `<ProtectedRoute>` to provide
//! `ReadSignal<String>` (user email) via context.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::api::{self, AttendeeData, CheckInData, WalkinRegisterRequest};
use crate::auth;
use crate::components::{self, ToastType};
use crate::utils;

// ===== Camera QR Scanner JS Interop =====
// Uses wasm_bindgen module imports from /js/scanner.js instead of js_sys::eval().
// This avoids requiring 'unsafe-eval' in the Content-Security-Policy.
//
// The JS module at frontend-leptos/js/scanner.js provides:
// - startCamera()      — request camera, attach to #scanner-video, start QR loop
// - stopCamera()       — stop camera stream and QR detection
// - checkQrResult()    — poll for detected QR code (string | null)
// - checkCameraError() — poll for camera error message (string | null)
// - isScannerActive()  — check if scanner loop is running (bool)
//
// Rust call sites use snake_case names mapped via #[wasm_bindgen(js_name = ...)].

#[wasm_bindgen(module = "/js/scanner.js")]
extern "C" {
    /// Start the camera and QR scanning loop.
    ///
    /// Requests camera access (rear-facing preferred), waits for the video element
    /// to be both present AND visible in the DOM, streams to `#scanner-video`,
    /// and starts a JS-side loop that polls for QR codes every 300ms.
    ///
    /// Results are stored in `window.__qrResult`; errors in `window.__cameraError`.
    #[wasm_bindgen(js_name = "startCamera")]
    fn start_camera_js();

    /// Stop the camera stream and QR scanning loop.
    #[wasm_bindgen(js_name = "stopCamera")]
    fn stop_camera_js();

    /// Poll for a detected QR code value. Returns the raw string and clears it.
    #[wasm_bindgen(js_name = "checkQrResult")]
    fn check_qr_result_js() -> Option<String>;

    /// Poll for camera errors set by the JS scanning loop.
    #[wasm_bindgen(js_name = "checkCameraError")]
    fn check_camera_error_js() -> Option<String>;

    /// Check if the scanner is still active (set by start/stop).
    #[wasm_bindgen(js_name = "isScannerActive")]
    fn is_scanner_active_js() -> bool;
}

// ===== QR Code Generation JS Interop =====
// Uses the QRious library (lazy-loaded by lazy_assets.js) to generate QR code images
// as base64 data URLs. The JS module at frontend-leptos/js/qr_generate.js provides:
// - preloadQrLibraries()        — async: loads jsQR + QRious from CDN (call on mount)
// - generateQrDataUrl(text, size) — sync: returns base64 PNG data URL (null if not loaded)
// - copyToClipboard(text)         — copies text to system clipboard

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    /// Preload jsQR and QRious libraries from CDN.
    ///
    /// Call this on component mount for pages that render QR codes.
    /// Deduplicates — safe to call multiple times. Libraries load in parallel.
    /// After resolution, `generate_qr_data_url` will return valid data URLs.
    #[wasm_bindgen(js_name = "preloadQrLibraries")]
    async fn preload_qr_libraries_js();

    /// Generate a QR code image as a base64 PNG data URL.
    ///
    /// Returns something like "data:image/png;base64,..." or null if
    /// the QRious library hasn't loaded yet.
    #[wasm_bindgen(js_name = "generateQrDataUrl")]
    fn generate_qr_data_url(text: &str, size: u32) -> Option<String>;

    /// Copy text to the system clipboard.
    ///
    /// Uses the Clipboard API with a textarea fallback for older browsers.
    /// Returns true if the copy operation was initiated successfully.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
}

// ===== Solana Wallet JS Interop (for on-chain escrow check-in) =====

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    /// Get a list of detected Solana wallet adapter names.
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    /// Connect to a Solana wallet and return the public key (base58).
    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Sign and send a base64-encoded serialized transaction.
    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;
}

/// Async wrapper: connect to a Solana wallet and return the public key (base58).
async fn connect_wallet_js(wallet_name: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[wasm] connect_wallet_js: empty wallet name, returning None");
        return None;
    }
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() { None } else { val.as_string() }
        }
        Err(e) => {
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            None
        }
    }
}

/// Async wrapper: sign and send a base64-encoded serialized transaction.
async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[wasm] sign_and_send_tx_js: empty wallet name, returning None");
        return None;
    }
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() { None } else { val.as_string() }
        }
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            None
        }
    }
}

// ===== Haptic & Audio Feedback JS Interop =====
// Uses Web Audio API + Vibration API for instant scan feedback.
// The JS module at frontend-leptos/js/feedback.js provides:
// - feedbackSuccess()  — short vibration + high beep
// - feedbackWarning()  — double-pulse vibration + medium tone
// - feedbackError()    — long vibration + low tone
// - enableAudio()      — opt in to audio beeps
// - disableAudio()     — opt out of audio beeps
// - isAudioFeedbackEnabled() — check session preference

#[wasm_bindgen(module = "/js/feedback.js")]
extern "C" {
    #[wasm_bindgen(js_name = "feedbackSuccess")]
    fn feedback_success_js();

    #[wasm_bindgen(js_name = "feedbackWarning")]
    fn feedback_warning_js();

    #[wasm_bindgen(js_name = "feedbackError")]
    fn feedback_error_js();

    #[wasm_bindgen(js_name = "enableAudio")]
    fn enable_audio_js();

    #[wasm_bindgen(js_name = "disableAudio")]
    fn disable_audio_js();

    #[wasm_bindgen(js_name = "isAudioFeedbackEnabled")]
    fn is_audio_enabled_js() -> bool;
}

// ===== State Types =====

/// Current state of the check-in flow.
#[derive(Clone)]
enum CheckInState {
    /// No active check-in.
    Idle,
    /// Looking up an attendee by ID.
    LookingUp,
    /// Attendee found, approved, and in-person — ready to confirm.
    Found(Box<AttendeeData>),
    /// Attendee is already checked in.
    AlreadyCheckedIn(Box<AttendeeData>),
    /// Attendee is not approved (status ≠ "Approved").
    NotApproved(Box<AttendeeData>),
    /// Attendee is not In-Person (e.g. Online/Virtual).
    NotInPerson(Box<AttendeeData>),
    /// Attendee not found by api_id.
    NotFound,
    /// Performing the check-in POST request.
    CheckingIn { name: String, _id: String },
    /// Check-in succeeded.
    Success(Box<CheckInData>),
    /// An error occurred at any step.
    Error,
    // --- Escrow on-chain check-in states (after off-chain Success) ---
    /// Organizer choosing which wallet to connect for on-chain check-in.
    EscrowChooseWallet {
        check_in_data: Box<CheckInData>,
        attendee_id: String,
        event_id: String,
    },
    /// Organizer wallet connected, ready to sign on-chain TX.
    EscrowWalletConnected {
        check_in_data: Box<CheckInData>,
        attendee_id: String,
        event_id: String,
        wallet_name: String,
        public_key: String,
    },
    /// On-chain TX being signed/sent.
    EscrowSigning {
        wallet_name: String,
    },
    /// On-chain check-in confirmed.
    EscrowConfirmed {
        check_in_data: Box<CheckInData>,
        signature: String,
    },
    /// On-chain check-in failed.
    EscrowError {
        check_in_data: Box<CheckInData>,
        message: String,
    },
    // --- Walk-in registration states ---
    /// Walk-in registration form is displayed.
    WalkinForm,
    /// Walk-in registration request in progress.
    WalkinRegistering,
    /// Walk-in registration succeeded — show claim QR to attendee.
    WalkinSuccess {
        claim_url: String,
        name: String,
    },
}


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

    // Event selector state — signals declared early because handlers below reference active_event_id.
    let (events_list, set_events_list) = signal(Vec::<api::EventMeta>::new());
    let (active_event_id, set_active_event_id) = signal(None::<String>);
    let (escrow_enabled, set_escrow_enabled) = signal(false);
    let (events_loading, set_events_loading) = signal(false);

    // Wallet detection — pre-poll on mount like events_page.rs.
    // Phantom injects async after page load; a single sync call returns [].
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    leptos::task::spawn_local(async move {
        let mut wallets = get_detected_wallets_js();
        if wallets.is_empty() {
            for _ in 0..10 {
                gloo::timers::future::TimeoutFuture::new(300).await;
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

    // Preload jsQR + QRious libraries on mount.
    // startCamera() also awaits them, but preloading here ensures they're
    // ready by the time the claim QR card needs QRious after check-in.
    leptos::task::spawn_local(async {
        preload_qr_libraries_js().await;
        log::info!("[scanner] QR libraries preloaded");
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
                // Brief delay for camera to initialize
                gloo::timers::future::TimeoutFuture::new(500).await;

                loop {
                    gloo::timers::future::TimeoutFuture::new(300).await;

                    // Stop polling when superseded by a new round
                    if scan_round.get() != round {
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
                            Some(id) => process_attendee_id(&id, set_state, set_t, set_s_total),
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
            process_attendee_id(&scan_id, set_check_in_state, set_toast, set_session_total);
        }
    });

    // Handle manual form submission
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
        match extract_attendee_id(&value) {
            Some(id) => process_attendee_id(&id, set_check_in_state, set_toast, set_session_total),
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
            let _interval = leptos::task::spawn_local(async move {
                loop {
                    gloo::timers::future::TimeoutFuture::new(1000).await;
                    // Stop if we're no longer in Success (e.g. user clicked Scan Next)
                    if !matches!(state_check.get(), CheckInState::Success(_)) {
                        break;
                    }
                    let remaining = secs_reader.get();
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

    // Load events on mount — populate events_list, auto-select first active event,
    // and check escrow status for the selected event.
    Effect::new(move |_| {
        set_events_loading.set(true);
        let set_eid = set_active_event_id;
        let set_ee = set_escrow_enabled;
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
            let first_active = events
                .iter()
                .find(|e| e.status == api::EventStatus::Active);
            let selected_id = first_active.map(|e| e.id.clone());
            set_el.set(events);
            set_eid.set(selected_id.clone());
            set_el_loading.set(false);

            // Load event detail for escrow status
            if let Some(ref event_id) = selected_id {
                match api::get_event_detail(event_id).await {
                    Ok(detail) => {
                        let enabled = detail.event.deposit_enabled
                            && !detail.event.escrow_address.is_empty();
                        log::info!(
                            "[scanner] event '{}' escrow_enabled={}",
                            event_id,
                            enabled
                        );
                        set_ee.set(enabled);
                    }
                    Err(e) => {
                        log::warn!("[scanner] failed to load event detail: {e}");
                    }
                }
            }
        });
    });

    // When active_event_id changes, reload escrow status for the new event.
    Effect::new(move |_| {
        let eid = active_event_id.get();
        let set_ee = set_escrow_enabled;
        leptos::task::spawn_local(async move {
            if let Some(ref event_id) = eid {
                if event_id.is_empty() {
                    return;
                }
                match api::get_event_detail(event_id).await {
                    Ok(detail) => {
                        let enabled = detail.event.deposit_enabled
                            && !detail.event.escrow_address.is_empty();
                        log::info!(
                            "[scanner] event '{}' escrow_enabled={}",
                            event_id,
                            enabled
                        );
                        set_ee.set(enabled);
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
        if let CheckInState::EscrowChooseWallet { check_in_data, attendee_id, event_id } = &state {
            let check_in_data = check_in_data.clone();
            let attendee_id = attendee_id.clone();
            let event_id = event_id.clone();
            let wn = wallet_name.clone();
            let set_state = set_check_in_state;
            let set_t = set_toast;
            leptos::task::spawn_local(async move {
                match connect_wallet_js(&wn).await {
                    Some(pk) => {
                        log::info!("[scanner] organizer wallet connected: {} ({})", wn, pk);
                        set_state.set(CheckInState::EscrowWalletConnected {
                            check_in_data,
                            attendee_id,
                            event_id,
                            wallet_name: wn,
                            public_key: pk,
                        });
                    }
                    None => {
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
                if let Err(cluster_err) = crate::pages::escrow_init::check_wallet_cluster(&wallet_name, &expected_cluster).await {
                    log::error!("[scanner] cluster mismatch: {cluster_err}");
                    set_state.set(CheckInState::EscrowError {
                        check_in_data,
                        message: cluster_err,
                    });
                    return;
                }

                // Step 2: Sign and send the TX via the wallet
                match sign_and_send_tx_js(&wallet_name, &tx_resp.transaction).await {
                    Some(signature) => {
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
                    None => {
                        log::error!("[scanner] wallet sign+send failed for on-chain check-in");
                        set_state.set(CheckInState::EscrowError {
                            check_in_data,
                            message: "Transaction rejected or failed".to_string(),
                        });
                    }
                }
            });
        }
    };

    // ===== Walk-in registration =====

    // Walk-in form signals
    let (walkin_name, set_walkin_name) = signal(String::new());
    let (walkin_email, set_walkin_email) = signal(String::new());
    let (walkin_phone, set_walkin_phone) = signal(String::new());

    // Handler: open walk-in form
    let handle_walkin_open = move |_: web_sys::MouseEvent| {
        set_walkin_name.set(String::new());
        set_walkin_email.set(String::new());
        set_walkin_phone.set(String::new());
        set_check_in_state.set(CheckInState::WalkinForm);
    };

    // Handler: cancel walk-in form → back to Idle
    let handle_walkin_cancel = move |_: web_sys::MouseEvent| {
        set_check_in_state.set(CheckInState::Idle);
        set_scan_round.update(|r| *r += 1);
    };

    // Handler: submit walk-in registration
    let handle_walkin_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name = walkin_name.get().trim().to_string();
        let email = walkin_email.get().trim().to_string();
        let phone = walkin_phone.get().trim().to_string();

        if name.is_empty() || email.is_empty() {
            components::show_toast(
                &set_toast,
                "Name and email are required",
                ToastType::Warning,
            );
            return;
        }

        let event_id = match active_event_id.get() {
            Some(eid) if !eid.is_empty() => eid,
            _ => {
                components::show_toast(
                    &set_toast,
                    "No event selected. Select an event first.",
                    ToastType::Warning,
                );
                return;
            }
        };

        set_check_in_state.set(CheckInState::WalkinRegistering);

        let set_state = set_check_in_state;
        let set_t = set_toast;
        let name_for_callback = name.clone();

        leptos::task::spawn_local(async move {
            let req = WalkinRegisterRequest {
                event_id,
                name: name.clone(),
                email: email.clone(),
                phone: if phone.is_empty() { None } else { Some(phone) },
            };
            match api::register_walkin(&req).await {
                Ok(resp) => {
                        log::info!("[scanner] walk-in registered: {}", resp.claim_url);
                        feedback_success_js(); // Walk-in registered — vibration + beep
                        set_state.set(CheckInState::WalkinSuccess {
                        claim_url: resp.claim_url,
                        name: name_for_callback,
                    });
                    components::show_toast(
                        &set_t,
                        "Walk-in attendee registered!",
                        ToastType::Success,
                    );
                }
                Err(err) => {
                    log::error!("[scanner] walk-in register failed: {err}");
                    set_state.set(CheckInState::WalkinForm);
                    components::show_toast(
                        &set_t,
                        &format!("Registration failed: {err}"),
                        ToastType::Error,
                    );
                }
            }
        });
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

            // Event selector bar — shows dropdown when multiple active events exist.
            // Hidden when only 0 or 1 active event. Shows warning when no active events.
            {move || {
                let all_events = events_list.get();
                let active_events: Vec<_> = all_events
                    .iter()
                    .filter(|e| e.status == api::EventStatus::Active)
                    .collect();

                if events_loading.get() {
                    view! {
                        <div class="scanner-event-bar">
                            <span class="scanner-event-label">"Loading events…"</span>
                        </div>
                    }.into_any()
                } else if active_events.is_empty() {
                    view! {
                        <div class="scanner-event-bar" style="background:rgba(239,68,68,0.1);border-bottom-color:rgba(239,68,68,0.3);">
                            <span class="scanner-event-label" style="color:var(--danger);">"⚠ No active events found"</span>
                        </div>
                    }.into_any()
                } else if active_events.len() == 1 {
                    // Only one active event — no need for dropdown, but show name
                    let name = active_events[0].name.clone();
                    view! {
                        <div class="scanner-event-bar">
                            <span class="scanner-event-label">"Event:"</span>
                            <span style="font-size:0.85rem;">{name}</span>
                        </div>
                    }.into_any()
                } else {
                    // Multiple active events — show dropdown
                    let options = active_events.clone();
                    view! {
                        <div class="scanner-event-bar">
                            <span class="scanner-event-label">"Event:"</span>
                            <select
                                class="scanner-event-select"
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    set_active_event_id.set(
                                        if val.is_empty() { None } else { Some(val) }
                                    );
                                }
                                prop:value=move || active_event_id.get().unwrap_or_default()
                            >
                                {options.into_iter().map(|e| {
                                    let id = e.id.clone();
                                    let name = e.name.clone();
                                    view! {
                                        <option value=id>{name}</option>
                                    }
                                }).collect::<Vec<_>>()}
                            </select>
                        </div>
                    }.into_any()
                }
            }}

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
                        class="scanner-scan-hint"
                        style="background:rgba(239,68,68,0.15);border-color:var(--danger-border);color:var(--danger);"
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
                        // Walk-in registration form
                        <Show
                            when=move || matches!(check_in_state.get(), CheckInState::WalkinForm)
                            fallback=|| view! { <div></div> }
                        >
                            <div>
                                <div class="scanner-state-header">
                                    <h2>"Register Walk-in Attendee"</h2>
                                </div>
                                <form on:submit=handle_walkin_submit>
                                    <div style="display:flex;flex-direction:column;gap:0.75rem;">
                                        <div>
                                            <label style="display:block;font-size:0.85rem;margin-bottom:0.25rem;color:var(--muted);">"Name *"</label>
                                            <input
                                                type="text"
                                                class="manual-input"
                                                style="width:100%;"
                                                placeholder="Attendee name"
                                                required=true
                                                prop:value=move || walkin_name.get()
                                                on:input=move |ev| set_walkin_name.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div>
                                            <label style="display:block;font-size:0.85rem;margin-bottom:0.25rem;color:var(--muted);">"Email *"</label>
                                            <input
                                                type="email"
                                                class="manual-input"
                                                style="width:100%;"
                                                placeholder="attendee@email.com"
                                                required=true
                                                prop:value=move || walkin_email.get()
                                                on:input=move |ev| set_walkin_email.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div>
                                            <label style="display:block;font-size:0.85rem;margin-bottom:0.25rem;color:var(--muted);">"Phone (optional)"</label>
                                            <input
                                                type="tel"
                                                class="manual-input"
                                                style="width:100%;"
                                                placeholder="+66..."
                                                prop:value=move || walkin_phone.get()
                                                on:input=move |ev| set_walkin_phone.set(event_target_value(&ev))
                                            />
                                        </div>
                                    </div>
                                    <button
                                        class="btn btn-success btn-block"
                                        type="submit"
                                        style="margin-top:1rem;"
                                    >
                                        "Register"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-block"
                                        type="button"
                                        style="margin-top:0.5rem;"
                                        on:click=handle_walkin_cancel
                                    >
                                        "Cancel"
                                    </button>
                                </form>
                            </div>
                        </Show>
                        // Walk-in registering spinner
                        <Show
                            when=move || matches!(check_in_state.get(), CheckInState::WalkinRegistering)
                            fallback=|| view! { <div></div> }
                        >
                            <div class="scanner-state-loading">
                                <div class="page-loading">
                                    <span class="spinner spinner-lg"></span>
                                    <span>"Registering walk-in..."</span>
                                </div>
                            </div>
                        </Show>
                        // Walk-in success — show claim QR
                        <Show
                            when=move || matches!(check_in_state.get(), CheckInState::WalkinSuccess { .. })
                            fallback=|| view! { <div></div> }
                        >
                            {move || {
                                let state = check_in_state.get();
                                match state {
                                    CheckInState::WalkinSuccess { ref claim_url, ref name } => {
                                        let qr_data_url = generate_qr_data_url(claim_url, 240);
                                        let claim_url_for_display = claim_url.clone();
                                        let name_clone = name.clone();
                                        view! {
                                            <div>
                                                <div class="result-success">
                                                    <div class="success-check">
                                                        <svg viewBox="0 0 24 24">
                                                            <polyline points="20 6 9 17 4 12"></polyline>
                                                        </svg>
                                                    </div>
                                                    <h2 class="claim-success-title">"Walk-in Registered!"</h2>
                                                    <div class="result-details">
                                                        <p class="scanner-attendee-name">{name_clone}</p>
                                                    </div>
                                                </div>
                                                {move || {
                                                    let url = claim_url_for_display.clone();
                                                    match &qr_data_url {
                                                        Some(img_src) => {
                                                            view! {
                                                                <ClaimQrCard
                                                                    qr_src=img_src.clone()
                                                                    claim_url=url
                                                                    label="Show this QR to the attendee:"
                                                                />
                                                            }
                                                                .into_any()
                                                        }
                                                        None => view! { <div></div> }.into_any(),
                                                    }
                                                }}
                                                <button
                                                    class="btn btn-success btn-block"
                                                    style="margin-top:0.5rem;"
                                                    on:click=handle_reset
                                                >
                                                    "Scan Another"
                                                </button>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    _ => view! { <div></div> }.into_any(),
                                }
                            }}
                        </Show>
                        // Non-walk-in states: delegate to render_check_in_state
                        <Show
                            when=move || !matches!(
                                check_in_state.get(),
                                CheckInState::WalkinForm
                                    | CheckInState::WalkinRegistering
                                    | CheckInState::WalkinSuccess { .. }
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
                        <div style="display:flex;gap:0.5rem;align-items:center;">
                            <button
                                class="scanner-manual-toggle"
                                on:click=move |_| set_manual_mode.update(|m| *m = !*m)
                            >
                                {move || if manual_mode.get() { "Cancel" } else { "Enter manually" }}
                            </button>
                            <button
                                class="scanner-manual-toggle"
                                style=move || if flash_enabled.get() { "color:var(--accent);" } else { "" }
                                on:click=move |_| set_flash_enabled.update(|e| *e = !*e)
                                title="Toggle success flash"
                            >
                                "⚡"
                                {move || if flash_enabled.get() { " Flash On" } else { " Flash Off" }}
                            </button>
                            <button
                                class="scanner-manual-toggle"
                                style=move || if audio_enabled.get() { "color:var(--accent);" } else { "" }
                                on:click=move |_| {
                                    let new_val = !audio_enabled.get();
                                    if new_val {
                                        enable_audio_js();
                                    } else {
                                        disable_audio_js();
                                    }
                                    set_audio_enabled.set(new_val);
                                }
                                title="Toggle scan audio feedback"
                            >
                                "🔊"
                                {move || if audio_enabled.get() { " Sound On" } else { " Sound Off" }}
                            </button>
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
                                <span class="scanner-session-stat-value" style="color:var(--success);">{move || session_success.get()}</span>
                                <span class="scanner-session-stat-label">"Checked In"</span>
                            </div>
                            <div class="scanner-session-stat">
                                <span class="scanner-session-stat-value" style="color:var(--warning);">{move || session_total.get() - session_success.get()}</span>
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
                                        placeholder="Enter attendee ID (e.g. gst-abc123)"
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
                    <div style="margin-top:0.75rem;padding:0 0.25rem;">
                        <button
                            class="btn btn-primary btn-block"
                            on:click=handle_walkin_open
                        >
                            "Register Walk-in Attendee"
                        </button>
                    </div>
                </div>
            </Show>

            <components::Toast toast_signal=toast />
        </div>
    }
}

// ===== Check-In Logic =====

/// Process an attendee ID through the lookup flow.
///
/// Sets the appropriate `CheckInState` based on the attendee's status:
/// - Already checked in → `AlreadyCheckedIn`
/// - Not approved → `NotApproved`
/// - Not In-Person → `NotInPerson`
/// - Approved & In-Person → `Found` (ready to confirm)
fn process_attendee_id(
    id: &str,
    set_state: WriteSignal<CheckInState>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_session_total: WriteSignal<u32>,
) {
    set_session_total.update(|c| *c += 1);
    let attendee_id = id.to_string();
    set_state.set(CheckInState::LookingUp);
    leptos::task::spawn_local(async move {
        match api::get_attendee(&attendee_id, None).await {
            Ok(data) => {
                if data.is_checked_in {
                    feedback_warning_js(); // Already checked in — warning tone
                    set_state.set(CheckInState::AlreadyCheckedIn(Box::new(data)));
                } else if !data.is_approved {
                    feedback_error_js(); // Not approved — error tone
                    set_state.set(CheckInState::NotApproved(Box::new(data)));
                } else if !data.is_in_person {
                    feedback_warning_js(); // Not in-person — warning tone
                    set_state.set(CheckInState::NotInPerson(Box::new(data)));
                } else {
                    // Found — no feedback yet (wait for actual check-in confirmation)
                    set_state.set(CheckInState::Found(Box::new(data)));
                }
            }
            Err(err) => {
                log::warn!("[scanner] attendee lookup failed for id={attendee_id}: {err}");
                feedback_error_js(); // Not found — error tone
                set_state.set(CheckInState::NotFound);
                components::show_toast(&set_toast, "Attendee not found", ToastType::Error);
            }
        }
    });
}

/// Extract attendee ID from a QR code value or manual input.
///
/// Handles multiple formats:
/// - Raw API ID: `gst-abc123`
/// - URL with `?scan=`: `https://server/staff/?scan=gst-abc123`
/// - URL with `?id=`: `https://server/staff/?id=gst-abc123`
fn extract_attendee_id(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try URL parameter extraction
    if trimmed.starts_with("http")
        && let Ok(url) = web_sys::Url::new(trimmed)
    {
        if let Some(scan) = url.search_params().get("scan") {
            return Some(scan);
        }
        if let Some(id_param) = url.search_params().get("id") {
            return Some(id_param);
        }
    }

    // Return as-is (gst- prefix or raw ID)
    Some(trimmed.to_string())
}

// ===== Claim URL Helpers =====

/// Build the full claim URL from a claim token using the current window origin.
///
/// This makes the QR code dynamic — works correctly on both localhost:8787
/// (local testing) and the production domain without backend config changes.
fn build_claim_url(token: &str) -> String {
    let window = web_sys::window().expect("no window");
    let origin = window
        .location()
        .origin()
        .unwrap_or_else(|_| "http://localhost:8787".to_string());
    format!("{origin}/claim/{token}")
}

// ===== State View Helper Components =====

/// QR code card for claim URLs — displayed on Success and AlreadyCheckedIn.
#[component]
fn ClaimQrCard(qr_src: String, claim_url: String, label: &'static str) -> impl IntoView {
    view! {
        <div class="scanner-qr-wrapper">
            <div class="scanner-qr-card">
                <p class="scanner-qr-label">{label}</p>
                <img src=qr_src alt="Claim URL QR Code" class="scanner-qr-img" />
                <button class="btn btn-primary btn-sm scanner-qr-copy-btn"
                    on:click=move |_| { let _ = copy_to_clipboard_js(&claim_url); }>
                    "📋 Copy Link"
                </button>
            </div>
        </div>
    }
}

/// Attendee name + email info block.
#[component]
fn AttendeeInfoCard(name: String, email: String) -> impl IntoView {
    view! {
        <div class="scanner-attendee-info">
            <p class="scanner-attendee-name">{name}</p>
            <p class="scanner-attendee-email">{email}</p>
        </div>
    }
}

// ===== State View Rendering =====

/// Render the current check-in state as a view.
fn render_check_in_state<E1, E2, E3, E4, E5>(
    state: CheckInState,
    on_check_in: impl Fn(web_sys::MouseEvent) + 'static,
    on_reset: impl Fn(web_sys::MouseEvent) + Clone + 'static,
    on_escrow_check_in: E1,
    on_escrow_wallet_connect: E2,
    on_escrow_sign: E3,
    escrow_enabled: bool,
    wallets: Vec<String>,
    on_online_check_in: E4,
    // Undo check-in
    on_undo: E5,
    undo_confirm: ReadSignal<bool>,
    undo_timer_secs: ReadSignal<u32>,
    undo_expired: ReadSignal<bool>,
) -> AnyView
where
    E1: Fn(web_sys::MouseEvent) + Clone + 'static,
    E2: Fn(String) + Clone + 'static,
    E3: Fn(web_sys::MouseEvent) + Clone + 'static,
    E4: Fn(web_sys::MouseEvent) + Clone + 'static,
    E5: Fn(web_sys::MouseEvent) + Clone + Send + 'static,
{
    match state {
        CheckInState::Idle => view! { <div></div> }.into_any(),
        CheckInState::LookingUp => view! {
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>"Looking up attendee..."</span>
                </div>
            </div>
        }
        .into_any(),
        CheckInState::Found(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let ticket = data.attendee.ticket_name.clone();
            let participation = data.participation_type.clone();
            let badge = utils::get_participation_badge(&participation);
            view! {
                <div>
                    <div class="scanner-state-header">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"Ready to Check In"</h2>
                    </div>
                    <AttendeeInfoCard name=name email=email />
                    <div class="scanner-attendee-badges">
                        <span class="badge badge-info badge-pill">{ticket}</span>
                        <span class=format!("badge badge-pill {}", badge.css_class)>{badge.label}</span>
                    </div>
                    <div class="scanner-actions">
                        <button class="btn btn-success btn-block" on:click=on_check_in>
                            "✓ Confirm Check-In"
                        </button>
                    </div>
                    <button class="btn btn-outline btn-block" style="margin-top:0.5rem;" on:click=on_reset>
                        "Cancel"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::AlreadyCheckedIn(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let checked_at = data.attendee.checked_in_at.clone().unwrap_or_default();
            let formatted = utils::format_timestamp(&checked_at);
            let by_suffix = data
                .attendee
                .checked_in_by
                .as_ref()
                .map_or(String::new(), |by| {
                    if by.is_empty() {
                        String::new()
                    } else {
                        format!(" by {}", utils::escape_html(by))
                    }
                });
            let claim_url = data.attendee.claim_token.as_ref().map(|t| build_claim_url(t));
            let qr_data_url = claim_url
                .as_ref()
                .and_then(|url| generate_qr_data_url(url, 240));
            let claim_url_for_display = claim_url.clone();
            view! {
                <div>
                    <div class="result-warning">
                        <h2>"Already Checked In"</h2>
                        <AttendeeInfoCard name=name email=email />
                        <p class="scanner-result-detail-line">
                            "Checked in at: "{formatted}{by_suffix}
                        </p>
                    </div>

                    // Claim URL QR code — re-show in case staff needs to display it again
                    {move || {
                        match (&qr_data_url, &claim_url_for_display) {
                            (Some(img_src), Some(url)) => {
                                view! {
                                    <ClaimQrCard
                                        qr_src=img_src.clone()
                                        claim_url=url.clone()
                                        label="Claim QR (show to attendee):"
                                    />
                                }
                                    .into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    <button class="btn btn-outline btn-block" style="margin-top:1rem;" on:click=on_reset>
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotApproved(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let status = data.attendee.approval_status.clone();
            view! {
                <div>
                    <div class="result-error">
                        <h2>"Not Approved"</h2>
                        <AttendeeInfoCard name=name email=email />
                        <p class="scanner-result-detail-line">
                            "Status: "
                            <span style="color:var(--warning);">{status}</span>
                        </p>
                    </div>
                    <button class="btn btn-outline btn-block" style="margin-top:1rem;" on:click=on_reset>
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotInPerson(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let badge = utils::get_participation_badge(&data.participation_type);
            let on_online = on_online_check_in.clone();
            view! {
                <div>
                    <div class="result-warning">
                        <h2>"Online Attendee"</h2>
                        <AttendeeInfoCard name=name email=email />
                        <div class="scanner-attendee-badges">
                            <span class=format!("badge badge-pill {}", badge.css_class)>{badge.label}</span>
                        </div>
                        <p class="scanner-hint" style="margin-top:0.75rem;">
                            "This attendee registered for the online track. You can perform a virtual check-in to generate their claim link."
                        </p>
                    </div>
                    <button
                        class="btn btn-primary btn-block"
                        style="margin-top:1rem;"
                        on:click=on_online
                    >
                        "🌐 Virtual Check-In"
                    </button>
                    <button class="btn btn-outline btn-block" style="margin-top:0.5rem;" on:click=on_reset>
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotFound => view! {
            <div>
                <div class="result-error">
                    <h2>"Not Found"</h2>
                    <div class="result-details">
                        <p>"No matching attendee found. Please try again."</p>
                    </div>
                </div>
                <button
                    class="btn btn-outline btn-block"
                    style="margin-top:1rem;"
                    on:click=on_reset
                >
                    "Try Again"
                </button>
            </div>
        }
        .into_any(),
        CheckInState::CheckingIn { name, .. } => view! {
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>"Checking in "{name}"..."</span>
                </div>
            </div>
        }
        .into_any(),
        CheckInState::Success(result) => {
            let name = result.name.clone();
            let checked_at = result.checked_in_at.clone();
            let formatted = utils::format_timestamp(&checked_at);
            let by_suffix = {
                let by = result.checked_in_by.clone();
                if by.is_empty() {
                    String::new()
                } else {
                    format!(" by {}", utils::escape_html(&by))
                }
            };
            let claim_url = result.claim_token.as_ref().map(|t| build_claim_url(t));
            let qr_data_url = claim_url
                .as_ref()
                .and_then(|url| generate_qr_data_url(url, 240));
            let claim_url_for_display = claim_url.clone();
            let show_escrow = escrow_enabled;
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2 class="claim-success-title">"Checked In!"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p>"Checked in at: "{formatted}{by_suffix}</p>
                        </div>
                    </div>

                    // Claim URL QR code — show to attendee so they can scan it
                    {move || {
                        match (&qr_data_url, &claim_url_for_display) {
                            (Some(img_src), Some(url)) => {
                                view! {
                                    <ClaimQrCard
                                        qr_src=img_src.clone()
                                        claim_url=url.clone()
                                        label="Show this QR to the attendee to claim their NFT:"
                                    />
                                }
                                    .into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    // On-chain escrow check-in button (if event has escrow enabled)
                    {if show_escrow {
                        view! {
                            <button
                                class="btn btn-outline btn-block"
                                style="margin-top:0.75rem;border-color:var(--accent);color:var(--accent);"
                                on:click=on_escrow_check_in
                            >
                                "⛓ Mark Checked In On-Chain"
                            </button>
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }}

                    <button class="btn btn-success btn-block" style="margin-top:0.5rem;" on:click=on_reset>
                        "Scan Next"
                    </button>

                    // Undo check-in button — available for 30 seconds after check-in
                    {move || {
                        let expired = undo_expired.get();
                        let confirmed = undo_confirm.get();
                        let secs = undo_timer_secs.get();
                        if expired {
                            view! { <div></div> }.into_any()
                        } else if confirmed {
                            view! {
                                <button
                                    class="btn btn-danger btn-block"
                                    style="margin-top:0.5rem;font-size:0.85rem;"
                                    on:click=on_undo.clone()
                                >
                                    "\u{26a0} Confirm Undo?"
                                </button>
                                <p class="scanner-hint" style="margin-top:0.25rem;">
                                    {format!("Undo available for {}s", secs)}
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <button
                                    class="btn btn-danger btn-sm btn-block"
                                    style="margin-top:0.5rem;font-size:0.8rem;opacity:0.8;"
                                    on:click=on_undo.clone()
                                >
                                    "\u{21a9} Undo Check-In"
                                </button>
                                <p class="scanner-hint" style="margin-top:0.25rem;">
                                    {format!("Undo available for {}s", secs)}
                                </p>
                            }.into_any()
                        }
                    }}
                </div>
            }
            .into_any()
        }
        CheckInState::Error => view! {
            <div>
                <div class="result-error">
                    <h2>"Error"</h2>
                    <div class="result-details">
                        <p>"Something went wrong. Please try again."</p>
                    </div>
                </div>
                <button
                    class="btn btn-outline btn-block"
                    style="margin-top:1rem;"
                    on:click=on_reset
                >
                    "Try Again"
                </button>
            </div>
        }
        .into_any(),

        // --- Escrow on-chain check-in states ---
        CheckInState::EscrowChooseWallet { check_in_data, .. } => {
            let name = check_in_data.name.clone();
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"On-Chain Check-In"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p>"Connect your organizer wallet to record check-in on Solana."</p>
                        </div>
                    </div>

                    <div style="margin-top:1rem;">
                        {if wallets.is_empty() {
                            view! {
                                <div class="result-warning" style="padding:0.75rem;">
                                    <p style="margin:0;">"No Solana wallet detected. Install Phantom or Solflare."</p>
                                </div>
                            }
                                .into_any()
                        } else {
                            let cb = on_escrow_wallet_connect.clone();
                            wallets
                                .into_iter()
                                .map(move |w| {
                                    let w_clone = w.clone();
                                    let cb = cb.clone();
                                    view! {
                                        <button
                                            class="btn btn-outline btn-block"
                                            style="margin-bottom:0.5rem;border-color:var(--accent);color:var(--accent);"
                                            on:click=move |_| cb(w_clone.clone())
                                        >
                                            {format!("Connect {}", w)}
                                        </button>
                                    }
                                })
                                .collect::<Vec<_>>()
                                .into_any()
                        }}
                    </div>

                    <button class="btn btn-outline btn-block" style="margin-top:0.5rem;" on:click=on_reset>
                        "Skip & Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::EscrowWalletConnected { check_in_data, wallet_name, public_key, .. } => {
            let name = check_in_data.name.clone();
            let short_pk = if public_key.len() > 8 {
                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
            } else {
                public_key.clone()
            };
            let wallet_label = wallet_name.clone();
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"Ready to Sign"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p>
                                <span style="color:var(--muted);">{format!("{} ({})", wallet_label, short_pk)}</span>
                            </p>
                        </div>
                    </div>

                    <button
                        class="btn btn-primary btn-block"
                        style="margin-top:1rem;"
                        on:click=on_escrow_sign
                    >
                        "⛓ Sign On-Chain Check-In"
                    </button>

                    <button class="btn btn-outline btn-block" style="margin-top:0.5rem;" on:click=on_reset>
                        "Skip & Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::EscrowSigning { wallet_name } => view! {
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>{format!("Waiting for {} to approve...", wallet_name)}</span>
                </div>
            </div>
        }
        .into_any(),
        CheckInState::EscrowConfirmed { check_in_data, signature } => {
            let name = check_in_data.name.clone();
            let short_sig = if signature.len() > 16 {
                format!("{}...{}", &signature[..8], &signature[signature.len()-8..])
            } else {
                signature.clone()
            };
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"On-Chain Check-In Confirmed!"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p style="font-size:0.8rem;color:var(--muted);word-break:break-all;">
                                {format!("TX: {}", short_sig)}
                            </p>
                            <a
                                href={utils::solscan_tx_url(&signature, &utils::get_cluster())}
                                target="_blank"
                                rel="noopener noreferrer"
                                style="font-size:0.8rem;color:var(--accent);"
                            >
                                "View on Solscan ↗"
                            </a>
                        </div>
                    </div>

                    <button class="btn btn-success btn-block" style="margin-top:1rem;" on:click=on_reset>
                        "Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::EscrowError { check_in_data, message } => {
            let name = check_in_data.name.clone();
            view! {
                <div>
                    <div class="result-error">
                        <h2>"On-Chain Check-In Failed"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p style="font-size:0.85rem;color:var(--warning);">
                                {message}
                            </p>
                        </div>
                    </div>

                    <button class="btn btn-outline btn-block" style="margin-top:1rem;" on:click=on_reset>
                        "Skip & Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        // Walk-in states are rendered directly in the view; these arms should not be reached.
        CheckInState::WalkinForm => view! { <div></div> }.into_any(),
        CheckInState::WalkinRegistering => view! { <div></div> }.into_any(),
        CheckInState::WalkinSuccess { .. } => view! { <div></div> }.into_any(),
    }
}
