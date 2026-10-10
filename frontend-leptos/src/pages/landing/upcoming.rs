//! Upcoming public events (.plans/043 L3): the nearest three, a poster, the
//! event's own deposit rule in one line, and an honest empty state.

use leptos::prelude::*;
use serde::Deserialize;

use crate::api::ApiResponse;
use crate::i18n::{t_string, use_i18n};

use super::event_card::{DepositRule, nearest_first};

/// Lightweight event item from the public events API.
#[derive(Clone, Deserialize)]
struct PublicEventItem {
    name: String,
    slug: String,
    event_start_ms: i64,
    #[serde(default)]
    time_tba: bool,
    deposit_enabled: bool,
    /// The configured PromptPay amount; absent from older payloads = unknown.
    #[serde(default)]
    deposit_amount_thb: u64,
    #[serde(default)]
    event_format: String,
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
    /// A seeded demo event to show when nothing is live; absent until set.
    #[serde(default)]
    sample_event_slug: Option<String>,
}

/// Event cards the landing shows before "See all" (ASKS-4 §17: nearest 3).
const LANDING_EVENT_CARDS: usize = 3;

/// The community Discord, for the empty state (prototype, ASKS-4 §6).
const DISCORD_URL: &str = "https://discord.gg/PGbUgNmsns";

/// Upcoming Events section — fetches active events and displays them.
#[component]
pub(super) fn UpcomingEvents() -> impl IntoView {
    let i18n = use_i18n();
    let (events, set_events) = signal(Vec::<PublicEventItem>::new());
    let (sample_slug, set_sample_slug) = signal(None::<String>);
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
                                set_sample_slug
                                    .set(data.sample_event_slug.filter(|s| !s.is_empty()));
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
        <section id="events" class="lp-events">
            <div class="lp-wrap">
                <p class="lp-quote">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.quote))}</p>
                <h2 class="lp-h2">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.title))}</h2>
                {move || {
                    let mut evts = events.get();
                    if !loaded.get() {
                        return view! {
                            <p class="lp-rule" role="status">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.loading))}
                            </p>
                        }.into_any();
                    }
                    if evts.is_empty() {
                        return view! {
                            <div class="lp-event-list lp-one">
                                <div class="lp-card lp-event lp-event-empty">
                                    <h3>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.none_title))}</h3>
                                    <p class="lp-rule">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.none_desc))}</p>
                                    <div class="lp-row">
                                        {sample_slug.get().map(|slug| view! {
                                            <a href=format!("/e/{slug}") class="lp-btn lp-btn-primary">
                                                {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.sample_event))}
                                            </a>
                                        })}
                                        <a class="lp-btn" href=DISCORD_URL target="_blank" rel="noopener noreferrer">"Discord"</a>
                                    </div>
                                </div>
                            </div>
                        }.into_any();
                    }
                    let total = evts.len();
                    let mut order: Vec<(i64, usize)> =
                        evts.iter().enumerate().map(|(i, e)| (e.event_start_ms, i)).collect();
                    nearest_first(&mut order);
                    let mut slots: Vec<Option<PublicEventItem>> = evts.drain(..).map(Some).collect();
                    let shown: Vec<PublicEventItem> = order
                        .into_iter()
                        .take(LANDING_EVENT_CARDS)
                        .filter_map(|(_, i)| slots[i].take())
                        .collect();
                    let list_class = match shown.len() {
                        1 => "lp-event-list lp-one",
                        _ => "lp-event-list",
                    };
                    view! {
                        <div class=list_class>
                            {shown.into_iter().map(|evt| event_card(i18n, evt)).collect::<Vec<_>>()}
                        </div>
                        {(total > LANDING_EVENT_CARDS).then(|| {
                            let label = move || crate::locale::fill(
                                t_string!(i18n, landing.upcoming.see_all_n),
                                &[("count", &total.to_string())],
                            );
                            view! { <p class="lp-more"><a class="lp-btn" href="/events">{label}</a></p> }
                        })}
                    }.into_any()
                }}
            </div>
        </section>
    }
}

/// One card: poster, when, name, where, chips, and the deposit rule.
fn event_card(
    i18n: leptos_i18n::I18nContext<crate::i18n::Locale>,
    evt: PublicEventItem,
) -> impl IntoView {
    let event_url = format!("/e/{}", evt.slug);
    // A closure so the date and "TBA" follow a language switch.
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
    let rule = DepositRule::for_event(
        evt.deposit_enabled,
        evt.deposit_amount_thb,
        &evt.event_format,
    );
    let rule_text = move || match rule {
        Some(DepositRule::BackWhenYouShowUp(n)) => crate::locale::fill(
            t_string!(i18n, landing.upcoming.rule_back),
            &[("amount", &n.to_string())],
        ),
        Some(DepositRule::OnlineFree) => {
            t_string!(i18n, landing.upcoming.rule_online_free).to_string()
        }
        Some(DepositRule::Free) => t_string!(i18n, landing.upcoming.rule_free).to_string(),
        None => String::new(),
    };
    let online_chip = matches!(evt.event_format.as_str(), "online" | "hybrid").then(|| view! {
        <span class="lp-chip">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.upcoming.chip_online))}</span>
    });
    // Poster first, badge second — the order `event_hero` and `past_events` use.
    let has_poster = !evt.poster_url.is_empty();
    let image_url = match has_poster {
        true => evt.poster_url.clone(),
        false => evt.nft_image_url.clone(),
    };
    let image_alt = move || match has_poster {
        true => t_string!(i18n, landing.upcoming.poster_alt),
        false => t_string!(i18n, landing.upcoming.badge_alt),
    };
    let cover = (!image_url.is_empty()).then(|| view! {
        <img class="lp-cover" src=image_url alt=image_alt loading="lazy" width="1600" height="900" />
    });
    let location = (!evt.location.is_empty())
        .then(|| view! { <span class="lp-meta">{evt.location.clone()}</span> });
    view! {
        <a class="lp-card lp-event" href=event_url>
            {cover}
            <span class="lp-when">{date_str}</span>
            {crate::components::postponed_badge(&evt.postponed_note)}
            <h3>{evt.name}</h3>
            {location}
            {online_chip.map(|chip| view! { <div class="lp-row">{chip}</div> })}
            {(rule.is_some()).then(|| view! { <p class="lp-rule">{rule_text}</p> })}
        </a>
    }
}
