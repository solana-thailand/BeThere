//! Staff scanner page — QR code scanning and manual attendee check-in.
//!
//! Provides two tabs:
//! - Scanner (camera QR code scanning via BarcodeDetector API + jsQR fallback)
//! - Manual (text input for attendee ID lookup)
//!
//! The video element is always present in the DOM (hidden via CSS `display:none`
//! when the Manual tab is active) to avoid race conditions between the reactive
//! Effect and DOM mounting. Visibility is toggled via CSS, not `<Show>`.
//!
//! Requires being wrapped in `<ProtectedRoute>` to provide
//! `ReadSignal<String>` (user email) via context.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::api::{self, AttendeeData, CheckInData};
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
// Uses the QRious library (CDN-loaded in index.html) to generate QR code images
// as base64 data URLs. The JS module at frontend-leptos/js/qr_generate.js provides:
// - generateQrDataUrl(text, size) — returns base64 PNG data URL for a QR code
// - copyToClipboard(text)         — copies text to system clipboard

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
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
}

/// Active tab in the scanner page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScannerTab {
    Scanner,
    Manual,
}

// ===== Scanner Component =====

/// Staff scanner page component.
#[component]
pub fn Scanner() -> impl IntoView {
    // Get user email from ProtectedRoute context
    let user_email = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[scanner] no user_email in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });

    // Reactive state
    let (active_tab, set_active_tab) = signal(ScannerTab::Scanner);
    let (manual_input, set_manual_input) = signal(String::new());
    let (check_in_state, set_check_in_state) = signal(CheckInState::Idle);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);
    let (camera_error, set_camera_error) = signal(None::<String>);
    // Incremented on reset to restart the polling loop without leaving the tab.
    let (scan_round, set_scan_round) = signal(0u32);

    // Start/stop camera when switching between Scanner and Manual tabs.
    // Also re-triggers when scan_round changes (reset after a scan).
    Effect::new(move |_| {
        let _round = scan_round.get(); // track for re-trigger on reset
        if active_tab.get() == ScannerTab::Scanner {
            set_camera_error.set(None);
            start_camera_js();

            let set_cam_err = set_camera_error;
            let set_state = set_check_in_state;
            let set_t = set_toast;

            leptos::task::spawn_local(async move {
                // Brief delay for camera to initialize
                gloo::timers::future::TimeoutFuture::new(500).await;

                loop {
                    gloo::timers::future::TimeoutFuture::new(300).await;

                    // Stop polling when scanner is deactivated (tab switch / unmount)
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
                            Some(id) => process_attendee_id(&id, set_state, set_t),
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
            process_attendee_id(&scan_id, set_check_in_state, set_toast);
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
            Some(id) => process_attendee_id(&id, set_check_in_state, set_toast),
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
            let set_state = set_check_in_state;
            let set_t = set_toast;
            leptos::task::spawn_local(async move {
                match api::check_in(&id).await {
                    Ok(result) => {
                        log::info!("[scanner] check-in successful: {}", result.name);
                        set_state.set(CheckInState::Success(Box::new(result)));
                        components::show_toast(
                            &set_t,
                            &format!("{name} checked in successfully!"),
                            ToastType::Success,
                        );
                    }
                    Err(err) => {
                        log::error!("[scanner] check-in failed: {err}");
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

    // Reset scanner to idle state and restart camera polling.
    // Don't stop the camera here — the JS startCamera() guard skips if already
    // active, avoiding the rapid stop/start race that causes "media resource
    // aborted" errors. Just drain any stale QR result and re-trigger the Effect.
    let handle_reset = move |_: web_sys::MouseEvent| {
        let _ = check_qr_result_js(); // drain stale result
        let _ = check_camera_error_js(); // drain stale error
        set_camera_error.set(None);
        set_check_in_state.set(CheckInState::Idle);
        set_manual_input.set(String::new());
        set_scan_round.update(|r| *r += 1);
    };

    // Handle sign out
    let handle_sign_out = move |_: web_sys::MouseEvent| {
        auth::logout();
    };

    view! {
        <div>
            <components::AppHeader
                title="🎫 Scanner"
                user_email=user_email
                on_sign_out=handle_sign_out
            />

            <div class="page-container">
                // Tab selector
                <div class="tabs">
                    <button
                        class=move || {
                            if active_tab.get() == ScannerTab::Scanner {
                                "tab active"
                            } else {
                                "tab"
                            }
                        }
                        on:click=move |_| set_active_tab.set(ScannerTab::Scanner)
                    >
                        "📷 Scanner"
                    </button>
                    <button
                        class=move || {
                            if active_tab.get() == ScannerTab::Manual {
                                "tab active"
                            } else {
                                "tab"
                            }
                        }
                        on:click=move |_| set_active_tab.set(ScannerTab::Manual)
                    >
                        "✏️ Manual"
                    </button>
                </div>

                // Scanner tab (camera QR scanner) — always in DOM to avoid
                // race between Effect and <Show> mounting the <video> element.
                // Visibility toggled via CSS display instead of conditional rendering.
                // Hidden when showing attendee info (non-Idle state) so staff can focus.
                <div style=move || {
                    let scanning = active_tab.get() == ScannerTab::Scanner
                        && matches!(check_in_state.get(), CheckInState::Idle);
                    if scanning { "" } else { "display:none" }
                }>
                    <div class="card" style="padding:0;overflow:hidden;position:relative;">
                        <video
                            id="scanner-video"
                            autoplay=true
                            playsinline=true
                            muted=true
                            style="width:100%;display:block;border-radius:8px;min-height:200px;background:#1a1a2e;"
                        />
                        // Scanning overlay indicator
                        <div style="position:absolute;top:50%;left:50%;transform:translate(-50%,-50%);pointer-events:none;">
                            <div style=move || {
                                if camera_error.get().is_none() {
                                    "width:180px;height:180px;border:3px solid rgba(99,102,241,0.7);border-radius:12px;box-shadow:0 0 0 2000px rgba(0,0,0,0.3);"
                                } else {
                                    "display:none;"
                                }
                            } />
                        </div>
                    </div>
                    // Camera error message
                    <Show
                        when=move || camera_error.get().is_some()
                        fallback=|| view! { <div></div> }
                    >
                        <div class="error-msg visible" style="margin-top:0.75rem;">
                            {move || camera_error.get().unwrap_or_default()}
                        </div>
                    </Show>
                    // Hint when camera is active
                    <Show
                        when=move || camera_error.get().is_none()
                        fallback=|| view! { <div></div> }
                    >
                        <p style="color:var(--text-muted);font-size:0.8rem;text-align:center;margin-top:0.5rem;">
                            "Point your camera at a QR code to scan"
                        </p>
                    </Show>
                </div>

                // Manual tab — always in DOM, toggled via CSS display
                <div style=move || {
                    if active_tab.get() == ScannerTab::Manual {
                        ""
                    } else {
                        "display:none"
                    }
                }>
                    <div class="card">
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
                </div>

                // Check-in state display
                <div class="mt-2">
                    {move || {
                        let state = check_in_state.get();
                        render_check_in_state(state, handle_check_in, handle_reset)
                    }}
                </div>
            </div>

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
) {
    let attendee_id = id.to_string();
    set_state.set(CheckInState::LookingUp);
    leptos::task::spawn_local(async move {
        match api::get_attendee(&attendee_id).await {
            Ok(data) => {
                if data.is_checked_in {
                    set_state.set(CheckInState::AlreadyCheckedIn(Box::new(data)));
                } else if !data.is_approved {
                    set_state.set(CheckInState::NotApproved(Box::new(data)));
                } else if !data.is_in_person {
                    set_state.set(CheckInState::NotInPerson(Box::new(data)));
                } else {
                    set_state.set(CheckInState::Found(Box::new(data)));
                }
            }
            Err(err) => {
                log::warn!("[scanner] attendee lookup failed for id={attendee_id}: {err}");
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
    if trimmed.starts_with("http") {
        if let Ok(url) = web_sys::Url::new(trimmed) {
            if let Some(scan) = url.search_params().get("scan") {
                return Some(scan);
            }
            if let Some(id_param) = url.search_params().get("id") {
                return Some(id_param);
            }
        }
    }

    // Return as-is (gst- prefix or raw ID)
    Some(trimmed.to_string())
}

// ===== State View Rendering =====

/// Render the current check-in state as a view.
fn render_check_in_state(
    state: CheckInState,
    on_check_in: impl Fn(web_sys::MouseEvent) + 'static,
    on_reset: impl Fn(web_sys::MouseEvent) + 'static,
) -> AnyView {
    match state {
        CheckInState::Idle => view! { <div></div> }.into_any(),
        CheckInState::LookingUp => view! {
            <div class="card text-center">
                <div class="page-loading" style="min-height:auto;padding:1rem;">
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
                <div class="card">
                    <div class="text-center mb-2">
                        <div style="font-size:2.5rem;">"✅"</div>
                        <h2>"Ready to Check In"</h2>
                    </div>
                    <div class="card" style="background:var(--bg-secondary);">
                        <p style="font-weight:600;color:#fff;font-size:1rem;margin-bottom:0.25rem;">
                            {name}
                        </p>
                        <p style="color:var(--text-secondary);font-size:0.85rem;margin-bottom:0.25rem;">
                            {email}
                        </p>
                        <div style="display:flex;gap:0.5rem;margin-top:0.5rem;">
                            <span class="badge badge-info">{ticket}</span>
                            <span class=badge.css_class>{badge.label}</span>
                        </div>
                    </div>
                    <div style="display:flex;gap:0.5rem;margin-top:1rem;">
                        <button class="btn btn-success btn-block" on:click=on_check_in>
                            "✓ Confirm Check-In"
                        </button>
                    </div>
                    <button
                        class="btn btn-outline btn-block"
                        style="margin-top:0.5rem;"
                        on:click=on_reset
                    >
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
            let claim_url = data.claim_url.clone();
            let qr_data_url = claim_url
                .as_ref()
                .and_then(|url| generate_qr_data_url(url, 200));
            let claim_url_for_display = claim_url.clone();
            view! {
                <div class="card">
                    <div class="result-warning">
                        <div style="font-size:2.5rem;">"⚠️"</div>
                        <h2>"Already Checked In"</h2>
                        <div class="result-details">
                            <p style="font-weight:600;color:#fff;">{name}</p>
                            <p>{email}</p>
                            <p style="margin-top:0.5rem;">
                                "Checked in at: "
                                {formatted}{by_suffix}
                            </p>
                        </div>
                    </div>

                    // Claim URL QR code — re-show in case staff needs to display it again
                    {move || {
                        match (&qr_data_url, &claim_url_for_display) {
                            (Some(img_src), Some(url)) => {
                                let url_for_copy = url.clone();
                                view! {
                                    <div style="margin-top:1.25rem;text-align:center;">
                                        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem;">
                                            <p style="font-size:0.8rem;color:var(--text-secondary);margin-bottom:0.75rem;">
                                                "Claim QR (show to attendee):"
                                            </p>
                                            <img
                                                src=img_src
                                                alt="Claim URL QR Code"
                                                style="width:200px;height:200px;border-radius:8px;"
                                            />
                                            <div style="margin-top:0.75rem;display:flex;gap:0.5rem;">
                                                <button
                                                    class="btn btn-primary btn-sm"
                                                    style="flex:1;"
                                                    on:click=move |_| {
                                                        let _ = copy_to_clipboard_js(&url_for_copy);
                                                    }
                                                >
                                                    "📋 Copy Link"
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    <button
                        class="btn btn-outline btn-block"
                        style="margin-top:1rem;"
                        on:click=on_reset
                    >
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
                <div class="card">
                    <div class="result-error">
                        <div style="font-size:2.5rem;">"🚫"</div>
                        <h2>"Not Approved"</h2>
                        <div class="result-details">
                            <p style="font-weight:600;color:#fff;">{name}</p>
                            <p>{email}</p>
                            <p style="margin-top:0.5rem;">
                                "Status: "
                                <span style="color:var(--warning);">{status}</span>
                            </p>
                        </div>
                    </div>
                    <button
                        class="btn btn-outline btn-block"
                        style="margin-top:1rem;"
                        on:click=on_reset
                    >
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
            view! {
                <div class="card">
                    <div class="result-warning">
                        <div style="font-size:2.5rem;">"💻"</div>
                        <h2>"Not In-Person"</h2>
                        <div class="result-details">
                            <p style="font-weight:600;color:#fff;">{name}</p>
                            <p>{email}</p>
                            <p>
                                "Participation: "
                                <span style="color:var(--warning);">{badge.label}</span>
                            </p>
                        </div>
                    </div>
                    <button
                        class="btn btn-outline btn-block"
                        style="margin-top:1rem;"
                        on:click=on_reset
                    >
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotFound => view! {
            <div class="card">
                <div class="result-error">
                    <div style="font-size:2.5rem;">"❌"</div>
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
            <div class="card text-center">
                <div class="page-loading" style="min-height:auto;padding:1rem;">
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
            let claim_url = result.claim_url.clone();
            let qr_data_url = claim_url
                .as_ref()
                .and_then(|url| generate_qr_data_url(url, 200));

            let claim_url_for_display = claim_url.clone();
            view! {
                <div class="card">
                    <div class="result-success">
                        <div style="font-size:2.5rem;">"🎉"</div>
                        <h2>"Checked In!"</h2>
                        <div class="result-details">
                            <p style="font-weight:600;color:#fff;">{name}</p>
                            <p>"Checked in at: "{formatted}{by_suffix}</p>
                        </div>
                    </div>

                    // Claim URL QR code — show to attendee so they can scan it
                    {move || {
                        match (&qr_data_url, &claim_url_for_display) {
                            (Some(img_src), Some(url)) => {
                                let url_for_copy = url.clone();
                                view! {
                                    <div style="margin-top:1.25rem;text-align:center;">
                                        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem;">
                                            <p style="font-size:0.8rem;color:var(--text-secondary);margin-bottom:0.75rem;">
                                                "Show this QR to the attendee to claim their NFT:"
                                            </p>
                                            <img
                                                src=img_src
                                                alt="Claim URL QR Code"
                                                style="width:200px;height:200px;border-radius:8px;"
                                            />
                                            <div style="margin-top:0.75rem;display:flex;gap:0.5rem;">
                                                <button
                                                    class="btn btn-primary btn-sm"
                                                    style="flex:1;"
                                                    on:click=move |_| {
                                                        let _ = copy_to_clipboard_js(&url_for_copy);
                                                    }
                                                >
                                                    "📋 Copy Link"
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    <button
                        class="btn btn-success btn-block"
                        style="margin-top:1rem;"
                        on:click=on_reset
                    >
                        "Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::Error => view! {
            <div class="card">
                <div class="result-error">
                    <div style="font-size:2.5rem;">"❌"</div>
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
    }
}
