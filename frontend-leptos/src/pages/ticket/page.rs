//! Public ticket page — attendees view their check-in QR slip.
//!
//! Accessible at `/ticket/:attendee_id?event_id=xxx`.
//! No auth required — uses the public `/api/public/ticket/{id}` endpoint.
//!
//! Two-tier auto-refresh polling:
//!   Tier 1 (10s): waiting for deposit verification or QR generation
//!   Tier 2 (30s): has QR, waiting for staff check-in at venue
//! Both tiers stop when attendee is checked in.
//! Tier 1 has a 5-minute cap; Tier 2 polls indefinitely until check-in.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use wasm_bindgen::prelude::*;

use crate::api::{self, cache_invalidate};
use crate::icons::{Icon, IconName};
use crate::utils;

use super::in_person_view::InPersonView;
use super::online_view::OnlineView;
use super::qr_section::FullscreenQrOverlay;
use super::view_data::TicketViewData;

/// Tier 1 interval: waiting for deposit verification / QR generation (10s).
const POLL_TIER1_INTERVAL_MS: u32 = 10_000;
/// Tier 2 interval: waiting for staff check-in at venue (30s).
const POLL_TIER2_INTERVAL_MS: u32 = 30_000;
/// Maximum duration for Tier 1 polling before stopping.
const POLL_TIER1_MAX_MS: u32 = 300_000; // 5 minutes

// ---------------------------------------------------------------------------
// Polling tier
// ---------------------------------------------------------------------------

/// Two-tier polling strategy for the ticket page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PollingTier {
    /// Tier 1: no QR yet, waiting for deposit verification or QR generation.
    /// Fast interval (10s), capped at 5 minutes.
    AwaitingDeposit,
    /// Tier 2: has QR, deposit verified, waiting for staff check-in.
    /// Slower interval (30s), no cap (attendee may wait hours at venue).
    AwaitingCheckIn,
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

    // Helper: determine which polling tier applies (or None to stop polling)
    let polling_tier = |data: &api::AttendeeData| -> Option<PollingTier> {
        // Online attendees never get QR/deposit — polling is pointless
        if !data.is_in_person {
            return None;
        }
        // Checked in — no further polling needed
        if data.is_checked_in {
            return None;
        }
        // No QR yet — waiting for deposit verification or QR generation
        if data.qr_image.is_none() {
            return Some(PollingTier::AwaitingDeposit);
        }
        // Has deposit but not verified — still fast polling
        if data.deposit_info.as_ref().is_some_and(|d| !d.verified) {
            return Some(PollingTier::AwaitingDeposit);
        }
        // Has QR and verified deposit — waiting for staff to scan at venue
        Some(PollingTier::AwaitingCheckIn)
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
                    let should_poll = polling_tier(&data).is_some();
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

    // Two-tier auto-refresh polling:
    //   Tier 1 (AwaitingDeposit): 10s interval, 5-min cap → then manual refresh
    //   Tier 2 (AwaitingCheckIn): 30s interval, no cap → polls until check-in
    // Uses raw web_sys::Window timers (gloo Interval is !Send, incompatible with on_cleanup)
    Effect::new(move |_| {
        // Only run when polling is active
        if !polling_active.get() {
            return;
        }

        // Determine current tier from latest state
        let current_tier = match &state.get() {
            TicketState::Found(data) => polling_tier(data),
            _ => None,
        };

        let tier = match current_tier {
            Some(t) => t,
            None => {
                // State resolved (e.g. checked in) — stop polling
                set_polling_active.set(false);
                return;
            }
        };

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

        let interval_ms = match tier {
            PollingTier::AwaitingDeposit => POLL_TIER1_INTERVAL_MS,
            PollingTier::AwaitingCheckIn => POLL_TIER2_INTERVAL_MS,
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
                            let new_tier = polling_tier(&data);
                            set_state.set(TicketState::Found(data));
                            match new_tier {
                                None => {
                                    log::info!("[ticket] polling stopped — state resolved");
                                    set_polling_active.set(false);
                                }
                                Some(PollingTier::AwaitingCheckIn) if tier == PollingTier::AwaitingDeposit => {
                                    // Tier upgrade: Tier 1 → Tier 2
                                    // Restart the effect to pick up new interval
                                    log::info!("[ticket] deposit verified — switching to check-in polling");
                                    set_polling_active.set(false);
                                    set_polling_active.set(true);
                                }
                                _ => {} // same tier, keep polling
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
                interval_ms as i32,
            )
            .unwrap();

        // Max timeout — only for Tier 1 (deposit verification)
        // Tier 2 (check-in) has no cap — attendee may wait hours at venue
        let timeout_id = match tier {
            PollingTier::AwaitingDeposit => {
                let timeout_cb = Closure::<dyn Fn()>::new(move || {
                    log::info!("[ticket] tier 1 polling expired after {}s", POLL_TIER1_MAX_MS / 1000);
                    set_polling_active.set(false);
                    set_polling_expired.set(true);
                });
                let id = web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        timeout_cb.as_ref().unchecked_ref(),
                        POLL_TIER1_MAX_MS as i32,
                    )
                    .unwrap();
                timeout_cb.forget();
                Some(id)
            }
            PollingTier::AwaitingCheckIn => None,
        };

        // Must keep interval closure alive for the lifetime of the timer
        interval_cb.forget();

        // Cleanup when this effect re-runs or component unmounts
        on_cleanup(move || {
            let _ = web_sys::window().map(|w| {
                w.clear_interval_with_handle(interval_id);
                if let Some(tid) = timeout_id {
                    w.clear_timeout_with_handle(tid);
                }
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
                    let still_needs = polling_tier(&data).is_some();
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

    // Reactive memos for fullscreen QR overlay
    let fullscreen_name: Memo<String> = Memo::new(move |_| {
        match state.get() {
            TicketState::Found(d) => utils::escape_html(&d.attendee.name),
            _ => String::new(),
        }
    });
    let fullscreen_qr_image: Memo<String> = Memo::new(move |_| {
        match state.get() {
            TicketState::Found(d) => d.qr_image.unwrap_or_default(),
            _ => String::new(),
        }
    });

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
                        let mut vd = TicketViewData::from_data(&data);
                        vd.deposit_href = build_deposit_href(&vd.api_id);

                        if vd.is_online {
                            view! {
                                <OnlineView view_data=vd />
                            }.into_any()
                        } else {
                            view! {
                                <InPersonView
                                    view_data=vd
                                    show_qr=show_qr
                                    set_show_qr=set_show_qr
                                    set_fullscreen_qr=set_fullscreen_qr
                                />
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

                // Auto-refresh polling indicator (tier-aware message)
                <Show
                    when=move || polling_active.get()
                    fallback=|| view! { <div></div> }
                >
                    <div class="ticket-poll">
                        {move || match &state.get() {
                            TicketState::Found(data) => match polling_tier(data) {
                                Some(PollingTier::AwaitingDeposit) => {
                                    "Checking for deposit verification...".into_any()
                                }
                                Some(PollingTier::AwaitingCheckIn) => {
                                    "Waiting for check-in at venue...".into_any()
                                }
                                None => view! { <div></div> }.into_any(),
                            },
                            _ => "Checking for updates...".into_any(),
                        }}
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

        // Fullscreen QR overlay — rendered outside main layout
        <FullscreenQrOverlay
            fullscreen_qr=fullscreen_qr
            set_fullscreen_qr=set_fullscreen_qr
            get_name=fullscreen_name
            get_qr_image=fullscreen_qr_image
        />
    }
}
