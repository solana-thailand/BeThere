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
                        let claimed = data.claimed;
                        let claimed_asset_id = data.claimed_asset_id.clone();
                        let cluster = data.cluster.clone();
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

                        // Deposit-related fields
                        let deposit_enabled = data.deposit_enabled;
                        let deposit_deadline_hours = data.deposit_deadline_hours;
                        let deposit_amount_thb = data.deposit_amount_thb;
                        let deadline_expired = data.deadline_expired;
                        let in_person_available = data.in_person_available;
                        let _event_slug = data.event_slug.clone();

                        // Online attendee detection
                        let is_online = !data.is_in_person;
                        let event_end_ms = data.event_end_ms;
                        let event_name = data.event_name.clone();
                        let video_url = data.video_url.clone();
                        let has_video = !video_url.is_empty();

                        if is_online {
                            // ── Online attendee view ──

                            // Live countdown: reactive signal updated every 60s
                            let (countdown_text, set_countdown_text) = signal(String::new());
                            let (event_ended, set_event_ended) = signal(event_end_ms > 0 && js_sys::Date::now() as i64 >= event_end_ms);

                            // Compute remaining time label from a timestamp
                            let fmt_remaining = move |now_ms: i64| -> String {
                                if event_end_ms <= 0 || now_ms >= event_end_ms {
                                    return String::new();
                                }
                                let diff_ms = event_end_ms - now_ms;
                                let days = diff_ms / (1000 * 60 * 60 * 24);
                                let hours = (diff_ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60);
                                if days > 0 {
                                    format!("{days}d {hours}h remaining")
                                } else {
                                    let mins = (diff_ms % (1000 * 60 * 60)) / (1000 * 60);
                                    format!("{hours}h {mins}m remaining")
                                }
                            };

                            // Initial value
                            set_countdown_text.set(fmt_remaining(js_sys::Date::now() as i64));

                            // Start a 60s interval to refresh countdown
                            Effect::new(move |_| {
                                let cb = Closure::<dyn Fn()>::new(move || {
                                    let now_ms = js_sys::Date::now() as i64;
                                    let ended = event_end_ms > 0 && now_ms >= event_end_ms;
                                    set_event_ended.set(ended);
                                    set_countdown_text.set(fmt_remaining(now_ms));
                                });
                                let interval_id = web_sys::window()
                                    .unwrap()
                                    .set_interval_with_callback_and_timeout_and_arguments_0(
                                        cb.as_ref().unchecked_ref(),
                                        60_000i32,
                                    )
                                    .unwrap();
                                // Keep closure alive for the timer lifetime
                                cb.forget();
                                on_cleanup(move || {
                                    let _ = web_sys::window().map(|w| {
                                        w.clear_interval_with_handle(interval_id);
                                    });
                                });
                            });

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

                                    // Deposit status slot — exactly one notice for online attendees
                                    {if deposit_enabled && deposit_info.is_none() {
                                        if deadline_expired && in_person_available.unwrap_or(false) {
                                            // Reclaim available — show reclaim banner
                                            let attendee_id_for_reclaim = api_id.clone();
                                            let eid_param = web_sys::Url::new(
                                                &web_sys::window().unwrap().location().href().unwrap(),
                                            ).ok().and_then(|url| url.search_params().get("event_id"));
                                            let reclaim_href = match eid_param {
                                                Some(ref eid) if !eid.is_empty() => format!("/deposit/{}?event_id={}", attendee_id_for_reclaim, eid),
                                                _ => format!("/deposit/{}", attendee_id_for_reclaim),
                                            };
                                            view! {
                                                <div style="background:rgba(250,204,21,0.08);border:1px solid rgba(250,204,21,0.3);border-radius:var(--radius);padding:1rem;margin-bottom:0.75rem;text-align:center;">
                                                    <div style="display:flex;align-items:center;justify-content:center;gap:0.5rem;margin-bottom:0.5rem;">
                                                        <span style="color:#facc15;"><Icon icon=IconName::Warning class="icon-sm" /></span>
                                                        <span style="color:#fbbf24;font-weight:700;font-size:0.9rem;">
                                                            "Want to Attend In-Person?"
                                                        </span>
                                                    </div>
                                                    <p style="color:#eab308;font-size:0.8rem;margin:0.25rem 0 0.75rem;">
                                                        "Your deposit deadline passed and you were moved to the online track. "
                                                        "But in-person spots are still available! Complete your deposit to reclaim your spot."
                                                    </p>
                                                    <a
                                                        href=reclaim_href
                                                        class="btn btn-success btn-block"
                                                        style="max-width:300px;margin:0 auto;"
                                                    >
                                                        <Icon icon=IconName::CreditCard class="icon-sm" />
                                                        " Deposit Now to Reclaim Spot"
                                                    </a>
                                                </div>
                                            }.into_any()
                                        } else if deadline_expired {
                                            // Moved to online — no reclaim possible
                                            view! {
                                                <div style="background:rgba(99,102,241,0.08);border:1px solid rgba(99,102,241,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:0.75rem;text-align:center;">
                                                    <span style="color:#818cf8;font-size:0.8rem;">
                                                        "You were moved to the online track because your deposit wasn't completed in time. "
                                                        "You can still claim your NFT after the event."
                                                    </span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            // Not expired, online attendee — no notice needed
                                            view! { <div></div> }.into_any()
                                        }
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}

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
                                            // Step 2: Wait for event to end (live countdown)
                                            <div style="display:flex;gap:0.75rem;align-items:flex-start;">
                                                <div style=format!("flex-shrink:0;width:28px;height:28px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.75rem;color:#fff;font-weight:700;", if event_ended.get() { "#22c55e" } else { "var(--text-secondary)" })>
                                                    {move || if event_ended.get() { "\u{2713}" } else { "2" }}
                                                </div>
                                                <div>
                                                    <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">
                                                        {move || if event_ended.get() { "Event Ended" } else { "Wait for Event" }}
                                                    </div>
                                                    <div style="font-size:0.75rem;color:var(--text-secondary);">
                                                        {move || {
                                                            if event_ended.get() {
                                                                "The event has ended — you can proceed to claim.".to_string()
                                                            } else {
                                                                let ct = countdown_text.get();
                                                                if !ct.is_empty() { ct } else { "Claims open after the event ends.".to_string() }
                                                            }
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
                                                    {if !is_checked_in && has_claim {
                                                        let quest_href = claim_href.clone();
                                                        view! {
                                                            <a
                                                                href=quest_href
                                                                style="font-size:0.75rem;color:var(--accent,#6366f1);text-decoration:none;font-weight:500;"
                                                            >
                                                                "→ Go to Quest"
                                                            </a>
                                                        }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }}
                                                </div>
                                            </div>
                                            // Step 4: Claim NFT
                                            <div style="display:flex;gap:0.75rem;align-items:flex-start;">
                                                <div style=format!("flex-shrink:0;width:28px;height:28px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.75rem;color:#fff;font-weight:700;", if claimed { "#22c55e" } else { "var(--text-secondary)" })>
                                                    {if claimed { "\u{2713}" } else { "4" }}
                                                </div>
                                                <div>
                                                    <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">
                                                        {if claimed { "Badge Claimed!" } else { "Claim Your Badge" }}
                                                    </div>
                                                    <div style="font-size:0.75rem;color:var(--text-secondary);">
                                                        {if claimed {
                                                            "Your compressed NFT attendance proof has been minted."
                                                        } else {
                                                            "Mint your compressed NFT attendance proof."
                                                        }}
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    // Action button
                                    <div style="margin-top:1.25rem;text-align:center;">
                                        {move || {
                                            if claimed {
                                                let solanafm_link = claimed_asset_id.as_ref().and_then(|id| {
                                                    let c = cluster.as_deref().unwrap_or("devnet");
                                                    if id.is_empty() { None } else { Some(utils::solanafm_asset_url(id, c)) }
                                                });
                                                view! {
                                                    <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.75rem;text-align:center;">
                                                        <div style="font-size:0.85rem;color:#34d399;font-weight:600;">
                                                            "\u{2713} NFT Badge claimed successfully!"
                                                        </div>
                                                        {if let Some(url) = solanafm_link {
                                                            view! {
                                                                <a
                                                                    href=url
                                                                    target="_blank"
                                                                    rel="noopener noreferrer"
                                                                    style="display:inline-block;margin-top:0.5rem;font-size:0.8rem;color:#818cf8;text-decoration:underline;"
                                                                >
                                                                    "View NFT on SolanaFM \u{2197}"
                                                                </a>
                                                            }.into_any()
                                                        } else {
                                                            view! { <div></div> }.into_any()
                                                        }}
                                                    </div>
                                                }.into_any()
                                            } else {
                                                let ended = event_ended.get();
                                                let available = has_claim && ended;
                                                if available {
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
                                                } else if has_claim && !ended {
                                                    view! {
                                                        <div style="background:rgba(250,204,21,0.08);border:1px solid rgba(250,204,21,0.2);border-radius:var(--radius);padding:0.75rem;text-align:center;">
                                                            <div style="font-size:0.8rem;color:#facc15;font-weight:500;">
                                                                "Claim link will be available after the event ends."
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! { <div></div> }.into_any()
                                                }
                                            }
                                        }}
                                    </div>
                                </div>

                                // Video / Livestream section (online attendees)
                                {if has_video {
                                    let url = video_url.clone();
                                    let is_yt = url.contains("youtube.com") || url.contains("youtu.be");
                                    let embed_url = if is_yt {
                                        let vid = if url.contains("youtu.be/") {
                                            url.split("youtu.be/").nth(1).map(|s| s.split('?').next().unwrap_or("")).unwrap_or("").to_string()
                                        } else if url.contains("v=") {
                                            url.split("v=").nth(1).map(|s| s.split('&').next().unwrap_or("")).unwrap_or("").to_string()
                                        } else if url.contains("/live/") {
                                            url.split("/live/").nth(1).map(|s| s.split('?').next().unwrap_or("")).unwrap_or("").to_string()
                                        } else {
                                            String::new()
                                        };
                                        if vid.is_empty() { String::new() } else { format!("https://www.youtube.com/embed/{vid}") }
                                    } else { String::new() };

                                    if !embed_url.is_empty() {
                                            let link = url.clone();
                                            view! {
                                                <div style="width:100%;margin-top:0.75rem;">
                                                    <h3 style="font-size:0.9rem;font-weight:600;color:#fff;margin-bottom:0.5rem;display:flex;align-items:center;gap:0.4rem;">
                                                        "\u{1F4FA} Livestream / Recording"
                                                    </h3>
                                                    <div style="position:relative;width:100%;padding-bottom:56.25%;border-radius:8px;overflow:hidden;">
                                                        <iframe
                                                            src=embed_url
                                                            style="position:absolute;top:0;left:0;width:100%;height:100%;border:none;"
                                                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                                                            allowfullscreen=true
                                                            title="Event video"
                                                        />
                                                    </div>
                                                    <a
                                                        href=link
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        style="display:inline-block;margin-top:0.5rem;background:var(--accent);color:#000;padding:0.5rem 1rem;border-radius:8px;text-decoration:none;font-weight:600;font-size:0.85rem;"
                                                    >
                                                        "Watch on YouTube \u{2192}"
                                                    </a>
                                                </div>
                                            }.into_any()
                                    } else {
                                        let href = url;
                                        view! {
                                            <div style="width:100%;margin-top:0.75rem;">
                                                <h3 style="font-size:0.9rem;font-weight:600;color:#fff;margin-bottom:0.5rem;display:flex;align-items:center;gap:0.4rem;">
                                                    "\u{1F4FA} Livestream / Recording"
                                                </h3>
                                                <a
                                                    href=href
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    style="display:inline-block;background:var(--accent);color:#000;padding:0.5rem 1rem;border-radius:8px;text-decoration:none;font-weight:600;font-size:0.85rem;"
                                                >
                                                    "Watch Video \u{2192}"
                                                </a>
                                            </div>
                                        }.into_any()
                                    }
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

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

                                // Checked-in claim banner — prominent CTA at the top
                                {if is_checked_in && !claimed && has_claim {
                                    let claim_cta_href = claim_href.clone();
                                    view! {
                                        <a
                                            href=claim_cta_href
                                            class="btn btn-primary"
                                            style="width:100%;margin-bottom:1rem;font-size:1rem;padding:0.85rem;"
                                        >
                                            <Icon icon=IconName::Gift class="icon-sm" />
                                            " Claim Your NFT Badge →"
                                        </a>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

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
                                    // Deposit status — single notice slot (exactly one shown)
                                    {if let Some(ref dep) = deposit_info {
                                        if dep.verified {
                                            view! {
                                                <div style="background:#d1fae5;border-radius:var(--radius);padding:0.5rem 0.75rem;margin-bottom:0.75rem;text-align:center;">
                                                    <span style="color:#065f46;font-weight:600;font-size:0.85rem;">
                                                        "Deposit: Verified ✓"
                                                    </span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div class="ticket-deposit-pending" style="background:#fef3c7;border-radius:var(--radius);padding:0.75rem;margin-bottom:0.75rem;text-align:center;">
                                                    <div style="display:flex;align-items:center;justify-content:center;gap:0.5rem;">
                                                        <span style="color:#d97706;"><Icon icon=IconName::Hourglass class="icon-sm" /></span>
                                                        <span style="color:#92400e;font-weight:600;">
                                                            {match dep.method {
                                                                DepositMethod::Thb => "Payment Slip: Pending Verification",
                                                                DepositMethod::Usdc => "Deposit: Pending Confirmation",
                                                                DepositMethod::CreditThb | DepositMethod::CreditUsdc => "Credit Deposit: Pending",
                                                            }}
                                                        </span>
                                                    </div>
                                                    <p style="color:#92400e;font-size:0.8rem;margin:0.5rem 0 0;">
                                                        {match dep.method {
                                                            DepositMethod::Thb => "Your payment slip has been submitted. We'll verify it shortly — check back in a few minutes.",
                                                            DepositMethod::Usdc => "Your deposit is being confirmed on-chain.",
                                                            DepositMethod::CreditThb | DepositMethod::CreditUsdc => "Your credit deposit is being processed.",
                                                        }}
                                                    </p>
                                                </div>
                                            }.into_any()
                                        }
                                    } else if deposit_enabled && deadline_expired && in_person_available.unwrap_or(false) {
                                        let attendee_id_for_reclaim = api_id.clone();
                                        let eid_param = web_sys::Url::new(
                                            &web_sys::window().unwrap().location().href().unwrap(),
                                        ).ok().and_then(|url| url.search_params().get("event_id"));
                                        let reclaim_href = match eid_param {
                                            Some(ref eid) if !eid.is_empty() => format!("/deposit/{}?event_id={}", attendee_id_for_reclaim, eid),
                                            _ => format!("/deposit/{}", attendee_id_for_reclaim),
                                        };
                                        view! {
                                            <div style="background:rgba(239,68,68,0.08);border:1px solid rgba(239,68,68,0.3);border-radius:var(--radius);padding:1rem;margin-bottom:0.75rem;text-align:center;">
                                                <div style="display:flex;align-items:center;justify-content:center;gap:0.5rem;margin-bottom:0.5rem;">
                                                    <span style="color:#ef4444;"><Icon icon=IconName::Warning class="icon-sm" /></span>
                                                    <span style="color:#f87171;font-weight:700;font-size:0.9rem;">
                                                        "Deadline Passed — Reclaim Your Spot"
                                                    </span>
                                                </div>
                                                <p style="color:#fca5a5;font-size:0.8rem;margin:0.25rem 0 0.75rem;">
                                                    "Your deposit deadline has passed and you've been moved to the online track. "
                                                    "However, in-person spots are still available!"
                                                </p>
                                                <a
                                                    href=reclaim_href
                                                    class="btn btn-success btn-block"
                                                    style="max-width:300px;margin:0 auto;"
                                                >
                                                    <Icon icon=IconName::CreditCard class="icon-sm" />
                                                    " Deposit Now to Reclaim"
                                                </a>
                                            </div>
                                        }.into_any()
                                    } else if deposit_enabled && deadline_expired && !in_person_available.unwrap_or(true) {
                                        view! {
                                            <div style="background:rgba(239,68,68,0.08);border:1px solid rgba(239,68,68,0.3);border-radius:var(--radius);padding:1rem;margin-bottom:0.75rem;text-align:center;">
                                                <div style="display:flex;align-items:center;justify-content:center;gap:0.5rem;margin-bottom:0.5rem;">
                                                    <span style="color:#ef4444;"><Icon icon=IconName::Warning class="icon-sm" /></span>
                                                    <span style="color:#f87171;font-weight:700;font-size:0.9rem;">
                                                        "Moved to Online Track"
                                                    </span>
                                                </div>
                                                <p style="color:#fca5a5;font-size:0.8rem;margin:0.25rem 0 0;">
                                                    "Your deposit deadline has passed. In-person spots are now full, so you've been automatically moved to the online track. "
                                                    "You can still claim your NFT after the event."
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else if deposit_enabled && !is_checked_in {
                                        let attendee_id_for_deposit = api_id.clone();
                                        let eid_param = web_sys::Url::new(
                                            &web_sys::window().unwrap().location().href().unwrap(),
                                        ).ok().and_then(|url| url.search_params().get("event_id"));
                                        let deposit_href = match eid_param {
                                            Some(ref eid) if !eid.is_empty() => format!("/deposit/{}?event_id={}", attendee_id_for_deposit, eid),
                                            _ => format!("/deposit/{}", attendee_id_for_deposit),
                                        };
                                        view! {
                                            <div style="background:rgba(250,204,21,0.08);border:1px solid rgba(250,204,21,0.3);border-radius:var(--radius);padding:1rem;margin-bottom:0.75rem;text-align:center;">
                                                <div style="display:flex;align-items:center;justify-content:center;gap:0.5rem;margin-bottom:0.5rem;">
                                                    <span style="color:#facc15;"><Icon icon=IconName::CreditCard class="icon-sm" /></span>
                                                    <span style="color:#fbbf24;font-weight:700;font-size:0.9rem;">
                                                        {if deposit_amount_thb > 0 {
                                                            format!("Deposit Required: {} THB", deposit_amount_thb)
                                                        } else {
                                                            "Deposit Required".to_string()
                                                        }}
                                                    </span>
                                                </div>
                                                <p style="color:#eab308;font-size:0.8rem;margin:0.25rem 0 0.75rem;">
                                                    {if let Some(hours) = deposit_deadline_hours {
                                                        format!("Complete your deposit within {} hours of registration to keep your in-person spot.", hours)
                                                    } else {
                                                        "Complete your deposit to secure your in-person spot.".to_string()
                                                    }}
                                                </p>
                                                <a
                                                    href=deposit_href
                                                    class="btn btn-primary btn-block"
                                                    style="max-width:300px;margin:0 auto;"
                                                >
                                                    <Icon icon=IconName::CreditCard class="icon-sm" />
                                                    " Pay Deposit Now"
                                                </a>
                                            </div>
                                        }.into_any()
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
                                            {if claimed {
                                                let solanafm_link = claimed_asset_id.as_ref().and_then(|id| {
                                                    let c = cluster.as_deref().unwrap_or("devnet");
                                                    if id.is_empty() { None } else { Some(utils::solanafm_asset_url(id, c)) }
                                                });
                                                view! {
                                                    <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.6rem 0.75rem;text-align:center;margin-top:0.5rem;">
                                                        <div style="font-size:0.85rem;color:#34d399;font-weight:600;">
                                                            "\u{2713} NFT Badge claimed!"
                                                        </div>
                                                        {if let Some(url) = solanafm_link {
                                                            view! {
                                                                <a
                                                                    href=url
                                                                    target="_blank"
                                                                    rel="noopener noreferrer"
                                                                    style="display:inline-block;margin-top:0.35rem;font-size:0.8rem;color:#818cf8;text-decoration:underline;"
                                                                >
                                                                    "View NFT on SolanaFM \u{2197}"
                                                                </a>
                                                            }.into_any()
                                                        } else {
                                                            view! { <div></div> }.into_any()
                                                        }}
                                                    </div>
                                                }.into_any()
                                            } else if has_claim {
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

                            // Video / Livestream section (in-person attendees)
                            {if has_video {
                                let url = video_url.clone();
                                let is_yt = url.contains("youtube.com") || url.contains("youtu.be");
                                let embed_url = if is_yt {
                                    let vid = if url.contains("youtu.be/") {
                                        url.split("youtu.be/").nth(1).map(|s| s.split('?').next().unwrap_or("")).unwrap_or("").to_string()
                                    } else if url.contains("v=") {
                                        url.split("v=").nth(1).map(|s| s.split('&').next().unwrap_or("")).unwrap_or("").to_string()
                                    } else if url.contains("/live/") {
                                        url.split("/live/").nth(1).map(|s| s.split('?').next().unwrap_or("")).unwrap_or("").to_string()
                                    } else {
                                        String::new()
                                    };
                                    if vid.is_empty() { String::new() } else { format!("https://www.youtube.com/embed/{vid}") }
                                } else { String::new() };

                                if !embed_url.is_empty() {
                                    let link = url.clone();
                                    view! {
                                        <div class="card" style="margin-top:0.75rem;padding:1rem;">
                                            <h3 style="font-size:0.9rem;font-weight:600;color:#fff;margin-bottom:0.5rem;display:flex;align-items:center;gap:0.4rem;">
                                                "\u{1F4FA} Livestream / Recording"
                                            </h3>
                                            <div style="position:relative;width:100%;padding-bottom:56.25%;border-radius:8px;overflow:hidden;">
                                                <iframe
                                                    src=embed_url
                                                    style="position:absolute;top:0;left:0;width:100%;height:100%;border:none;"
                                                    allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                                                    allowfullscreen=true
                                                    title="Event video"
                                                />
                                            </div>
                                            <a
                                                href=link
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                style="display:inline-block;margin-top:0.5rem;background:var(--accent);color:#000;padding:0.5rem 1rem;border-radius:8px;text-decoration:none;font-weight:600;font-size:0.85rem;"
                                            >
                                                "Watch on YouTube \u{2192}"
                                            </a>
                                        </div>
                                    }.into_any()
                                } else {
                                    let href = url;
                                    view! {
                                        <div class="card" style="margin-top:0.75rem;padding:1rem;">
                                            <h3 style="font-size:0.9rem;font-weight:600;color:#fff;margin-bottom:0.5rem;display:flex;align-items:center;gap:0.4rem;">
                                                "\u{1F4FA} Livestream / Recording"
                                            </h3>
                                            <a
                                                href=href
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                style="display:inline-block;background:var(--accent);color:#000;padding:0.5rem 1rem;border-radius:8px;text-decoration:none;font-weight:600;font-size:0.85rem;"
                                            >
                                                "Watch Video \u{2192}"
                                            </a>
                                        </div>
                                    }.into_any()
                                }
                            } else {
                                view! { <div></div> }.into_any()
                            }}

                            // Navigation links
                            <div class="ticket-nav-links" style="display:flex;gap:0.75rem;justify-content:center;margin-top:1rem;">
                                <a href="/" class="btn btn-outline btn-sm">
                                    "\u{2190} Home"
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
