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

use super::in_person_view::InPersonView;
use super::online_view::OnlineView;
use super::qr_section::FullscreenQrOverlay;
use super::view_data::TicketViewData;

/// Polling interval for auto-refresh.
const POLL_INTERVAL_MS: u32 = 10_000;
/// Maximum duration for auto-refresh polling before stopping.
const POLL_MAX_MS: u32 = 300_000; // 5 minutes

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

        // Fullscreen QR overlay — rendered outside main layout
        <FullscreenQrOverlay
            fullscreen_qr=fullscreen_qr
            set_fullscreen_qr=set_fullscreen_qr
            get_name=fullscreen_name
            get_qr_image=fullscreen_qr_image
        />
    }
}
