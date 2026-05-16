//! Public event page — displays event details to attendees (no auth required).
//!
//! Accessed via `/e/:slug`. Shows event info, countdown timer, deposit details,
//! NFT badge preview, self-registration form, and external link.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::icons::{Icon, IconName};

// ---------------------------------------------------------------------------
// Registration API types
// ---------------------------------------------------------------------------

/// Request body for POST /api/public/register.
#[derive(serde::Serialize)]
struct RegisterBody {
    slug: String,
    name: String,
    email: String,
    participation_type: Option<String>,
    contact_channel: Option<String>,
    contact_handle: Option<String>,
    deposit_agreed: Option<bool>,
}

/// Next step returned after registration.
#[derive(serde::Deserialize, Clone, Debug)]
struct NextStep {
    #[serde(rename = "type")]
    _step_type: String,
    url: String,
}

/// Response from POST /api/public/register.
#[derive(serde::Deserialize)]
struct RegisterResponse {
    success: bool,
    data: Option<RegisterData>,
    error: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct RegisterData {
    attendee_id: String,
    name: String,
    email: String,
    claim_token: String,
    next_step: NextStep,
}

/// Registration form state.
#[derive(Clone, Debug)]
enum RegState {
    Idle,
    Submitting,
    Success(RegisterData),
    Error(String),
}

// ---------------------------------------------------------------------------
// Route params
// ---------------------------------------------------------------------------

/// Route parameters for `/e/:slug`.
#[derive(Params, PartialEq, Clone)]
struct PublicEventParams {
    slug: Option<String>,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Public event data returned by `GET /api/public/event/{slug}`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct PublicEventData {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub tagline: String,
    pub link: String,
    pub status: String,
    pub event_start_ms: i64,
    pub event_end_ms: i64,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: u64,
    pub deposit_amount_thb: u64,
    pub event_format: crate::api::EventFormat,
    pub nft_image_url: String,
    pub nft_name_template: String,
    pub nft_symbol: String,
    pub nft_description_template: String,
    pub quiz_enabled: bool,
    pub refund_deadline_hours: u64,
    pub require_contact_info: bool,
    pub description: String,
    pub location: String,
    pub created_at: String,
    pub dev_mode: bool,
}

/// API response wrapper for public event endpoint.
#[derive(serde::Deserialize)]
struct PublicEventResponse {
    success: bool,
    data: Option<PublicEventData>,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Page state
// ---------------------------------------------------------------------------

/// Top-level state for the public event page.
#[derive(Clone, Debug)]
enum PublicEventState {
    /// Loading event data from backend.
    Loading,
    /// Event loaded successfully.
    Loaded(PublicEventData),
    /// Event not found (404).
    NotFound,
    /// API error.
    Error(String),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format USDC amount from smallest unit (6 decimals) to display string.
fn format_usdc(amount: u64) -> String {
    format!("${:.2} USDC", amount as f64 / 1_000_000.0)
}

/// Format THB amount (already in whole baht).
fn format_thb(amount: u64) -> String {
    format!("฿{amount}")
}

/// Format a millisecond timestamp as a human-readable date string.
fn format_event_date(ms: i64) -> String {
    let js_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("year"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("month"),
        &wasm_bindgen::JsValue::from_str("short"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("day"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );

    js_date
        .to_locale_string("en-US", &opts)
        .as_string()
        .unwrap_or_else(|| "TBA".to_string())
}

/// Format a millisecond timestamp as time string (e.g. "10:00 AM").
fn format_event_time(ms: i64) -> String {
    let js_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("hour"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("minute"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    );

    js_date
        .to_locale_string("en-US", &opts)
        .as_string()
        .unwrap_or_else(|| "TBA".to_string())
}

/// Format remaining time as "Xd Xh Xm Xs" countdown string.
fn format_countdown(remaining_ms: i64) -> String {
    if remaining_ms <= 0 {
        return String::new();
    }

    let total_secs = remaining_ms / 1000;
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m {secs}s")
    } else if hours > 0 {
        format!("{hours}h {mins}m {secs}s")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

/// Format refund deadline hours as human-readable (e.g. "7 days").
fn format_refund_deadline(hours: u64) -> String {
    if hours >= 24 {
        let days = hours / 24;
        if days == 1 {
            "1 day".to_string()
        } else {
            format!("{days} days")
        }
    } else if hours == 1 {
        "1 hour".to_string()
    } else {
        format!("{hours} hours")
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Public event page component.
///
/// Displays event details for a given slug without requiring authentication.
#[component]
pub fn PublicEvent() -> impl IntoView {
    let params = use_params::<PublicEventParams>();

    // Reactive state
    let (state, set_state) = signal(PublicEventState::Loading);
    let (countdown, set_countdown) = signal(String::new());
    let (event_completed, set_event_completed) = signal(false);
    let (event_name, set_event_name) = signal(String::new());

    // Fetch event data on mount
    Effect::new(move |_| {
        let slug = match params.get() {
            Ok(p) => p.slug.unwrap_or_default(),
            Err(_) => {
                set_state.set(PublicEventState::NotFound);
                return;
            }
        };

        if slug.is_empty() {
            set_state.set(PublicEventState::NotFound);
            return;
        }

        let slug_clone = slug.clone();
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/public/event/{slug_clone}");

            match gloo::net::http::Request::get(&url).send().await {
                Ok(resp) => {
                    if resp.status() == 404 {
                        set_state.set(PublicEventState::NotFound);
                        return;
                    }

                    match resp.text().await {
                        Ok(body) => {
                            match serde_json::from_str::<PublicEventResponse>(&body) {
                                Ok(api_resp) => {
                                    if api_resp.success {
                                        if let Some(data) = api_resp.data {
                                            let is_completed =
                                                data.status == "completed" || data.status == "Completed";
                                            let start_ms = data.event_start_ms;
                                            let name = data.name.clone();
                                            set_event_name.set(name);
                                            set_event_completed.set(is_completed);
                                            set_state.set(PublicEventState::Loaded(data));

                                            // Start countdown if event is in the future
                                            let now_ms = js_sys::Date::now() as i64;
                                            if !is_completed && start_ms > now_ms {
                                                set_countdown
                                                    .set(format_countdown(start_ms - now_ms));

                                                // Tick every second
                                                set_interval(
                                                    move || {
                                                        let now = js_sys::Date::now() as i64;
                                                        let remaining = start_ms - now;
                                                        if remaining <= 0 {
                                                            set_countdown
                                                                .set(String::new());
                                                        } else {
                                                            set_countdown.set(
                                                                format_countdown(remaining),
                                                            );
                                                        }
                                                    },
                                                    std::time::Duration::from_secs(1),
                                                );
                                            }
                                        } else {
                                            set_state.set(PublicEventState::Error(
                                                "No event data returned".to_string(),
                                            ));
                                        }
                                    } else {
                                        set_state.set(PublicEventState::Error(
                                            api_resp
                                                .error
                                                .unwrap_or_else(|| "Unknown error".to_string()),
                                        ));
                                    }
                                }
                                Err(e) => {
                                    log::error!("[public_event] JSON parse error: {e}");
                                    set_state.set(PublicEventState::Error(format!(
                                        "Failed to parse response: {e}"
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[public_event] body read error: {e}");
                            set_state.set(PublicEventState::Error(
                                "Failed to read response".to_string(),
                            ));
                        }
                    }
                }
                Err(e) => {
                    log::error!("[public_event] fetch error: {e}");
                    set_state.set(PublicEventState::Error(format!(
                        "Failed to fetch event: {e}"
                    )));
                }
            }
        });
    });

    // Dynamic title
    let title_text = move || {
        let name = event_name.get();
        if name.is_empty() {
            "Event — BeThere".to_string()
        } else {
            format!("{name} — BeThere")
        }
    };

    view! {
        <Title text=title_text />
        <div class="center-page">
            <div class="container layout-col-center" style="gap:0;">

                // Back link
                <div style="width:100%;margin-bottom:1rem;">
                    <a href="/" style="color:var(--text-secondary);font-size:0.85rem;text-decoration:none;display:inline-flex;align-items:center;gap:0.3rem;">
                        "← Back to BeThere"
                    </a>
                </div>

                // Loading state
                {move || {
                    let s = state.get();
                    match s {
                        PublicEventState::Loading => {
                            view! {
                                <div style="text-align:center;padding:3rem 0;">
                                    <div style="margin-bottom:1rem;"><Icon icon=IconName::Ticket class="icon-2xl" /></div>
                                    <p style="color:var(--text-secondary);">"Loading event..."</p>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::NotFound => {
                            view! {
                                <div style="text-align:center;padding:3rem 0;">
                                    <div style="margin-bottom:1rem;"><Icon icon=IconName::Search class="icon-2xl" /></div>
                                    <h1 style="font-size:1.5rem;color:var(--text-primary);margin-bottom:0.5rem;">"Event Not Found"</h1>
                                    <p style="color:var(--text-secondary);margin-bottom:1.5rem;">
                                        "This event doesn't exist or is not publicly available."
                                    </p>
                                    <a href="/" class="btn btn-primary">"Go Home"</a>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::Error(msg) => {
                            view! {
                                <div style="text-align:center;padding:3rem 0;">
                                    <div style="margin-bottom:1rem;"><Icon icon=IconName::Warning class="icon-md icon-danger" /></div>
                                    <h1 style="font-size:1.5rem;color:var(--text-primary);margin-bottom:0.5rem;">"Something went wrong"</h1>
                                    <p style="color:var(--text-secondary);margin-bottom:1.5rem;">{msg}</p>
                                    <a href="/" class="btn btn-primary">"Go Home"</a>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::Loaded(data) => {
                            render_loaded_event(data, countdown, event_completed)
                        }
                    }
                }}

                // Footer
                <div style="width:100%;margin-top:2rem;padding-top:1.5rem;border-top:1px solid var(--border);text-align:center;">
                    <p style="color:var(--text-secondary);font-size:0.8rem;">
                        "Powered by "
                        <a href="/" style="color:var(--accent);text-decoration:none;">"BeThere"</a>
                    </p>
                </div>
            </div>
        </div>
    }
}

/// Render the loaded event content.
fn render_loaded_event(
    data: PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
) -> AnyView {
    let has_nft_image = !data.nft_image_url.is_empty();
    let has_description = !data.description.is_empty();
    let has_link = !data.link.is_empty();
    let has_deposit = data.deposit_enabled && (data.deposit_amount_usdc > 0 || data.deposit_amount_thb > 0);
    let has_location = !data.location.is_empty();
    let is_hybrid = data.event_format == crate::api::EventFormat::Hybrid;
    let date_str = format_event_date(data.event_start_ms);
    let time_str = format!(
        "{} — {}",
        format_event_time(data.event_start_ms),
        format_event_time(data.event_end_ms)
    );
    let nft_image_url = data.nft_image_url.clone();
    let nft_image_url_2 = data.nft_image_url.clone();
    let _link = data.link.clone();
    let link_2 = data.link.clone();
    let name = data.name.clone();
    let tagline = data.tagline.clone();
    let description = data.description.clone();
    let location = data.location.clone();
    let slug_for_reg = data.slug.clone();
    let usdc_display = format_usdc(data.deposit_amount_usdc);
    let thb_display = if data.deposit_amount_thb > 0 {
        Some(format_thb(data.deposit_amount_thb))
    } else {
        None
    };
    let refund_label = format_refund_deadline(data.refund_deadline_hours);
    let deposit_thb = data.deposit_amount_thb;
    let _show_deposit_cta = has_deposit && !event_completed.get();
    let show_reg_form = !event_completed.get();
    let require_contact = data.require_contact_info;

    // Registration form signals
    let (reg_name, set_reg_name) = signal(String::new());
    let (reg_email, set_reg_email) = signal(String::new());
    let (reg_participation, set_reg_participation) = signal(String::new());
    let (reg_contact_channel, set_reg_contact_channel) = signal(String::new());
    let (reg_contact_handle, set_reg_contact_handle) = signal(String::new());
    let (reg_deposit_agreed, set_reg_deposit_agreed) = signal(false);
    let (reg_state, set_reg_state) = signal(RegState::Idle);

    // Resume: check localStorage for saved progress on this event.
    // If attendee already registered but didn't finish deposit, redirect them.
    let slug_for_resume = slug_for_reg.clone();
    leptos::task::spawn_local(async move {
        let progress_json = js_sys::eval("localStorage.getItem('bethere_progress')")
            .ok()
            .and_then(|v| v.as_string());
        if let Some(json_str) = progress_json {
            if !json_str.is_empty() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let saved_slug = parsed.get("slug").and_then(|v| v.as_str()).unwrap_or("");
                    if saved_slug == slug_for_resume {
                        let attendee_id = parsed.get("attendee_id").and_then(|v| v.as_str()).unwrap_or("");
                        let event_id = parsed.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
                        if !attendee_id.is_empty() && !event_id.is_empty() {
                            // Check deposit status to determine where to redirect
                            let window = web_sys::window().expect("no window");
                            let origin = window.location().origin().unwrap_or_else(|_| "http://localhost:8787".to_string());
                            let url = format!("{origin}/api/deposit/status/{attendee_id}?event_id={event_id}");
                            match gloo::net::http::Request::get(&url).send().await {
                                Ok(resp) => {
                                    if let Ok(body) = resp.text().await {
                                        if let Ok(status_resp) = serde_json::from_str::<serde_json::Value>(&body) {
                                            let has_deposit = status_resp
                                                .get("data")
                                                .and_then(|d| d.get("status"))
                                                .is_some();
                                            if has_deposit {
                                                // Already deposited → go to ticket
                                                let _ = js_sys::eval(&format!(
                                                    "window.location.href = '/ticket/{}?event_id={}'",
                                                    attendee_id, event_id
                                                ));
                                            } else {
                                                // Not deposited → go to deposit page
                                                let _ = js_sys::eval(&format!(
                                                    "window.location.href = '/deposit/{}?event_id={}'",
                                                    attendee_id, event_id
                                                ));
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    // API failed — just show the page normally
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    view! {
        // NFT Badge Image (hero)
        {move || {
            if has_nft_image {
                let url = nft_image_url.clone();
                view! {
                    <div style="width:100%;text-align:center;margin-bottom:1.5rem;">
                        <img
                            src=url
                            alt="Event Badge"
                            style="max-width:180px;max-height:180px;border-radius:var(--radius);box-shadow:var(--shadow);"
                        />
                    </div>
                }.into_any()
            } else {
                view! {
                    <div style="text-align:center;margin-bottom:1.5rem;">
                        <span><Icon icon=IconName::Ticket class="icon-2xl" /></span>
                    </div>
                }.into_any()
            }
        }}

        // Event Name + Tagline
        <div style="width:100%;text-align:center;margin-bottom:1.5rem;">
            <h1 style="font-size:1.6rem;font-weight:700;color:#fff;margin-bottom:0.4rem;line-height:1.25;">
                {name}
            </h1>
            {move || {
                if !tagline.is_empty() {
                    let t = tagline.clone();
                    view! {
                        <p style="color:var(--text-secondary);font-size:0.95rem;margin:0;">
                            {t}
                        </p>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}
        </div>

        // Event Details Card
        <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">

            // Location
            {move || {
                if has_location {
                    let loc = location.clone();
                    view! {
                        <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.75rem;">
                            <span><Icon icon=IconName::Pin class="icon-sm icon-muted" /></span>
                            <span style="color:var(--text-primary);font-size:0.95rem;">
                                {loc}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}

            // Date & Time
            <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.75rem;">
                <span><Icon icon=IconName::Calendar class="icon-sm icon-muted" /></span>
                <span style="color:var(--text-primary);font-size:0.95rem;">
                    {date_str}
                </span>
            </div>
            <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.75rem;margin-left:1.6rem;">
                <span style="color:var(--text-secondary);font-size:0.9rem;">
                    {time_str}
                </span>
            </div>

            // Countdown or Completed
            {move || {
                let completed = event_completed.get();
                if completed {
                    view! {
                        <div style="display:flex;align-items:center;gap:0.5rem;">
                            <span><Icon icon=IconName::Party class="icon-sm icon-success" /></span>
                            <span style="color:#34d399;font-weight:600;font-size:0.95rem;">
                                "Event Completed"
                            </span>
                        </div>
                    }.into_any()
                } else {
                    let cd = countdown.get();
                    if cd.is_empty() {
                        ().into_any()
                    } else {
                        view! {
                            <div style="display:flex;align-items:center;gap:0.5rem;">
                                <span><Icon icon=IconName::Timer class="icon-sm icon-muted" /></span>
                                <span style="color:var(--accent);font-weight:600;font-size:0.95rem;">
                                    "Starts in "{cd}
                                </span>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>

        // About this Event
        {move || {
            if has_description {
                let desc = description.clone();
                view! {
                    <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                        <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.75rem;">
                            "About this Event"
                        </h2>
                        <p style="color:var(--text-secondary);font-size:0.9rem;line-height:1.6;white-space:pre-line;margin:0;">
                            {desc}
                        </p>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }
        }}

        // Deposit Info Section (read-only, shows price + refund policy)
        {move || {
            if has_deposit {
                let usdc = usdc_display.clone();
                let thb = thb_display.clone();
                let refund = refund_label.clone();
                view! {
                    <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                        <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.75rem;">
                            <Icon icon=IconName::Coin class="icon-md" />" Deposit Commitment"
                        </h2>
                        <div style="margin-bottom:0.75rem;">
                            <span style="color:var(--text-primary);font-size:1.1rem;font-weight:600;">
                                {usdc}
                            </span>
                            {move || {
                                if let Some(ref thb_str) = thb {
                                    view! {
                                        <span style="color:var(--text-secondary);font-size:0.9rem;margin-left:0.5rem;">
                                            "(~" {thb_str.clone()} ")"
                                        </span>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }
                            }}
                        </div>
                        <div style="display:flex;flex-direction:column;gap:0.4rem;">
                            <div style="display:flex;align-items:center;gap:0.4rem;">
                                <span style="color:#34d399;">"✓"</span>
                                <span style="color:var(--text-secondary);font-size:0.85rem;">
                                    "Fully refundable when you show up"
                                </span>
                            </div>
                            <div style="display:flex;align-items:center;gap:0.4rem;">
                                <span style="color:#34d399;">"✓"</span>
                                <span style="color:var(--text-secondary);font-size:0.85rem;">
                                    "Refund deadline: "{refund}" after event"
                                </span>
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }
        }}

        // Registration Form Section
        {move || {
            if !show_reg_form {
                return ().into_any();
            }

            let current_reg = reg_state.get();
            match &current_reg {
                RegState::Success(data) => {
                    // Auto-redirect: save progress to localStorage then navigate.
                    // Shows a brief confirmation message while redirecting.
                    let next_url = data.next_step.url.clone();
                    let attendee_id = data.attendee_id.clone();
                    let _event_id = data.next_step.url.clone(); // parse event_id from URL
                    let eid = next_url
                        .split("event_id=")
                        .nth(1)
                        .map(|s| s.split('&').next().unwrap_or(s).to_string())
                        .unwrap_or_default();
                    let slug_for_ls = slug_for_reg.clone();

                    // Persist to localStorage for resume capability
                    let _ = js_sys::eval(&format!(
                        "localStorage.setItem('bethere_progress', JSON.stringify({{attendee_id:'{}',event_id:'{}',slug:'{}'}}'))",
                        attendee_id, eid, slug_for_ls
                    ));

                    // Auto-navigate after a brief moment so the user sees the confirmation
                    let redirect_url = next_url.clone();
                    leptos::task::spawn_local(async move {
                        gloo::timers::future::TimeoutFuture::new(800).await;
                        let _ = js_sys::eval(&format!(
                            "window.location.href = '{}'",
                            redirect_url
                        ));
                    });

                    view! {
                        <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                            <div style="text-align:center;">
                                <div style="font-size:2rem;margin-bottom:0.5rem;"><Icon icon=IconName::Check class="icon-2xl icon-success" /></div>
                                <h2 style="font-size:1.1rem;font-weight:600;color:#34d399;margin-bottom:0.5rem;">
                                    "You're registered!"
                                </h2>
                                <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                    {format!("Welcome, {}!", data.name)}
                                </p>
                                <p style="color:var(--text-secondary);font-size:0.8rem;">
                                    "Redirecting..."
                                </p>
                            </div>
                        </div>
                    }.into_any()
                }
                RegState::Error(msg) => {
                    let msg_clone = msg.clone();
                    view! {
                        <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                            <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.75rem;">
                                <Icon icon=IconName::Ticket class="icon-md" />" Reserve Your Spot"
                            </h2>
                            <div style="padding:0.75rem;background:rgba(239,68,68,0.1);border-radius:var(--radius);margin-bottom:1rem;color:#f87171;font-size:0.85rem;">
                                {msg_clone}
                            </div>
                            <button
                                class="btn btn-outline btn-block"
                                on:click=move |_| set_reg_state.set(RegState::Idle)
                            >
                                "Try Again"
                            </button>
                        </div>
                    }.into_any()
                }
                RegState::Submitting => {
                    view! {
                        <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);text-align:center;">
                            <div style="margin-bottom:0.5rem;"><Icon icon=IconName::Hourglass class="icon-md" /></div>
                            <p style="color:var(--text-secondary);font-size:0.9rem;">"Registering..."</p>
                        </div>
                    }.into_any()
                }
                RegState::Idle => {
                    let slug = slug_for_reg.clone();
                    let is_hybrid_clone = is_hybrid;
                    view! {
                        <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                            <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.75rem;">
                                <Icon icon=IconName::Ticket class="icon-md" />" Reserve Your Spot"
                            </h2>
                            <div style="display:flex;flex-direction:column;gap:0.75rem;">
                                // Name
                                <input
                                    type="text"
                                    placeholder="Your name"
                                    prop:value=move || reg_name.get()
                                    on:input=move |ev| set_reg_name.set(event_target_value(&ev))
                                    style="width:100%;padding:0.6rem 0.8rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);color:var(--text-primary);font-size:0.9rem;outline:none;"
                                />
                                // Email
                                <input
                                    type="email"
                                    placeholder="Email address"
                                    prop:value=move || reg_email.get()
                                    on:input=move |ev| set_reg_email.set(event_target_value(&ev))
                                    style="width:100%;padding:0.6rem 0.8rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);color:var(--text-primary);font-size:0.9rem;outline:none;"
                                />
                                // Participation type (hybrid only)
                                {move || {
                                    if is_hybrid_clone {
                                        view! {
                                            <select
                                                style="width:100%;padding:0.6rem 0.8rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);color:var(--text-primary);font-size:0.9rem;outline:none;"
                                                on:change=move |ev| set_reg_participation.set(event_target_value(&ev))
                                            >
                                                <option value="">"Select track..."</option>
                                                <option value="In-Person">"In-Person (on-site)"</option>
                                                <option value="Online">"Online (virtual)"</option>
                                            </select>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                                // Preferred Contact Channel
                                <div style="display:flex;flex-direction:column;gap:0.25rem;">
                                    <label style="font-size:0.8rem;color:var(--text-secondary);">
                                        "Preferred Contact Channel / ช่องทางที่สะดวกให้ทีมงานติดต่อกลับเพื่อยืนยันสิทธิ์ (Confirm Seat)"
                                        {move || if require_contact {
                                            view! { <span style="color:#f87171;">" *"</span> }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </label>
                                    <select
                                        style="width:100%;padding:0.6rem 0.8rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);color:var(--text-primary);font-size:0.9rem;outline:none;"
                                        on:change=move |ev| set_reg_contact_channel.set(event_target_value(&ev))
                                    >
                                        <option value="">"Select channel..."</option>
                                        <option value="Telegram">"Telegram"</option>
                                        <option value="Line">"Line"</option>
                                        <option value="Facebook">"Facebook"</option>
                                        <option value="X (Twitter)">"X (Twitter)"</option>
                                    </select>
                                </div>
                                // Contact Handle
                                <input
                                    type="text"
                                    placeholder="Username or profile link / โปรดระบุ Username หรือลิงก์โปรไฟล์"
                                    prop:value=move || reg_contact_handle.get()
                                    on:input=move |ev| set_reg_contact_handle.set(event_target_value(&ev))
                                    style="width:100%;padding:0.6rem 0.8rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);color:var(--text-primary);font-size:0.9rem;outline:none;"
                                />
                                // Deposit Agreement (only when deposit enabled)
                                {move || {
                                    if has_deposit {
                                        let dep_thb = deposit_thb;
                                        view! {
                                            <label style="display:flex;align-items:flex-start;gap:0.5rem;font-size:0.85rem;color:var(--text-secondary);cursor:pointer;">
                                                <input
                                                    type="checkbox"
                                                    style="margin-top:0.2rem;accent-color:var(--accent);"
                                                    checked=move || reg_deposit_agreed.get()
                                                    on:change=move |ev| set_reg_deposit_agreed.set(event_target_checked(&ev))
                                                />
                                                <span>{format!("ยอมรับการจ่ายมัดจำ {} บาท (จะได้รับคืนภายในงาน) / I agree to pay a {} THB commitment deposit to secure my seat and understand I will receive a refund upon check-in at the venue.", dep_thb, dep_thb)}</span>
                                            </label>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                                // Submit button
                                {
                                    let slug = slug.clone();
                                    view! {
                                        <button
                                            style="width:100%;padding:0.75rem;background:var(--accent);color:#fff;border:none;border-radius:var(--radius);font-weight:600;font-size:0.95rem;cursor:pointer;transition:opacity 0.2s;"
                                            on:click=move |_| {
                                                let name_val = reg_name.get();
                                                let email_val = reg_email.get();
                                                let part_val = reg_participation.get();
                                                let channel_val = reg_contact_channel.get();
                                                let handle_val = reg_contact_handle.get();
                                                let deposit_val = reg_deposit_agreed.get();

                                                // Client-side validation
                                                if name_val.trim().is_empty() {
                                                    set_reg_state.set(RegState::Error("Please enter your name".to_string()));
                                                    return;
                                                }
                                                if email_val.trim().is_empty() || !email_val.contains('@') {
                                                    set_reg_state.set(RegState::Error("Please enter a valid email".to_string()));
                                                    return;
                                                }
                                                if require_contact && channel_val.trim().is_empty() {
                                                    set_reg_state.set(RegState::Error("Please select a preferred contact channel".to_string()));
                                                    return;
                                                }
                                                if require_contact && handle_val.trim().is_empty() {
                                                    set_reg_state.set(RegState::Error("Please provide your contact username or profile link".to_string()));
                                                    return;
                                                }
                                                if has_deposit && !deposit_val {
                                                    set_reg_state.set(RegState::Error("You must agree to the deposit commitment to register".to_string()));
                                                    return;
                                                }

                                                set_reg_state.set(RegState::Submitting);
                                                let body = RegisterBody {
                                                    slug: slug.clone(),
                                                    name: name_val.trim().to_string(),
                                                    email: email_val.trim().to_lowercase(),
                                                    participation_type: if part_val.is_empty() { None } else { Some(part_val.clone()) },
                                                    contact_channel: if channel_val.trim().is_empty() { None } else { Some(channel_val.trim().to_string()) },
                                                    contact_handle: if handle_val.trim().is_empty() { None } else { Some(handle_val.trim().to_string()) },
                                                    deposit_agreed: if deposit_val { Some(true) } else { None },
                                                };

                                                leptos::task::spawn_local(async move {
                                                    let window = web_sys::window().expect("no window");
                                                    let origin = window.location().origin().unwrap_or_else(|_| "http://localhost:8787".to_string());
                                                    let url = format!("{origin}/api/public/register");

                                                    match gloo::net::http::Request::post(&url)
                                                        .json(&body)
                                                    {
                                                        Ok(req) => {
                                                            match req.send().await {
                                                                Ok(resp) => {
                                                                    match resp.text().await {
                                                                        Ok(text) => {
                                                                            match serde_json::from_str::<RegisterResponse>(&text) {
                                                                                Ok(api_resp) => {
                                                                                    if api_resp.success {
                                                                                        if let Some(data) = api_resp.data {
                                                                                            set_reg_state.set(RegState::Success(data));
                                                                                        } else {
                                                                                            set_reg_state.set(RegState::Error("No data returned".to_string()));
                                                                                        }
                                                                                    } else {
                                                                                        set_reg_state.set(RegState::Error(
                                                                                            api_resp.error.unwrap_or_else(|| "Registration failed".to_string())
                                                                                        ));
                                                                                    }
                                                                                }
                                                                                Err(e) => set_reg_state.set(RegState::Error(format!("Parse error: {e}"))),
                                                                            }
                                                                        }
                                                                        Err(e) => set_reg_state.set(RegState::Error(format!("Read error: {e}"))),
                                                                    }
                                                                }
                                                                Err(e) => set_reg_state.set(RegState::Error(format!("Network error: {e}"))),
                                                            }
                                                        }
                                                        Err(e) => set_reg_state.set(RegState::Error(format!("Request error: {e}"))),
                                                    }
                                                });
                                            }
                                        >
                                            "Reserve My Spot"
                                        </button>
                                    }
                                }
                            </div>
                        </div>
                    }.into_any()
                }
            }
        }}

        // NFT Badge Section
        {move || {
            if has_nft_image {
                let url = nft_image_url_2.clone();
                view! {
                    <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                        <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            <Icon icon=IconName::Ticket class="icon-md" />" NFT Badge"
                        </h2>
                        <p style="color:var(--text-secondary);font-size:0.85rem;margin-bottom:0.75rem;">
                            "Earn a commemorative NFT badge when you attend."
                        </p>
                        <img
                            src=url
                            alt="NFT Badge"
                            style="max-width:120px;border-radius:var(--radius);"
                        />
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }
        }}

        // External Link
        {move || {
            if has_link {
                let href = link_2.clone();
                view! {
                    <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                        <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.75rem;">
                            <Icon icon=IconName::Link class="icon-sm" />" External Link"
                        </h2>
                        <a
                            href=href
                            target="_blank"
                            rel="noopener noreferrer"
                            style="color:var(--accent);text-decoration:none;font-size:0.9rem;font-weight:500;"
                        >
                            "View Event Page →"
                        </a>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }
        }}
    }.into_any()
}
