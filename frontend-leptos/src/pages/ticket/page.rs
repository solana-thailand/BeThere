//! Public ticket page — attendees view their check-in QR slip.
//!
//! Accessible at `/ticket/:attendee_id?event_id=xxx`.
//! No auth required — uses the public `/api/public/ticket/{id}` endpoint.
//!
//! Smart auto-refresh: polls every 10s when awaiting deposit verification or
//! check-in. Stops polling once QR appears or attendee is checked in.
//! 5-minute max polling window, then shows manual refresh button.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use wasm_bindgen::prelude::*;

use crate::api::{self, cache_invalidate};
use crate::icons::{Icon, IconName};
use crate::utils;

use super::action_cards::*;
use super::event_context::EventContext;
use super::hero::TicketHero;
use super::nft_badge::NftClaimedBadge;
use super::timeline::{Timeline, TimelineStep};
use super::video_section::VideoSection;

/// Polling interval for auto-refresh.
const POLL_INTERVAL_MS: u32 = 10_000;
/// Maximum duration for auto-refresh polling before stopping.
const POLL_MAX_MS: u32 = 300_000; // 5 minutes

#[wasm_bindgen(module = "/js/download.js")]
extern "C" {
    #[wasm_bindgen(js_name = "downloadDataUrl")]
    fn download_data_url(data_url: &str, filename: &str);
}

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
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
// Helpers
// ---------------------------------------------------------------------------

/// Build a deposit/reclaim href with optional event_id query param.
fn build_deposit_href(api_id: &str) -> String {
    let eid = web_sys::Url::new(
        &web_sys::window()
            .unwrap()
            .location()
            .href()
            .unwrap(),
    )
    .ok()
    .and_then(|url| url.search_params().get("event_id"));

    match eid {
        Some(ref e) if !e.is_empty() => format!("/deposit/{api_id}?event_id={e}"),
        _ => format!("/deposit/{api_id}"),
    }
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
    // Collapsible QR toggle — collapsed by default after check-in
    let (show_qr, set_show_qr) = signal(false);
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

        <div class="ticket-page">
            <div class="ticket-page-inner">

                {move || match state.get() {
                    TicketState::Loading => view! {
                        <div class="page-loading">
                            <span class="spinner spinner-lg"></span>
                            " Loading your ticket..."
                        </div>
                    }.into_any(),

                    TicketState::Found(data) => {
                        // ── Clone all fields from data ──
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

                        // Pre-computed status detail text
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

                        // Claim href
                        let claim_href = claim_token.map(|t| format!("/claim/{t}")).unwrap_or_default();
                        let has_claim = !claim_href.is_empty();

                        // Deposit-related fields
                        let deposit_enabled = data.deposit_enabled;
                        let deposit_deadline_hours = data.deposit_deadline_hours;
                        let deposit_amount_thb = data.deposit_amount_thb;
                        let deadline_expired = data.deadline_expired;
                        let in_person_available = data.in_person_available;
                        let refund_link = data.refund_link.clone();
                        let escrow_status = data.escrow_status.clone();
                        let escrow_closed = escrow_status == "closed"
                            || escrow_status == "cancelled"
                            || escrow_status == "deactivated";

                        // Event fields
                        let is_online = !data.is_in_person;
                        let event_end_ms = data.event_end_ms;
                        let video_url = data.video_url.clone();
                        let has_video = !video_url.is_empty();
                        let event_link = data.event_link.clone();
                        let event_location = data.event_location.clone();
                        let event_tagline = data.event_tagline.clone();
                        let nft_image_url = data.nft_image_url.clone();

                        // Build deposit/reclaim href
                        let deposit_href = build_deposit_href(&api_id);

                        // Orb link for NFT badge
                        let orb_link = claimed_asset_id.as_ref().and_then(|id| {
                            let c = cluster.as_deref().unwrap_or("devnet");
                            if id.is_empty() { None } else { Some(utils::orb_nft_url(id, c)) }
                        });

                        if is_online {
                            // ────────────────────────────────────────────────────
                            // ONLINE ATTENDEE VIEW
                            // ────────────────────────────────────────────────────

                            // Live countdown: reactive signal updated every 60s
                            let (countdown_text, set_countdown_text) = signal(String::new());
                            let (event_ended, set_event_ended) = signal(
                                event_end_ms > 0 && js_sys::Date::now() as i64 >= event_end_ms
                            );

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
                                cb.forget();
                                on_cleanup(move || {
                                    let _ = web_sys::window().map(|w| {
                                        w.clear_interval_with_handle(interval_id);
                                    });
                                });
                            });

                            // ── Build timeline steps ──
                            let quest_link = if !is_checked_in && has_claim {
                                Some((claim_href.clone(), "→ Go to Quest".to_string()))
                            } else {
                                None
                            };

                            view! {
                                // 1. Hero banner
                                <TicketHero
                                    variant="ticket-hero--online"
                                    icon=IconName::Globe
                                    title="Online Registration"
                                    badge="Online Track".to_string()
                                />

                                // 2. Main card
                                <div class="ticket-main-card">
                                    // Event context
                                    <EventContext
                                        nft_image_url=nft_image_url.clone()
                                        tagline=event_tagline.clone()
                                        location=event_location.clone()
                                        event_link=event_link.clone()
                                    />

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
                                                    <span class="ticket-info-value">
                                                        {utils::escape_html(&email)}
                                                    </span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}
                                    </div>

                                    // Deposit notice slot — exactly one for online
                                    {if deposit_enabled && deposit_info.is_none() {
                                        if deadline_expired && in_person_available.unwrap_or(false) {
                                            view! {
                                                <ReclaimActionCard reclaim_href=deposit_href.clone() />
                                            }.into_any()
                                        } else if deadline_expired {
                                            view! {
                                                <MovedOnlineCard />
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                </div>

                                // 3. Timeline — "What's Next?"
                                <Timeline steps=vec![
                                    TimelineStep {
                                        done: true,
                                        number: 1,
                                        title: "Register".into(),
                                        desc: "You're all signed up!".into(),
                                        link: None,
                                    },
                                    TimelineStep {
                                        done: event_ended.get(),
                                        number: 2,
                                        title: if event_ended.get() { "Event Ended" } else { "Wait for Event" }.into(),
                                        desc: if event_ended.get() {
                                            "The event has ended — you can proceed to claim.".to_string()
                                        } else {
                                            let ct = countdown_text.get();
                                            if !ct.is_empty() { ct } else { "Claims open after the event ends.".to_string() }
                                        },
                                        link: None,
                                    },
                                    TimelineStep {
                                        done: is_checked_in,
                                        number: 3,
                                        title: if is_checked_in { "Quest Completed" } else { "Complete Quest" }.into(),
                                        desc: if is_checked_in {
                                            "Virtual check-in complete!".into()
                                        } else {
                                            "Pass the quiz or adventure to virtually check in.".into()
                                        },
                                        link: quest_link,
                                    },
                                    TimelineStep {
                                        done: claimed,
                                        number: 4,
                                        title: if claimed { "Badge Claimed!" } else { "Claim Your Badge" }.into(),
                                        desc: if claimed {
                                            "Your compressed NFT attendance proof has been minted.".into()
                                        } else {
                                            "Mint your compressed NFT attendance proof.".into()
                                        },
                                        link: None,
                                    },
                                ] />

                                // 4. NFT section
                                {if claimed {
                                    let asset_id = claimed_asset_id.clone().unwrap_or_default();
                                    view! {
                                        <NftClaimedBadge
                                            asset_id=asset_id
                                            orb_link=orb_link.clone().unwrap_or_default()
                                            on_copy=Box::new(|text| copy_to_clipboard_js(text))
                                        />
                                    }.into_any()
                                } else {
                                    let ended = event_ended.get();
                                    let available = has_claim && ended;
                                    if available {
                                        view! {
                                            <ClaimActionCard claim_href=claim_href.clone() />
                                        }.into_any()
                                    } else if has_claim && !ended {
                                        view! {
                                            <div class="ticket-action-card ticket-action-card--pending">
                                                <div class="ticket-action-icon">
                                                    <Icon icon=IconName::Clock class="icon-sm" />
                                                </div>
                                                <div>
                                                    <div class="ticket-action-title">"Claim Available Soon"</div>
                                                    <div class="ticket-action-desc">
                                                        "Claim link will be available after the event ends."
                                                    </div>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }
                                }}

                                // 5. Video section
                                {if has_video {
                                    view! {
                                        <VideoSection video_url=video_url.clone() variant=String::new() />
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

                                // 6. Footer
                                <div class="ticket-footer">
                                    <div class="ticket-nav">
                                        <a href="/">"← Home"</a>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            // ────────────────────────────────────────────────────
                            // IN-PERSON ATTENDEE VIEW
                            // ────────────────────────────────────────────────────

                            // Determine hero variant
                            let (hero_variant, hero_icon, hero_title, hero_subtitle) = if is_checked_in {
                                (
                                    "ticket-hero--checked-in".to_string(),
                                    IconName::Check,
                                    "Checked In".to_string(),
                                    status_detail.clone(),
                                )
                            } else if !is_approved {
                                (
                                    "ticket-hero--pending".to_string(),
                                    IconName::Clock,
                                    "Pending Approval".to_string(),
                                    String::new(),
                                )
                            } else if deposit_info.as_ref().is_some_and(|d| !d.verified) {
                                (
                                    "ticket-hero--pending".to_string(),
                                    IconName::Hourglass,
                                    "Awaiting Deposit Verification".to_string(),
                                    String::new(),
                                )
                            } else {
                                (
                                    "ticket-hero--ready".to_string(),
                                    IconName::QrCode,
                                    "Ready for Check-In".to_string(),
                                    String::new(),
                                )
                            };

                            // NFT hero section (pre-computed to avoid FnOnce issues)
                            let nft_hero = if is_checked_in && claimed {
                                let asset_id = claimed_asset_id.clone().unwrap_or_default();
                                Some(("claimed", asset_id, orb_link.clone().unwrap_or_default()))
                            } else if is_checked_in && !claimed && has_claim {
                                Some(("cta", String::new(), String::new()))
                            } else {
                                None
                            };

                            view! {
                                // 1. Hero banner
                                <TicketHero
                                    variant=hero_variant
                                    icon=hero_icon
                                    title=hero_title
                                    subtitle=hero_subtitle
                                />

                                // 2. Main card
                                <div class="ticket-main-card">

                                    // ── NFT/Claim hero section ──
                                    {match &nft_hero {
                                        Some(("claimed", asset_id, ol)) => {
                                            let aid = asset_id.clone();
                                            let orb = ol.clone();
                                            view! {
                                                <NftClaimedBadge
                                                    asset_id=aid
                                                    orb_link=orb
                                                    on_copy=Box::new(|text| copy_to_clipboard_js(text))
                                                />
                                            }.into_any()
                                        }
                                        Some(("cta", _, _)) => {
                                            view! {
                                                <ClaimActionCard claim_href=claim_href.clone() />
                                            }.into_any()
                                        }
                                        _ => view! { <div></div> }.into_any(),
                                    }}

                                    // ── Event context ──
                                    <EventContext
                                        nft_image_url=nft_image_url.clone()
                                        tagline=event_tagline.clone()
                                        location=event_location.clone()
                                        event_link=event_link.clone()
                                    />

                                    // ── QR Code section ──
                                    {if is_checked_in {
                                        // Collapsible QR after check-in
                                        view! {
                                            <div class="ticket-qr-section">
                                                <button
                                                    class="btn btn-outline btn-sm"
                                                    on:click=move |_| set_show_qr.set(!show_qr.get())
                                                >
                                                    {move || if show_qr.get() {
                                                        "▲ Hide QR Code"
                                                    } else {
                                                        "▼ Show QR Code"
                                                    }}
                                                </button>
                                                <Show
                                                    when=move || show_qr.get()
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    {if has_qr {
                                                        view! {
                                                            <div class="ticket-qr-wrapper">
                                                                <img
                                                                    src=qr_image.clone().unwrap_or_default()
                                                                    alt="Check-in QR Code"
                                                                    class="ticket-qr-img"
                                                                />
                                                            </div>
                                                            <div class="ticket-qr-actions">
                                                                <button
                                                                    class="btn btn-outline btn-sm"
                                                                    on:click=move |_| set_fullscreen_qr.set(true)
                                                                >
                                                                    <Icon icon=IconName::Expand class="icon-sm" />
                                                                    " Full Screen"
                                                                </button>
                                                            </div>
                                                        }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }}
                                                </Show>
                                            </div>
                                        }.into_any()
                                    } else {
                                        // Pre-checkin: QR as hero
                                        view! {
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
                                                        <div class="ticket-qr-actions">
                                                            <button
                                                                class="btn btn-outline btn-sm"
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
                                                                                download_data_url(
                                                                                    data_url,
                                                                                    &format!("{name}-qrcode.svg"),
                                                                                );
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            >
                                                                <Icon icon=IconName::Save class="icon-sm" />
                                                                " Save QR Code"
                                                            </button>
                                                        </div>
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
                                        }.into_any()
                                    }}

                                    // ── Attendee info ──
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
                                                    <span class="ticket-info-value">
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
                                    </div>

                                    // ── Status section: deposit/refund/claim action cards ──
                                    // Exactly one deposit notice + optional refund + optional refund link

                                    // Deposit status
                                    {if let Some(ref dep) = deposit_info {
                                        view! {
                                            {if dep.verified {
                                                view! {
                                                    <DepositVerifiedCard />
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <DepositPendingCard method=dep.method.clone() />
                                                }.into_any()
                                            }}
                                            {if dep.refunded {
                                                view! {
                                                    <RefundCard refund_proof_url=dep.refund_proof_url.clone().unwrap_or_default() />
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                        }.into_any()
                                    } else if deposit_enabled && deadline_expired && in_person_available.unwrap_or(false) && !escrow_closed {
                                        view! {
                                            <ReclaimActionCard reclaim_href=deposit_href.clone() />
                                        }.into_any()
                                    } else if deposit_enabled && deadline_expired && !in_person_available.unwrap_or(true) && !escrow_closed {
                                        view! {
                                            <MovedOnlineCard />
                                        }.into_any()
                                    } else if deposit_enabled && !is_checked_in && !escrow_closed {
                                        view! {
                                            <DepositActionCard
                                                amount_thb=deposit_amount_thb
                                                deadline_hours=deposit_deadline_hours
                                                deposit_href=deposit_href.clone()
                                            />
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}

                                    // Organizer refund link (from Google Sheet, independent of deposit)
                                    {if let Some(ref link) = refund_link {
                                        if !link.is_empty() {
                                            view! {
                                                <div class="ticket-action-card ticket-action-card--info">
                                                    <div class="ticket-action-icon">
                                                        <Icon icon=IconName::Link class="icon-sm" />
                                                    </div>
                                                    <div>
                                                        <div class="ticket-action-title">"Organizer Refund Link"</div>
                                                        <a
                                                            href=link
                                                            target="_blank"
                                                            rel="noopener noreferrer"
                                                            class="ticket-action-link"
                                                        >
                                                            "View Refund Details →"
                                                        </a>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}

                                    // Status badge
                                    {if is_checked_in {
                                        let detail = status_detail.clone();
                                        view! {
                                            <div class="ticket-action-card ticket-action-card--verified">
                                                <div class="ticket-action-icon">
                                                    <Icon icon=IconName::Check class="icon-sm" />
                                                </div>
                                                <div>
                                                    <div class="ticket-action-title">"Checked In"</div>
                                                    {if !detail.is_empty() {
                                                        view! {
                                                            <div class="ticket-action-desc">{detail}</div>
                                                        }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }}
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else if !is_approved {
                                        view! {
                                            <div class="ticket-action-card ticket-action-card--pending">
                                                <div class="ticket-action-icon">
                                                    <Icon icon=IconName::Clock class="icon-sm" />
                                                </div>
                                                <div>
                                                    <div class="ticket-action-title">"Pending Approval"</div>
                                                    <div class="ticket-action-desc">
                                                        "Your registration is being reviewed."
                                                    </div>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else if deposit_info.as_ref().is_some_and(|d| !d.verified) {
                                        view! {
                                            <div class="ticket-action-card ticket-action-card--pending">
                                                <div class="ticket-action-icon">
                                                    <Icon icon=IconName::Hourglass class="icon-sm" />
                                                </div>
                                                <div>
                                                    <div class="ticket-action-title">"Awaiting Deposit Verification"</div>
                                                    <div class="ticket-action-desc">
                                                        "Your deposit is being verified. QR code will appear once confirmed."
                                                    </div>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="ticket-action-card ticket-action-card--ready">
                                                <div class="ticket-action-icon">
                                                    <Icon icon=IconName::QrCode class="icon-sm" />
                                                </div>
                                                <div>
                                                    <div class="ticket-action-title">"Ready for Check-In"</div>
                                                    <div class="ticket-action-desc">
                                                        "Show this QR code to staff at the event."
                                                    </div>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }}
                                </div>

                                // 5. Video section
                                {if has_video {
                                    view! {
                                        <VideoSection video_url=video_url.clone() variant="card".to_string() />
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

                                // 6. Footer
                                <div class="ticket-footer">
                                    <div class="ticket-nav">
                                        <a href="/">"← Home"</a>
                                    </div>
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
                                </div>
                            }.into_any()
                        }
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
                    <div class="ticket-poll">
                        "Checking for updates..."
                    </div>
                </Show>

                // Manual refresh button (shown when polling expired)
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
