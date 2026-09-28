//! Upcoming public events list.

use leptos::prelude::*;
use serde::Deserialize;

use crate::api::ApiResponse;
use crate::i18n::{t_string, use_i18n};
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
    /// Marketing poster, served from R2 as `/api/storage/posters/{event_id}`.
    ///
    /// First tier of the same fallback `event_hero` and the past-events card
    /// use. Without it this card showed `nft_image_url` — which for every event
    /// so far is the generic `badge-hd.svg` — so the one upcoming event on the
    /// landing page was unrecognisable (`.issues/094`).
    #[serde(default)]
    poster_url: String,
    /// Non-empty = postponed (migration 0053); the card shows a badge only.
    #[serde(default)]
    postponed_note: String,
}

#[derive(Clone, Deserialize, Default)]
struct PublicEventsResponse {
    events: Vec<PublicEventItem>,
}

/// Upcoming Events section — fetches active events and displays them.
#[component]
pub(super) fn UpcomingEvents() -> impl IntoView {
    let i18n = use_i18n();
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
                    match crate::api::fetch::response_json::<ApiResponse<PublicEventsResponse>>(
                        &resp,
                    )
                    .await
                    {
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
                        <Icon icon=IconName::Party class="icon-sm"/>" "{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.title))}
                    </h2>
                    <p class="landing-subtitle">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.subtitle))}
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
                            <p class="landing-events-loading-text">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.loading))}</p>
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
                            <div class="landing-sandbox-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.none_title))}</div>
                            <div class="landing-sandbox-desc">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.none_desc))}
                            </div>
                            <a href="#how-it-works" class="btn btn-primary btn-sm landing-sandbox-btn">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.see_how))}
                            </a>
                        </div>
                        <div class="landing-sandbox-secondary">
                            <a href="#waitlist" class="btn btn-outline btn-sm">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.organize))}
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
                                // A closure so the date and "TBA" follow a
                                // language switch.
                                let (start_ms, time_tba) = (evt.event_start_ms, evt.time_tba);
                                let date_str = move || match (start_ms > 0, time_tba) {
                                    (false, _) => t_string!(i18n, landing.upcoming.date_tba).to_string(),
                                    (true, true) => format!(
                                        "{} · {}",
                                        crate::utils::format_event_day(start_ms),
                                        t_string!(i18n, landing.upcoming.time_tba)
                                    ),
                                    (true, false) => crate::utils::format_event_datetime(start_ms),
                                };
                                let deposit_badge = if evt.deposit_enabled {
                                    view! { <span class="landing-inline-icon"><Icon icon=IconName::Coin class="icon-xs"/>" "{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.deposit_required))}</span> }.into_any()
                                } else {
                                    view! { <span class="landing-inline-icon"><Icon icon=IconName::TicketFree class="icon-xs"/>" "{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.free_entry))}</span> }.into_any()
                                };

                                // Poster first, badge second — the same order
                                // `event_hero` and `past_events` already use.
                                let has_poster = !evt.poster_url.is_empty();
                                let image_url = match has_poster {
                                    true => evt.poster_url.clone(),
                                    false => evt.nft_image_url.clone(),
                                };
                                let image_alt = move || match has_poster {
                                    true => t_string!(i18n, landing.upcoming.poster_alt),
                                    false => t_string!(i18n, landing.upcoming.badge_alt),
                                };
                                let badge_img = if !image_url.is_empty() {
                                    view! {
                                        <div class="landing-event-badge-img">
                                            <img
                                                src=image_url
                                                alt=image_alt
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
                                            {crate::components::postponed_badge(&evt.postponed_note)}
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
