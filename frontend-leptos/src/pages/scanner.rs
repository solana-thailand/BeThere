use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::{self, AttendeeData, CheckInData};
use crate::auth::{self, get_token_email, handle_token_from_url, is_authenticated};

/// Current state of the check-in flow.
#[derive(Clone)]
enum CheckInState {
    /// No active check-in.
    Idle,
    /// Looking up an attendee by ID.
    LookingUp,
    /// Attendee found, ready to confirm check-in.
    Found(Box<AttendeeData>),
    /// Attendee is already checked in.
    AlreadyCheckedIn(Box<AttendeeData>),
    /// Attendee is not approved.
    NotApproved(Box<AttendeeData>),
    /// Attendee not found.
    NotFound(String),
    /// Performing the check-in POST.
    CheckingIn { name: String, id: String },
    /// Check-in succeeded.
    Success(Box<CheckInData>),
    /// An error occurred at any step.
    Error(String),
}

/// Active tab in the scanner page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScannerTab {
    Scanner,
    Manual,
}

/// Staff scanner page component.
///
/// Provides QR scanning (placeholder) and manual attendee ID entry
/// for performing event check-ins.
#[component]
pub fn Scanner() -> impl IntoView {
    let navigate = use_navigate();

    // Reactive state
    let (active_tab, set_active_tab) = signal(ScannerTab::Manual);
    let (manual_input, set_manual_input) = signal(String::new());
    let (check_in_state, set_check_in_state) = signal(CheckInState::Idle);
    let (toast, set_toast) = signal(None::<ToastMessage>);
    let (user_email, set_user_email) = signal(String::new());

    // On mount: handle token from URL, check auth, load user info
    Effect::new(move |_| {
        // Handle token from OAuth callback
        handle_token_from_url();

        // Require authentication
        if !is_authenticated() {
            auth::clear_token();
            navigate("/", Default::default());
            return;
        }

        // Load user info
        spawn_local(async move {
            match api::get_me().await {
                Ok(me) => {
                    set_user_email.set(me.email);
                }
                Err(_) => {
                    log::warn!("[scanner] failed to load user info");
                }
            }
        });

        // Check for scan param from QR code URL
        let window = web_sys::window().expect("no window");
        if let Ok(url_str) = window.location().href() {
            if let Ok(url) = web_sys::Url::new(&url_str) {
                if let Some(scan_id) = url.search_params().get("scan") {
                    // Clean up URL
                    url.search_params().delete("scan");
                    let clean_path = url.pathname();
                    let _ = window.history().and_then(|h| {
                        h.replace_state_with_url(
                            &wasm_bindgen::JsValue::NULL,
                            "",
                            Some(&clean_path),
                        )
                    });
                    // Process the scanned ID
                    process_attendee_id(&scan_id, set_check_in_state, set_toast);
                }
            }
        }
    });

    // Handle manual form submission
    let handle_manual_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let value = manual_input.get().trim().to_string();
        if value.is_empty() {
            show_toast(&set_toast, "Please enter an attendee ID", ToastType::Warning);
            return;
        }
        let attendee_id = extract_attendee_id(&value);
        if let Some(id) = attendee_id {
            process_attendee_id(&id, set_check_in_state, set_toast);
        } else {
            show_toast(&set_toast, "Invalid attendee ID format", ToastType::Error);
        }
    };

    // Handle check-in confirmation
    let handle_check_in = move |_| {
        let state = check_in_state.get();
        match &*state {
            CheckInState::Found(data) => {
                let id = data.attendee.api_id.clone();
                let name = data.attendee.name.clone();
                let id_for_check = id.clone();
                let name_for_check = name.clone();
                set_check_in_state.set(CheckInState::CheckingIn {
                    name: name_for_check,
                    id: id_for_check,
                });
                let set_state = set_check_in_state;
                let set_t = set_toast;
                spawn_local(async move {
                    match api::check_in(&id).await {
                        Ok(result) => {
                            log::info!("[scanner] check-in successful: {}", result.name);
                            set_state.set(CheckInState::Success(Box::new(result)));
                            show_toast(
                                &set_t,
                                &format!("{name} checked in successfully!"),
                                ToastType::Success,
                            );
                        }
                        Err(err) => {
                            log::error!("[scanner] check-in failed: {err}");
                            set_state.set(CheckInState::Error(format!("{err}")));
                            show_toast(&set_t, &format!("{err}"), ToastType::Error);
                        }
                    }
                });
            }
            _ => {}
        }
    };

    // Reset scanner to idle state
    let handle_reset = move |_| {
        set_check_in_state.set(CheckInState::Idle);
        set_manual_input.set(String::new());
    };

    // Handle sign out
    let handle_sign_out = move |_| {
        auth::logout(&|path: &str| navigate(path, Default::default()));
    };

    view! {
        <div>
            // Header
            <header class="header">
                <div class="header-inner">
                    <span class="header-title">"🎫 Scanner"</span>
                    <div style="display:flex;align-items:center;gap:0.75rem;">
                        <span class="header-user">{move || user_email.get()}</span>
                        <button class="btn btn-outline btn-sm" on:click=handle_sign_out>
                            "Sign Out"
                        </button>
                    </div>
                </div>
            </header>

            <div class="page-container">
                // Tabs
                <div class="tabs">
                    <button
                        class=move || {
                            if active_tab.get() == ScannerTab::Scanner { "tab active" } else { "tab" }
                        }
                        on:click=move |_| set_active_tab.set(ScannerTab::Scanner)
                    >
                        "📷 Scanner"
                    </button>
                    <button
                        class=move || {
                            if active_tab.get() == ScannerTab::Manual { "tab active" } else { "tab" }
                        }
                        on:click=move |_| set_active_tab.set(ScannerTab::Manual)
                    >
                        "✏️ Manual"
                    </button>
                </div>

                // Scanner tab content
                <Show
                    when=move || active_tab.get() == ScannerTab::Scanner
                    fallback=|| view! { <div></div> }
                >
                    <div class="card">
                        <div class="scanner-placeholder">
                            <div class="icon">"📷"</div>
                            <p style="color:var(--text-secondary);margin-bottom:0.5rem;">
                                "Camera QR scanning"
                            </p>
                            <p style="color:var(--text-muted);font-size:0.85rem;">
                                "Use the Manual tab to enter attendee IDs"
                            </p>
                        </div>
                    </div>
                </Show>

                // Manual tab content
                <Show
                    when=move || active_tab.get() == ScannerTab::Manual
                    fallback=|| view! { <div></div> }
                >
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
                                        &*check_in_state.get(),
                                        CheckInState::LookingUp | CheckInState::CheckingIn { .. }
                                    )
                                >
                                    "Look Up"
                                </button>
                            </div>
                        </form>
                    </div>
                </Show>

                // Check-in state display
                <div class="mt-2">
                    {move || render_check_in_state(&check_in_state.get(), handle_check_in, handle_reset)}
                </div>
            </div>

            // Toast notification
            <Toast toast_signal=toast />
        </div>
    }
}

/// Process an attendee ID through the lookup flow.
fn process_attendee_id(
    id: &str,
    set_state: WriteSignal<CheckInState>,
    set_toast: WriteSignal<Option<ToastMessage>>,
) {
    set_state.set(CheckInState::LookingUp);
    let id = id.to_string();
    let set_state_clone = set_state;
    let set_toast_clone = set_toast;

    spawn_local(async move {
        match api::get_attendee(&id).await {
            Ok(data) => {
                if data.is_checked_in {
                    set_state_clone.set(CheckInState::AlreadyCheckedIn(Box::new(data)));
                } else if !data.is_approved {
                    set_state_clone.set(CheckInState::NotApproved(Box::new(data)));
                } else {
                    set_state_clone.set(CheckInState::Found(Box::new(data)));
                }
            }
            Err(err) => {
                log::warn!("[scanner] attendee lookup failed: {err}");
                set_state_clone.set(CheckInState::NotFound(id.clone()));
                show_toast(
                    &set_toast_clone,
                    &format!("Attendee not found: {id}"),
                    ToastType::Error,
                );
            }
        }
    });
}

/// Extract attendee ID from scanned QR content or manual input.
/// Supports formats:
/// - Full URL: https://example.com/staff?scan=gst-abc123
/// - Direct ID: gst-abc123
/// - API ID only: abc123
fn extract_attendee_id(text: &str) -> Option<String> {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Try to extract from URL parameter
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

/// Format an ISO timestamp to a readable string.
fn format_timestamp(iso: &str) -> String {
    if iso.is_empty() {
        return "N/A".to_string();
    }
    // Use js_sys::Date for formatting in WASM
    let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(
        0, 0, 0, 0, 0, 0.0,
    );
    js_date.set_time(js_sys::Date::parse(iso));
    if js_date.get_time().is_nan() {
        return iso.to_string();
    }
    // Simple formatting via to_locale_string
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("year"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    )
    .ok();
    js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("month"),
        &wasm_bindgen::JsValue::from_str("short"),
    )
    .ok();
    js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("day"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    )
    .ok();
    js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("hour"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    )
    .ok();
    js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("minute"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    )
    .ok();

    js_date
        .to_locale_string("en-US", &opts)
        .as_string()
        .unwrap_or_else(|| iso.to_string())
}

/// Render the current check-in state as a view.
#[component]
fn CheckInStateView(
    state: CheckInState,
    on_check_in: ev::EventHandler<leptos::ev::ClickEvent>,
    on_reset: ev::EventHandler<leptos::ev::ClickEvent>,
) -> impl IntoView {
    match state {
        CheckInState::Idle => view! { <div></div> }.into_any(),
        CheckInState::LookingUp => {
            view! {
                <div class="card text-center">
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Looking up attendee..."
                    </div>
                </div>
            }
            .into_any()
        }
        CheckInState::Found(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let ticket = data.attendee.ticket_name.clone();
            view! {
                <div class="result-success" style="background:var(--info-bg);border-color:var(--info-border);">
                    <h3 style="color:var(--info);">"Attendee Found"</h3>
                    <div class="result-details mt-1">
                        <p><strong>"Name: "</strong>{name}</p>
                        <p><strong>"Email: "</strong>{email}</p>
                        <p><strong>"Ticket: "</strong>{ticket}</p>
                    </div>
                    <div class="mt-2" style="display:flex;gap:0.5rem;justify-content:center;">
                        <button class="btn btn-success" on:click=on_check_in>
                            "✅ Confirm Check-In"
                        </button>
                        <button class="btn btn-outline" on:click=on_reset>
                            "Cancel"
                        </button>
                    </div>
                </div>
            }
            .into_any()
        }
        CheckInState::AlreadyCheckedIn(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let checked_at = data
                .attendee
                .checked_in_at
                .as_deref()
                .unwrap_or("unknown time");
            let formatted = format_timestamp(checked_at);
            view! {
                <div class="result-warning">
                    <div class="icon">"⚠️"</div>
                    <h2>"Already Checked In"</h2>
                    <p class="mt-1">{format!("{name} was already checked in.")}</p>
                    <div class="result-details">
                        <p><strong>"Name: "</strong>{name}</p>
                        <p><strong>"Email: "</strong>{email}</p>
                        <p><strong>"Checked in: "</strong>{formatted}</p>
                    </div>
                    <button class="btn btn-outline btn-sm mt-2" on:click=on_reset>
                        "New Scan"
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
                <div class="result-error">
                    <div class="icon">"❌"</div>
                    <h2>"Not Approved"</h2>
                    <p class="mt-1">{format!("{name} has not been approved for check-in.")}</p>
                    <div class="result-details">
                        <p><strong>"Name: "</strong>{name}</p>
                        <p><strong>"Email: "</strong>{email}</p>
                        <p><strong>"Status: "</strong>{status}</p>
                    </div>
                    <button class="btn btn-outline btn-sm mt-2" on:click=on_reset>
                        "New Scan"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotFound(id) => {
            view! {
                <div class="result-error">
                    <div class="icon">"❌"</div>
                    <h2>"Not Found"</h2>
                    <p class="mt-1">{format!("No attendee found with ID: {id}")}</p>
                    <button class="btn btn-outline btn-sm mt-2" on:click=on_reset>
                        "Try Again"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::CheckingIn { name, .. } => {
            view! {
                <div class="card text-center">
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        {format!("Checking in {name}...")}
                    </div>
                </div>
            }
            .into_any()
        }
        CheckInState::Success(data) => {
            let formatted = format_timestamp(&data.checked_in_at);
            view! {
                <div class="result-success">
                    <div class="icon">"✅"</div>
                    <h2>"Checked In!"</h2>
                    <p class="mt-1">{&data.message}</p>
                    <div class="result-details">
                        <p><strong>"Name: "</strong>{&data.name}</p>
                        <p><strong>"ID: "</strong>{&data.api_id}</p>
                        <p><strong>"Time: "</strong>{formatted}</p>
                    </div>
                    <button class="btn btn-primary btn-sm mt-2" on:click=on_reset>
                        "🔄 New Scan"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::Error(msg) => {
            view! {
                <div class="result-error">
                    <div class="icon">"❌"</div>
                    <h2>"Error"</h2>
                    <p class="mt-1">{msg}</p>
                    <button class="btn btn-outline btn-sm mt-2" on:click=on_reset>
                        "Try Again"
                    </button>
                </div>
            }
            .into_any()
        }
    }
}

/// Helper to render check-in state without needing a full component for each variant.
fn render_check_in_state(
    state: &CheckInState,
    on_check_in: impl Fn(leptos::ev::ClickEvent) + Clone + 'static,
    on_reset: impl Fn(leptos::ev::ClickEvent) + Clone + 'static,
) -> AnyView {
    let state = state.clone();
    let on_check_in = on_check_in;
    let on_reset = on_reset;
    view! {
        <CheckInStateView state state=on_check_in on:click=on_reset on_reset=on_reset />
    }
    .into_any()
}

// ===== Toast Notification =====

/// Toast notification type.
#[derive(Clone)]
struct ToastMessage {
    text: String,
    toast_type: ToastType,
}

#[derive(Clone, Copy)]
enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}

/// Show a toast notification that auto-dismisses after 4 seconds.
fn show_toast(
    set_toast: &WriteSignal<Option<ToastMessage>>,
    text: &str,
    toast_type: ToastType,
) {
    set_toast.set(Some(ToastMessage {
        text: text.to_string(),
        toast_type,
    }));

    // Auto-dismiss after 4 seconds
    let set_toast = *set_toast;
    set_timeout(
        move || {
            set_toast.set(None);
        },
        std::time::Duration::from_secs(4),
    );
}

/// Toast notification component.
#[component]
fn Toast(toast_signal: ReadSignal<Option<ToastMessage>>) -> impl IntoView {
    view! {
        <Show
            when=move || toast_signal.get().is_some()
            fallback=|| view! { <div></div> }
        >
            {move || {
                let msg = toast_signal.get();
                match msg {
                    Some(m) => {
                        let class = match m.toast_type {
                            ToastType::Success => "toast toast-success",
                            ToastType::Error => "toast toast-error",
                            ToastType::Warning => "toast toast-warning",
                            ToastType::Info => "toast toast-info",
                        };
                        view! {
                            <div class=class style="position:fixed;top:1rem;right:1rem;z-index:9999;max-width:360px;">
                                {m.text}
                            </div>
                        }
                            .into_any()
                    }
                    None => view! { <div></div> }.into_any(),
                }
            }}
        </Show>
    }
}
