//! Public ticket page — attendees view their check-in QR slip.
//!
//! Accessible at `/ticket/:attendee_id?event_id=xxx`.
//! No auth required — uses the public `/api/public/ticket/{id}` endpoint.
//! Email is masked server-side for privacy.
//!
//! Smart auto-refresh: polls every 10s when awaiting deposit verification or
//! check-in. Stops polling once QR appears or attendee is checked in.
//! 5-minute max polling window, then shows manual refresh button.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use wasm_bindgen::prelude::*;

use crate::api::{self, cache_invalidate, DepositMethod};
use crate::icons::{Icon, IconName};
use crate::utils;

/// Polling interval for auto-refresh.
const POLL_INTERVAL_MS: u32 = 10_000;
/// Maximum duration for auto-refresh polling before stopping.
const POLL_MAX_MS: u32 = 300_000; // 5 minutes

#[wasm_bindgen(module = "/js/download.js")]
extern "C" {
    #[wasm_bindgen(js_name = "downloadDataUrl")]
    fn download_data_url(data_url: &str, filename: &str);
}

// ---------------------------------------------------------------------------
// URL params
// ---------------------------------------------------------------------------

#[derive(Params, PartialEq, Clone)]
struct TicketParams {
    attendee_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Page state
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum TicketState {
    Loading,
    Found(api::AttendeeData),
    NotFound(String),
    Error(String),
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Public ticket page where attendees view their check-in QR code.
#[component]
pub fn Ticket() -> impl IntoView {
    let params = use_params::<TicketParams>();

    let (state, set_state) = signal(TicketState::Loading);
    let (fullscreen_qr, set_fullscreen_qr) = signal(false);
    // Auto-refresh polling state
    let (polling_active, set_polling_active) = signal(false);
    let (polling_expired, set_polling_expired) = signal(false);

    // Helper: determine if current state needs polling
    let needs_polling = |data: &api::AttendeeData| -> bool {
        // Online attendees never get QR/deposit — polling is pointless
        if !data.is_in_person {
            return false;
        }
        if data.is_checked_in {
            return false;
        }
        // No QR yet — likely waiting for deposit verification or QR generation
        if data.qr_image.is_none() {
            return true;
        }
        // Has deposit but not verified — still waiting
        if data.deposit_info.as_ref().is_some_and(|d| !d.verified) {
            return true;
        }
        false
    };

    // Extract attendee_id from URL and event_id from query, then fetch ticket data
    Effect::new(move |_| {
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => {
                set_state.set(TicketState::Error(
                    "Invalid ticket link — missing attendee ID.".to_string(),
                ));
                return;
            }
        };

        if attendee_id.is_empty() {
            set_state.set(TicketState::Error(
                "Invalid ticket link — missing attendee ID.".to_string(),
            ));
            return;
        }

        // Parse event_id from query params: ?event_id=xxx
        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"));

        leptos::task::spawn_local(async move {
            match api::get_public_ticket(&attendee_id, event_id.as_deref()).await {
                Ok(data) => {
                    log::info!(
                        "[ticket] loaded ticket for {}",
                        data.attendee.name
                    );
                    let should_poll = needs_polling(&data);
                    set_state.set(TicketState::Found(data));
                    set_polling_active.set(should_poll);
                }
                Err(e) => {
                    log::error!("[ticket] failed to load: {e}");
                    let msg = e.message.to_lowercase();
                    if msg.contains("not found") {
                        set_state.set(TicketState::NotFound(
                            "Attendee not found. Check your ticket link and try again."
                                .to_string(),
                        ));
                    } else {
                        set_state.set(TicketState::Error(format!(
                            "Failed to load ticket: {e}"
                        )));
                    }
                }
            }
        });
    });

    // Smart auto-refresh polling — only when waiting for deposit verification or QR
    // Stops when: QR appears, checked in, or 5 min elapsed
    // Uses raw web_sys::Window timers (gloo Interval is !Send, incompatible with on_cleanup)
    Effect::new(move |_| {
        // Only run when polling is active
        if !polling_active.get() {
            return;
        }

        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => return,
        };
        if attendee_id.is_empty() {
            return;
        }

        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"));

        // Build the cache key for this ticket
        let cache_key = match &event_id {
            Some(e) if !e.is_empty() => format!("/public/ticket/{attendee_id}?event_id={e}"),
            _ => format!("/public/ticket/{attendee_id}"),
        };

        // Poll callback — invalidate cache and refetch
        let interval_cb = {
            let cache_key = cache_key.clone();
            let attendee_id = attendee_id.clone();
            let event_id = event_id.clone();
            Closure::<dyn Fn()>::new(move || {
                let aid = attendee_id.clone();
                let eid = event_id.clone();
                cache_invalidate(&cache_key);

                leptos::task::spawn_local(async move {
                    match api::get_public_ticket(&aid, eid.as_deref()).await {
                        Ok(data) => {
                            let still_needs = needs_polling(&data);
                            set_state.set(TicketState::Found(data));
                            if !still_needs {
                                log::info!("[ticket] polling stopped — state resolved");
                                set_polling_active.set(false);
                            }
                        }
                        Err(e) => {
                            log::warn!("[ticket] poll refresh failed: {e}");
                        }
                    }
                });
            })
        };

        // Start interval
        let interval_id = web_sys::window()
            .unwrap()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                interval_cb.as_ref().unchecked_ref(),
                POLL_INTERVAL_MS as i32,
            )
            .unwrap();

        // Max timeout — stop after 5 minutes
        let timeout_cb = Closure::<dyn Fn()>::new(move || {
            log::info!("[ticket] polling expired after {}s", POLL_MAX_MS / 1000);
            set_polling_active.set(false);
            set_polling_expired.set(true);
        });
        let timeout_id = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_cb.as_ref().unchecked_ref(),
                POLL_MAX_MS as i32,
            )
            .unwrap();

        // Must keep closures alive for the lifetime of the timers
        interval_cb.forget();
        timeout_cb.forget();

        // Cleanup when this effect re-runs or component unmounts
        on_cleanup(move || {
            let _ = web_sys::window().map(|w| {
                w.clear_interval_with_handle(interval_id);
                w.clear_timeout_with_handle(timeout_id);
            });
        });
    });

    // Manual refresh handler (for when polling expired or user wants to refresh)
    let on_manual_refresh = move || {
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => return,
        };
        if attendee_id.is_empty() {
            return;
        }
        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"));

        set_state.set(TicketState::Loading);

        leptos::task::spawn_local(async move {
            // Bust cache
            let cache_key = match &event_id {
                Some(e) if !e.is_empty() => format!("/public/ticket/{attendee_id}?event_id={e}"),
                _ => format!("/public/ticket/{attendee_id}"),
            };
            cache_invalidate(&cache_key);

            match api::get_public_ticket(&attendee_id, event_id.as_deref()).await {
                Ok(data) => {
                    let still_needs = needs_polling(&data);
                    set_state.set(TicketState::Found(data));
                    if still_needs {
                        // Restart polling on manual refresh
                        set_polling_expired.set(false);
                        set_polling_active.set(true);
                    }
                }
                Err(e) => {
                    set_state.set(TicketState::Error(format!("Failed to refresh: {e}")));
                }
            }
        });
    };

    view! {
        <Title text="Your Ticket — BeThere" />
        <div class="center-page">
            <div class="container layout-col-center">
                {move || match state.get() {
                    TicketState::Loading => view! {
                        <div class="page-loading">
                            <span class="spinner spinner-lg"></span>
                            " Loading your ticket..."
                        </div>
                    }.into_any(),
                    TicketState::Found(data) => {
                        let qr_image = data.qr_image.clone();
                        let has_qr = data.qr_image.is_some();
                        let name = data.attendee.name.clone();
                        let ticket_name = data.attendee.ticket_name.clone();
                        let participation = data.participation_type.clone();
                        let is_checked_in = data.is_checked_in;
                        let is_approved = data.is_approved;
                        let checked_in_at = data.attendee.checked_in_at.clone();
                        let checked_in_by = data.attendee.checked_in_by.clone();
                        let masked_email = data.attendee.email.clone();
                        let api_id = data.attendee.api_id.clone();
                        let claim_token = data.attendee.claim_token.clone();
                        let deposit_info = data.deposit_info.clone();

                        // Status text (pre-computed to avoid FnOnce issues)
                        let status_detail = if is_checked_in {
                            let ts = checked_in_at.as_deref()
                                .map(|t| utils::format_timestamp(t))
                                .unwrap_or_default();
                            let by = checked_in_by.as_ref()
                                .map(|by| if by.is_empty() { String::new() } else { format!(" by {}", utils::escape_html(by)) })
                                .unwrap_or_default();
                            format!("{ts}{by}")
                        } else {
                            String::new()
                        };

                        // Claim href (pre-computed)
                        let claim_href = claim_token.map(|t| format!("/claim/{t}")).unwrap_or_default();
                        let has_claim = !claim_href.is_empty();

                        // Online attendee detection
                        let is_online = !data.is_in_person;
                        let event_end_ms = data.event_end_ms;
                        let event_name = data.event_name.clone();

                        if is_online {
                            // ── Online attendee view ──
                            let now_ms = js_sys::Date::now() as i64;
                            let event_ended = event_end_ms > 0 && now_ms >= event_end_ms;
                            let claim_available = has_claim && event_ended;

                            // Time remaining until event ends
                            let remaining_text = if event_end_ms > 0 && !event_ended {
                                let diff_ms = event_end_ms - now_ms;
                                let days = diff_ms / (1000 * 60 * 60 * 24);
                                let hours = (diff_ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60);
                                if days > 0 {
                                    format!("{days}d {hours}h remaining")
                                } else {
                                    let mins = (diff_ms % (1000 * 60 * 60)) / (1000 * 60);
                                    format!("{hours}h {mins}m remaining")
                                }
                            } else {
                                String::new()
                            };

                            view! {
                                // Main ticket card
                                <div class="card ticket-card">
                                    // Header
                                    <div class="ticket-header">
                                        <Icon icon=IconName::Globe class="icon-lg" />
                                        <h1 class="ticket-title">"Online Registration"</h1>
                                    </div>

                                    // Online badge
                                    <div style="text-align:center;margin-bottom:1rem;">
                                        <div style="display:inline-flex;align-items:center;gap:0.4rem;background:rgba(99,102,241,0.12);border:1px solid rgba(99,102,241,0.3);border-radius:9999px;padding:0.35rem 0.85rem;font-size:0.8rem;font-weight:600;color:#818cf8;">
                                            <Icon icon=IconName::Globe class="icon-sm" />
                                            "Online Track"
                                        </div>
                                    </div>

                                    // Attendee info
                                    <div class="ticket-info">
                                        <div class="ticket-info-row">
                                            <span class="ticket-info-label">"Name"</span>
                                            <span class="ticket-info-value">
                                                {utils::escape_html(&name)}
                                            </span>
                                        </div>
                                        {if !masked_email.is_empty() {
                                            let email = masked_email;
                                            view! {
                                                <div class="ticket-info-row">
                                                    <span class="ticket-info-label">"Email"</span>
                                                    <span class="ticket-info-value ticket-email-masked">
                                                        {utils::escape_html(&email)}
                                                    </span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}
                                        {if !event_name.is_empty() {
                                            let en = event_name;
                                            view! {
                                                <div class="ticket-info-row">
                                                    <span class="ticket-info-label">"Event"</span>
                                                    <span class="ticket-info-value">
                                                        {utils::escape_html(&en)}
                                                    </span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}
                                    </div>

                                    // Next steps timeline
                                    <div style="margin-top:1.25rem;border-top:1px solid var(--border);padding-top:1.25rem;">
                                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:1rem;">
                                            "What's Next?"
                                        </h3>
                                        <div style="display:flex;flex-direction:column;gap:0.75rem;">
                                            // Step 1: Register — done
                                            <div style="display:flex;gap:0.75rem;align-items:flex-start;">
                                                <div style="flex-shrink:0;width:28px;height:28px;border-radius:50%;background:#22c55e;display:flex;align-items:center;justify-content:center;font-size:0.75rem;color:#fff;font-weight:700;">
                                                    "\u{2713}" // ✓
                                                </div>
                                                <div>
                                                    <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">"Register"</div>
                                                    <div style="font-size:0.75rem;color:var(--text-secondary);">"You're all signed up!"</div>
                                                </div>
                                            </div>
                                            // Step 2: Wait for event to end
                                            <div style="display:flex;gap:0.75rem;align-items:flex-start;">
                                                <div style=format!("flex-shrink:0;width:28px;height:28px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.75rem;color:#fff;font-weight:700;", if event_ended { "#22c55e" } else { "var(--text-secondary)" })>
                                                    {if event_ended { "\u{2713}" } else { "2" }}
                                                </div>
                                                <div>
                                                    <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">
                                                        {if event_ended { "Event Ended" } else { "Wait for Event" }}
                                                    </div>
                                                    <div style="font-size:0.75rem;color:var(--text-secondary);">
                                                        {if event_ended {
                                                            "The event has ended — you can proceed to claim.".to_string()
                                                        } else if !remaining_text.is_empty() {
                                                            remaining_text
                                                        } else {
                                                            "Claims open after the event ends.".to_string()
                                                        }}
                                                    </div>
                                                </div>
                                            </div>
                                            // Step 3: Complete quest
                                            <div style="display:flex;gap:0.75rem;align-items:flex-start;">
                                                <div style=format!("flex-shrink:0;width:28px;height:28px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.75rem;color:#fff;font-weight:700;", if is_checked_in { "#22c55e" } else { "var(--text-secondary)" })>
                                                    {if is_checked_in { "\u{2713}" } else { "3" }}
                                                </div>
                                                <div>
                                                    <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">
                                                        {if is_checked_in { "Quest Completed" } else { "Complete Quest" }}
                                                    </div>
                                                    <div style="font-size:0.75rem;color:var(--text-secondary);">
                                                        {if is_checked_in {
                                                            "Virtual check-in complete!"
                                                        } else {
                                                            "Pass the quiz or adventure to virtually check in."
                                                        }}
                                                    </div>
                                                </div>
                                            </div>
                                            // Step 4: Claim NFT
                                            <div style="display:flex;gap:0.75rem;align-items:flex-start;">
                                                <div style="flex-shrink:0;width:28px;height:28px;border-radius:50%;background:var(--text-secondary);display:flex;align-items:center;justify-content:center;font-size:0.75rem;color:#fff;font-weight:700;">
                                                    "4"
                                                </div>
                                                <div>
                                                    <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">"Claim Your Badge"</div>
                                                    <div style="font-size:0.75rem;color:var(--text-secondary);">
                                                        "Mint your compressed NFT attendance proof."
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    // Action button
                                    <div style="margin-top:1.25rem;text-align:center;">
                                        {if claim_available {
                                            let href = claim_href.clone();
                                            view! {
                                                <a
                                                    href=href
                                                    class="btn btn-primary"
                                                    style="width:100%;"
                                                >
                                                    <Icon icon=IconName::Gift class="icon-sm" />
                                                    " Claim Your NFT Badge"
                                                </a>
                                            }.into_any()
                                        } else if has_claim && !event_ended {
                                            view! {
                                                <div style="background:rgba(250,204,21,0.08);border:1px solid rgba(250,204,21,0.2);border-radius:var(--radius);padding:0.75rem;text-align:center;">
                                                    <div style="font-size:0.8rem;color:#facc15;font-weight:500;">
                                                        "Claim link will be available after the event ends."
                                                    </div>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}
                                    </div>
                                </div>

                                // Navigation links
                                <div class="ticket-nav-links" style="display:flex;gap:0.75rem;justify-content:center;margin-top:1rem;">
                                    <a href="/" class="btn btn-outline btn-sm">
                                        "\u{2190} Home"
                                    </a>
                                </div>
                            }.into_any()
                        } else {
                        // ── In-Person attendee view (original) ──
                        view! {
                            // Main ticket card
                            <div class="card ticket-card">
                                // Header with logo
                                <div class="ticket-header">
                                    <Icon icon=IconName::Ticket class="icon-lg" />
                                    <h1 class="ticket-title">"Your Ticket"</h1>
                                </div>

                                // QR Code section
                                <div class="ticket-qr-section">
                                    {if has_qr {
                                        view! {
                                            <div class="ticket-qr-wrapper">
                                                <img
                                                    src=qr_image.clone().unwrap_or_default()
                                                    alt="Check-in QR Code"
                                                    class="ticket-qr-img"
                                                />
                                            </div>
                                            <button
                                                class="btn btn-outline btn-sm ticket-fullscreen-btn"
                                                on:click=move |_| set_fullscreen_qr.set(true)
                                            >
                                                <Icon icon=IconName::Expand class="icon-sm" />
                                                " Full Screen"
                                            </button>
                                            <button
                                                class="btn btn-outline btn-sm"
                                                on:click={
                                                    let name = name.clone();
                                                    move |_| {
                                                        let qr = qr_image.clone();
                                                        if let Some(ref data_url) = qr {
                                                            if !data_url.is_empty() {
                                                                download_data_url(data_url, &format!("{name}-qrcode.svg"));
                                                            }
                                                        }
                                                    }
                                                }
                                            >
                                                <Icon icon=IconName::Save class="icon-sm" />
                                                " Save QR Code"
                                            </button>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="ticket-qr-placeholder">
                                                <Icon icon=IconName::QrCode class="icon-xl" />
                                                <p class="hint">"QR code not yet generated"</p>
                                            </div>
                                        }.into_any()
                                    }}
                                </div>

                                // Attendee info
                                <div class="ticket-info">
                                    <div class="ticket-info-row">
                                        <span class="ticket-info-label">"Name"</span>
                                        <span class="ticket-info-value">
                                            {utils::escape_html(&name)}
                                        </span>
                                    </div>
                                    {if !masked_email.is_empty() {
                                        let email = masked_email;
                                        view! {
                                            <div class="ticket-info-row">
                                                <span class="ticket-info-label">"Email"</span>
                                                <span class="ticket-info-value ticket-email-masked">
                                                    {utils::escape_html(&email)}
                                                </span>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                    {if !ticket_name.is_empty() {
                                        let tn = ticket_name;
                                        view! {
                                            <div class="ticket-info-row">
                                                <span class="ticket-info-label">"Ticket"</span>
                                                <span class="ticket-info-value">
                                                    {utils::escape_html(&tn)}
                                                </span>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                    {if !participation.is_empty() {
                                        let pt = participation;
                                        view! {
                                            <div class="ticket-info-row">
                                                <span class="ticket-info-label">"Type"</span>
                                                <span class="ticket-info-value">
                                                    {utils::escape_html(&pt)}
                                                </span>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                    <div class="ticket-info-row">
                                        <span class="ticket-info-label">"ID"</span>
                                        <span class="ticket-info-value ticket-id">
                                            {utils::escape_html(&api_id)}
                                        </span>
                                    </div>
                                </div>

                                // Status section
                                <div class="ticket-status-section">
                                    // Deposit status section (only shown when deposit info is present)
                                    {if let Some(ref dep) = deposit_info {
                                        if !dep.verified {
                                            view! {
                                                <div class="ticket-deposit-pending" style="background:#fef3c7;border-radius:var(--radius);padding:0.75rem;margin-bottom:0.75rem;text-align:center;">
                                                    <div style="display:flex;align-items:center;justify-content:center;gap:0.5rem;">
                                                        <span style="color:#d97706;"><Icon icon=IconName::Hourglass class="icon-sm" /></span>
                                                        <span style="color:#92400e;font-weight:600;">
                                                            {match dep.method {
                                                                DepositMethod::Thb => "Payment Slip: Pending Verification",
                                                                DepositMethod::Usdc => "Deposit: Pending Confirmation",
                                                            }}
                                                        </span>
                                                    </div>
                                                    <p style="color:#92400e;font-size:0.8rem;margin:0.5rem 0 0;">
                                                        {match dep.method {
                                                            DepositMethod::Thb => "Your payment slip has been submitted. We'll notify you once it's verified.",
                                                            DepositMethod::Usdc => "Your deposit is being confirmed on-chain.",
                                                        }}
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div style="background:#d1fae5;border-radius:var(--radius);padding:0.5rem 0.75rem;margin-bottom:0.75rem;text-align:center;">
                                                    <span style="color:#065f46;font-weight:600;font-size:0.85rem;">
                                                        "Deposit: Verified ✓"
                                                    </span>
                                                </div>
                                            }.into_any()
                                        }
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                    {if is_checked_in {
                                        let detail = status_detail;
                                        let href = claim_href;
                                        view! {
                                            <div class="ticket-status-badge ticket-status-checked-in">
                                                <Icon icon=IconName::Check class="icon-sm" />
                                                " Checked In"
                                            </div>
                                            {if !detail.is_empty() {
                                                view! {
                                                    <p class="ticket-status-detail">
                                                        {detail}
                                                    </p>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                            {if has_claim {
                                                view! {
                                                    <a
                                                        href=href
                                                        class="btn btn-primary ticket-claim-btn"
                                                    >
                                                        <Icon icon=IconName::Gift class="icon-sm" />
                                                        " Claim Your NFT"
                                                    </a>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                        }.into_any()
                                    } else if !is_approved {
                                        view! {
                                            <div class="ticket-status-badge ticket-status-pending">
                                                <Icon icon=IconName::Clock class="icon-sm" />
                                                " Pending Approval"
                                            </div>
                                            <p class="ticket-status-detail">
                                                "Your registration is being reviewed."
                                            </p>
                                        }.into_any()
                                    } else if deposit_info.as_ref().is_some_and(|d| !d.verified) {
                                        // Approved but deposit not yet verified — don't show QR yet
                                        view! {
                                            <div class="ticket-status-badge ticket-status-pending">
                                                <Icon icon=IconName::Hourglass class="icon-sm" />
                                                " Awaiting Deposit Verification"
                                            </div>
                                            <p class="ticket-status-detail">
                                                "Your deposit is being verified. QR code will appear once confirmed."
                                            </p>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="ticket-status-badge ticket-status-ready">
                                                <Icon icon=IconName::QrCode class="icon-sm" />
                                                " Ready for Check-In"
                                            </div>
                                            <p class="ticket-status-detail">
                                                "Show this QR code to staff at the event."
                                            </p>
                                        }.into_any()
                                    }}
                                </div>
                            </div>

                            // Navigation links
                            <div class="ticket-nav-links" style="display:flex;gap:0.75rem;justify-content:center;margin-top:1rem;">
                                <a href="/" class="btn btn-outline btn-sm">
                                    "← Home"
                                </a>
                            </div>

                            // Footer hint
                            {if is_checked_in {
                                view! {
                                    <p class="ticket-footer-hint">
                                        "You're checked in! Enjoy the event."
                                    </p>
                                }.into_any()
                            } else if !is_approved {
                                view! {
                                    <p class="ticket-footer-hint">
                                        "Your registration is being reviewed. You'll receive a QR code once approved."
                                    </p>
                                }.into_any()
                            } else {
                                view! {
                                    <p class="ticket-footer-hint">
                                        "Present this ticket at the registration desk for check-in."
                                    </p>
                                }.into_any()
                            }}

                            // Auto-refresh indicator (outside reactive block to access signals)
                        }.into_any()
                        } // end else (in-person branch)
                    },
                    TicketState::NotFound(msg) => view! {
                        <div class="center-page">
                            <div class="container layout-col-center">
                                <Icon icon=IconName::Search class="icon-xl" />
                                <h1>"Ticket Not Found"</h1>
                                <p class="subtitle">{msg}</p>
                                <a href="/" class="btn btn-primary">"Go Home"</a>
                            </div>
                        </div>
                    }.into_any(),
                    TicketState::Error(msg) => view! {
                        <div class="center-page">
                            <div class="container layout-col-center">
                                <Icon icon=IconName::AlertTriangle class="icon-xl" />
                                <h1>"Something Went Wrong"</h1>
                                <p class="subtitle">{utils::escape_html(&msg)}</p>
                                <a href="/" class="btn btn-primary">"Go Home"</a>
                            </div>
                        </div>
                    }.into_any(),
                }}

                // Auto-refresh polling indicator
                <Show
                    when=move || polling_active.get()
                    fallback=|| view! { <div></div> }
                >
                    <div class="ticket-poll-indicator" style="display:flex;align-items:center;justify-content:center;gap:0.5rem;margin-top:0.75rem;color:var(--text-secondary);font-size:0.8rem;">
                        <span class="spinner spinner-sm"></span>
                        "Checking for updates..."
                    </div>
                </Show>

                // Manual refresh button (shown when polling expired or user wants to refresh)
                <Show
                    when=move || {
                        let expired = polling_expired.get();
                        let found = matches!(state.get(), TicketState::Found(_));
                        expired && found
                    }
                    fallback=|| view! { <div></div> }
                >
                    <button
                        class="btn btn-outline btn-sm"
                        style="margin-top:0.75rem;"
                        on:click=move |_| on_manual_refresh()
                    >
                        <Icon icon=IconName::Refresh class="icon-sm" />
                        " Refresh Status"
                    </button>
                </Show>
            </div>
        </div>

        // Fullscreen QR overlay — rendered outside main layout, controlled by signal
        <Show
            when=move || fullscreen_qr.get()
            fallback=|| view! { <div></div> }
        >
            <div
                class="ticket-fullscreen-overlay"
                on:click=move |_| set_fullscreen_qr.set(false)
            >
                <div
                    class="ticket-fullscreen-card"
                    on:click=move |ev| ev.stop_propagation()
                >
                    <div class="ticket-fullscreen-header">
                        <span class="ticket-fullscreen-name">
                            {move || match state.get() {
                                TicketState::Found(d) => utils::escape_html(&d.attendee.name),
                                _ => String::new(),
                            }}
                        </span>
                        <button
                            class="ticket-fullscreen-close"
                            on:click=move |_| set_fullscreen_qr.set(false)
                        >
                            "✕"
                        </button>
                    </div>
                    <img
                        src=move || match state.get() {
                            TicketState::Found(d) => d.qr_image.unwrap_or_default(),
                            _ => String::new(),
                        }
                        alt="QR Code"
                        class="ticket-fullscreen-qr"
                    />
                    <p class="ticket-fullscreen-hint">"Show this code to staff"</p>
                </div>
            </div>
        </Show>
    }
}
