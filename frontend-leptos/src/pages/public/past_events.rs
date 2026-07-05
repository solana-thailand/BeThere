//! Public past-events feed (Plan 008 — Phase 2 §3.2.4).
//!
//! Route: `/past-events` (unauthenticated).
//!
//! Renders a grid of completed events with a published public recap. Each card
//! shows the event's marketing poster (falling back to the NFT badge image),
//! name, date, tagline, location, and a "Read recap" CTA linking to the
//! dedicated recap page (`/events/{slug}/recap`).
//!
//! Data source: `GET /api/public/events/past`, which the backend filters to
//! `status = 'completed' AND recap_published = 1` sorted by `event_end_ms DESC`.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;

use crate::api::{self, PastEventItem, PastEventsResponse};
use crate::icons::{Icon, IconName};

/// Coarse load state for the feed. Once we have any data we keep it on screen,
/// mirroring the live dashboard + event summary's stale-on-error behavior so a
/// transient blip never blanks the page.
#[derive(Debug, Clone, PartialEq)]
enum PastEventsLoadState {
    Loading,
    Loaded,
    Failed(String),
}

/// Past-events feed page.
#[component]
#[allow(non_snake_case)]
pub fn PastEvents() -> impl IntoView {
    let (events, set_events) = signal(Vec::<PastEventItem>::new());
    let (load_state, set_load_state) = signal(PastEventsLoadState::Loading);

    Effect::new(move |_| {
        set_load_state.set(PastEventsLoadState::Loading);
        let set_ev = set_events;
        let set_ls = set_load_state;
        leptos::task::spawn_local(async move {
            match api::list_past_events().await {
                Ok(PastEventsResponse { events: list }) => {
                    set_ev.set(list);
                    set_ls.set(PastEventsLoadState::Loaded);
                }
                Err(e) => {
                    log::error!("[past-events] load failed: {e}");
                    set_ls.set(PastEventsLoadState::Failed(format!("{e}")));
                }
            }
        });
    });

    let is_loading = move || {
        matches!(load_state.get(), PastEventsLoadState::Loading) && events.get().is_empty()
    };
    let is_hard_failure = move || {
        matches!(load_state.get(), PastEventsLoadState::Failed(_)) && events.get().is_empty()
    };
    let is_empty = move || {
        matches!(load_state.get(), PastEventsLoadState::Loaded) && events.get().is_empty()
    };

    view! {
        <Title text="Past Events — BeThere" />
        <div class="center-page">
            <div class="container layout-col-center">
                // ---------- Header ----------
                <div class="flex-row-gap events-flex-wrap-center" style="margin-bottom:2rem;width:100%;">
                    <h1 style="margin:0;">"Past Events"</h1>
                    <span class="badge badge-info-xs">
                        {move || events.get().len().to_string()}
                    </span>
                </div>

                // ---------- Loading ----------
                <Show when=move || is_loading() fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading past events..."
                    </div>
                </Show>

                // ---------- Hard failure ----------
                <Show when=move || is_hard_failure() fallback=|| view! { <div></div> }>
                    <div class="card">
                        <h2>"Failed to load"</h2>
                        <p class="subtitle">
                            {move || match load_state.get() {
                                PastEventsLoadState::Failed(msg) => msg,
                                _ => String::new(),
                            }}
                        </p>
                        <a href="/past-events" class="btn btn-primary">"Try again"</a>
                    </div>
                </Show>

                // ---------- Empty state ----------
                <Show when=move || is_empty() fallback=|| view! { <div></div> }>
                    <div class="card layout-col-center">
                        <span style="margin-bottom:1rem;opacity:0.6;">
                            <Icon icon=IconName::Calendar class="icon-2xl" />
                        </span>
                        <h2 style="margin:0 0 0.5rem;">"No past events yet"</h2>
                        <p class="subtitle" style="margin:0;">
                            "Recaps from completed events will appear here once published."
                        </p>
                    </div>
                </Show>

                // ---------- Grid of event cards ----------
                <Show when=move || !events.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="events-grid">
                        {move || {
                            events
                                .get()
                                .into_iter()
                                .map(|ev| past_event_card(ev).into_any())
                                .collect::<Vec<_>>()
                        }}
                    </div>
                </Show>

                // ---------- Back to landing ----------
                <div class="flex-row-gap" style="margin-top:2rem;">
                    <A href="/" attr:class="btn btn-outline btn-sm">"← Back to home"</A>
                </div>
            </div>
        </div>
    }
}

/// Render a single past-event card. The hero image prefers the marketing
/// `poster_url`, falling back to the NFT badge image, then to a Ticket icon
/// empty-state — mirroring the dedicated event-page hero logic.
fn past_event_card(ev: PastEventItem) -> impl IntoView {
    let image_url = if !ev.poster_url.is_empty() {
        ev.poster_url.clone()
    } else {
        ev.nft_image_url.clone()
    };
    let date_str = format_event_date(ev.event_start_ms);
    let slug = ev.slug.clone();

    view! {
        <A href=format!("/events/{slug}/recap") attr:class="event-card-link">
            <div class="event-card">
                // Hero image (or icon fallback).
                <div class="event-card-image">
                    {if !image_url.is_empty() {
                        view! {
                            <img src=image_url.clone() alt=&ev.name class="event-card-img" />
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="event-card-image-placeholder">
                                <Icon icon=IconName::Ticket class="icon-xl" />
                            </div>
                        }
                            .into_any()
                    }}
                </div>

                // Body: name + date + tagline + location.
                <div class="event-card-body">
                    <h3 class="event-card-title">{ev.name.clone()}</h3>
                    <div class="event-card-date">{date_str}</div>
                    {move || {
                        let tagline = ev.tagline.clone();
                        if tagline.is_empty() {
                            view! { <div></div> }.into_any()
                        } else {
                            view! {
                                <p class="event-card-tagline">{tagline}</p>
                            }
                                .into_any()
                        }
                    }}
                    {move || {
                        let location = ev.location.clone();
                        if location.is_empty() {
                            view! { <div></div> }.into_any()
                        } else {
                            view! {
                                <div class="event-card-location">
                                    <Icon icon=IconName::Pin class="icon-sm" />
                                    <span>{location}</span>
                                </div>
                            }
                                .into_any()
                        }
                    }}

                    // CTA.
                    <div class="event-card-cta">
                        <span class="btn btn-outline btn-sm">"Read recap →"</span>
                    </div>
                </div>
            </div>
        </A>
    }
}

/// Format a millisecond timestamp as a human-readable date string.
///
/// Mirrors `crate::pages::public_event::types::format_event_date` but kept
/// local so this page has no cross-module coupling. Falls back to the raw
/// timestamp when the JS `Date` can't parse the value (e.g. 0 / NaN).
fn format_event_date(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&(ms as f64).into());
    if date.get_time().is_nan() {
        return ms.to_string();
    }
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    format!("{year:04}-{month:02}-{day:02}")
}
