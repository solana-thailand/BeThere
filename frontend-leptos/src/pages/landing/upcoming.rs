//! Upcoming public events list.

use leptos::prelude::*;
use serde::Deserialize;

use crate::api::ApiResponse;
use crate::icons::{Icon, IconName};


/// Lightweight event item from the public events API.
#[derive(Clone, Deserialize)]
struct PublicEventItem {
    name: String,
    slug: String,
    event_start_ms: i64,
    #[serde(default)]
    time_tba: bool,
    deposit_enabled: bool,
    #[serde(default)]
    tagline: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    nft_image_url: String,
}

#[derive(Clone, Deserialize)]
#[derive(Default)]
struct PublicEventsResponse {
    events: Vec<PublicEventItem>,
}

/// Upcoming Events section — fetches active events and displays them.
#[component]
pub(super) fn UpcomingEvents() -> impl IntoView {
    let (events, set_events) = signal(Vec::<PublicEventItem>::new());
    let (loaded, set_loaded) = signal(false);

    // Fetch events on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/public/events");

            match crate::api::fetch::get(&url, &[]).await {
                Ok(resp) if resp.status() == 200 => {
                    match crate::api::fetch::response_json::<ApiResponse<PublicEventsResponse>>(&resp).await {
                        Ok(wrapper) => {
                            if let Some(data) = wrapper.data {
                                set_events.set(data.events);
                            }
                        }
                        Err(e) => {
                            log::warn!("[landing] failed to parse events: {e}");
                        }
                    }
                }
                Ok(_) => {
                    log::warn!("[landing] events API returned non-200");
                }
                Err(e) => {
                    log::warn!("[landing] events fetch error: {e}");
                }
            }
            set_loaded.set(true);
        });
    });

    view! {
        {move || {
            let evts = events.get();
            let is_loaded = loaded.get();
            let heading = view! {
                <div class="landing-section-header-sm">
                    <h2 class="landing-h2">
                        <Icon icon=IconName::Party class="icon-sm"/>" Upcoming Events"
                    </h2>
                    <p class="landing-subtitle">
                        "Reserve your spot with a deposit. Show up. Get refunded."
                    </p>
                </div>
            };
            if !is_loaded {
                // Still loading — show heading + spinner
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-events-loading">
                            <span class="landing-events-loading-spinner"></span>
                            <p class="landing-events-loading-text">"Loading events..."</p>
                        </div>
                    </section>
                }.into_any()
            } else if evts.is_empty() {
                // No events — show heading + sandbox demo card
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-sandbox-card">
                            <div class="landing-sandbox-icon">{"🎟️"}</div>
                            <div class="landing-sandbox-title">"No live events right now"</div>
                            <div class="landing-sandbox-desc">
                                "BeThere is a deposit-backed check-in platform. Try the flow below or host your own event."
                            </div>
                            <a href="#how-it-works" class="btn btn-primary btn-sm landing-sandbox-btn">
                                "See how it works ↓"
                            </a>
                        </div>
                        <div class="landing-sandbox-secondary">
                            <a href="#waitlist" class="btn btn-outline btn-sm">
                                "Organize an Event"
                            </a>
                        </div>
                    </section>
                }.into_any()
            } else {
                view! {
                    <section id="events" class="landing-section-sm">
                        {heading}
                        <div class="landing-events-grid">
                            {evts.into_iter().map(|evt| {
                                let event_url = format!("/e/{}", evt.slug);
                                let date_str = if evt.event_start_ms > 0 {
                                    let d = js_sys::Date::new_with_year_month_day(0, 0, 0);
                                    d.set_time(evt.event_start_ms as f64);
                                    if evt.time_tba {
                                        // Date only — show just the date, time is TBA
                                        let opts = js_sys::Object::new();
                                        let _ = js_sys::Reflect::set(&opts, &"year".into(), &"numeric".into());
                                        let _ = js_sys::Reflect::set(&opts, &"month".into(), &"short".into());
                                        let _ = js_sys::Reflect::set(&opts, &"day".into(), &"numeric".into());
                                        let date_part = d.to_locale_string("en-US", &opts).as_string().unwrap_or_default();
                                        format!("{date_part} · Time TBA")
                                    } else {
                                        d.to_locale_string("en-US", &js_sys::Object::new()).as_string().unwrap_or_default()
                                    }
                                } else {
                                    "Date TBA".to_string()
                                };
                                let deposit_badge = if evt.deposit_enabled {
                                    view! { <span class="landing-inline-icon"><Icon icon=IconName::Coin class="icon-xs"/>" Deposit required"</span> }.into_any()
                                } else {
                                    view! { <span class="landing-inline-icon"><Icon icon=IconName::TicketFree class="icon-xs"/>" Free entry"</span> }.into_any()
                                };

                                let badge_img = if !evt.nft_image_url.is_empty() {
                                    view! {
                                        <div class="landing-event-badge-img">
                                            <img
                                                src=evt.nft_image_url.clone()
                                                alt="Event badge"
                                            />
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                };

                                let tagline_html = if !evt.tagline.is_empty() {
                                    view! {
                                        <p class="landing-event-tagline">
                                            {evt.tagline.clone()}
                                        </p>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                };

                                let location_html = if !evt.location.is_empty() {
                                    view! {
                                        <p class="landing-event-location">
                                            <span class="landing-inline-icon"><Icon icon=IconName::Pin class="icon-xs"/>" "{evt.location.clone()}</span>
                                        </p>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                };

                                view! {
                                    <a
                                        href=event_url
                                        class="event-card-link"
                                    >
                                        <div
                                            class="card event-card landing-event-card"
                                        >
                                            {badge_img}
                                            <h3 class="landing-event-name">
                                                {evt.name}
                                            </h3>
                                            {tagline_html}
                                            <p class="landing-event-meta">
                                                <span class="landing-inline-icon"><Icon icon=IconName::Calendar class="icon-xs"/>" "{date_str}</span>
                                            </p>
                                            {location_html}
                                            <p class="landing-event-deposit">
                                                {deposit_badge}
                                            </p>
                                        </div>
                                    </a>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </section>
                }.into_any()
            }
        }}
    }
}
