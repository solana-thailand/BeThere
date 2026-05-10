//! Public event page — displays event details to attendees (no auth required).
//!
//! Accessed via `/e/:slug`. Shows event info, countdown timer, deposit details,
//! NFT badge preview, and external link.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

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
    pub nft_image_url: String,
    pub nft_name_template: String,
    pub nft_symbol: String,
    pub nft_description_template: String,
    pub quiz_enabled: bool,
    pub refund_deadline_hours: u64,
    pub description: String,
    pub location: String,
    pub created_at: String,
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
                                    <div style="font-size:2rem;margin-bottom:1rem;">"🎫"</div>
                                    <p style="color:var(--text-secondary);">"Loading event..."</p>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::NotFound => {
                            view! {
                                <div style="text-align:center;padding:3rem 0;">
                                    <div style="font-size:2rem;margin-bottom:1rem;">"🔍"</div>
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
                                    <div style="font-size:2rem;margin-bottom:1rem;">"⚠️"</div>
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
    let date_str = format_event_date(data.event_start_ms);
    let time_str = format!(
        "{} — {}",
        format_event_time(data.event_start_ms),
        format_event_time(data.event_end_ms)
    );
    let nft_image_url = data.nft_image_url.clone();
    let nft_image_url_2 = data.nft_image_url.clone();
    let link = data.link.clone();
    let link_2 = data.link.clone();
    let name = data.name.clone();
    let tagline = data.tagline.clone();
    let description = data.description.clone();
    let location = data.location.clone();
    let usdc_display = format_usdc(data.deposit_amount_usdc);
    let thb_display = if data.deposit_amount_thb > 0 {
        Some(format_thb(data.deposit_amount_thb))
    } else {
        None
    };
    let refund_label = format_refund_deadline(data.refund_deadline_hours);
    let show_deposit_cta = has_deposit && !event_completed.get();

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
                        <span style="font-size:3rem;">"🎫"</span>
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
                            <span style="font-size:1.1rem;">"📍"</span>
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
                <span style="font-size:1.1rem;">"📅"</span>
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
                            <span style="font-size:1.1rem;">"🎉"</span>
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
                                <span style="font-size:1.1rem;">"⏱️"</span>
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

        // Deposit Section
        {move || {
            if has_deposit {
                let usdc = usdc_display.clone();
                let thb = thb_display.clone();
                let refund = refund_label.clone();
                let cta_link = link.clone();
                view! {
                    <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                        <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.75rem;">
                            "💰 Secure Your Spot"
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
                        <div style="display:flex;flex-direction:column;gap:0.4rem;margin-bottom:1rem;">
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
                        {move || {
                            if show_deposit_cta {
                                if !cta_link.is_empty() {
                                    let href = cta_link.clone();
                                    view! {
                                        <a
                                            href=href
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            style="display:inline-block;width:100%;text-align:center;padding:0.75rem 1.5rem;background:var(--accent);color:#fff;border-radius:var(--radius);font-weight:600;font-size:0.95rem;text-decoration:none;transition:opacity 0.2s;"
                                        >
                                            "Register Now →"
                                        </a>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div style="text-align:center;padding:0.75rem;background:var(--bg-secondary);border-radius:var(--radius);color:var(--text-secondary);font-size:0.9rem;">
                                            "Contact the organizer to register"
                                        </div>
                                    }.into_any()
                                }
                            } else {
                                ().into_any()
                            }
                        }}
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }
        }}

        // NFT Badge Section
        {move || {
            if has_nft_image {
                let url = nft_image_url_2.clone();
                view! {
                    <div style="width:100%;background:var(--bg-card);border-radius:var(--radius);padding:1.25rem;margin-bottom:1rem;box-shadow:var(--shadow);">
                        <h2 style="font-size:1.1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "🎫 NFT Badge"
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
                            "🔗 External Link"
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
