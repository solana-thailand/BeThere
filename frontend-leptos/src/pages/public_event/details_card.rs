use super::types::*;
use crate::api::EventFormat;
use crate::i18n::{Locale, td_string, use_i18n};
use crate::icons::{Icon, IconName};
use event_checkin_domain::models::event::safe_map_url;
use leptos::prelude::*;

/// The format badge text. `EventFormat::label()` stays English for staff pages.
fn format_label(locale: Locale, format: &EventFormat) -> &'static str {
    match format {
        EventFormat::InPerson => td_string!(locale, event.format_in_person),
        EventFormat::Online => td_string!(locale, event.format_online),
        EventFormat::Hybrid => td_string!(locale, event.format_hybrid),
    }
}

pub fn details_card(
    data: &PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
) -> AnyView {
    let i18n = use_i18n();
    let has_location = !data.location.is_empty();
    let location = data.location.clone();
    // The Worker already filters to https; re-check since this becomes an href.
    let location_map_url = data.location_map_url.as_deref().and_then(safe_map_url);
    let (start_ms, end_ms, time_tba) = (data.event_start_ms, data.event_end_ms, data.time_tba);
    // EN keeps its long weekday form; TH uses the shared Intl helper (Thai
    // month names, Buddhist-era year). Closures, so both follow a switch.
    let date_str = move || match i18n.get_locale() {
        Locale::en => format_event_date(start_ms),
        Locale::th => crate::utils::format_event_day(start_ms),
    };
    let time_str = move || {
        let locale = i18n.get_locale();
        if time_tba {
            return td_string!(locale, event.time_tba).to_string();
        }
        format!(
            "{} — {}{}",
            format_event_time(start_ms, locale),
            format_event_time(end_ms, locale),
            td_string!(locale, event.time_unit)
        )
    };

    let (badge_bg, badge_border, badge_color, badge_icon) = match data.event_format {
        crate::api::EventFormat::Online => (
            "rgba(99,102,241,0.12)",
            "rgba(99,102,241,0.3)",
            "#818cf8",
            IconName::Globe,
        ),
        crate::api::EventFormat::Hybrid => (
            "rgba(52,211,153,0.12)",
            "rgba(52,211,153,0.3)",
            "#34d399",
            IconName::Ticket,
        ),
        crate::api::EventFormat::InPerson => (
            "rgba(96,165,250,0.12)",
            "rgba(96,165,250,0.3)",
            "#60a5fa",
            IconName::Pin,
        ),
    };
    let event_format = data.event_format.clone();
    let fmt_label = move || format_label(i18n.get_locale(), &event_format);

    view! {
        <div class="pe-card">
            // Format badge
            <div class="pe-badge-row">
                <div style=format!("display:inline-flex;align-items:center;gap:0.4rem;background:{};border:1px solid {};border-radius:9999px;padding:0.25rem 0.75rem;font-size:0.8rem;font-weight:600;color:{};", badge_bg, badge_border, badge_color)>
                    <Icon icon=badge_icon class="icon-sm" />
                    {fmt_label}
                </div>
            </div>

            // Location — only render when an actual location string exists.
            // For online-only events without a location, the format badge above
            // already says "Online", so a redundant "Virtual Event" row here
            // would just be noise.
            {if has_location {
                let loc = location.clone();
                let map_link = location_map_url.clone().map(|href| view! {
                    <a
                        href=href
                        target="_blank"
                        rel="noopener noreferrer"
                        class="pe-map-link"
                    >
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.open_in_maps))}
                    </a>
                });
                view! {
                    <div class="pe-detail-row">
                        <span><Icon icon=IconName::Pin class="icon-sm icon-muted" /></span>
                        <span class="pe-detail-text">
                            {loc}
                            {map_link}
                        </span>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            // Date
            <div class="pe-detail-row">
                <span><Icon icon=IconName::Calendar class="icon-sm icon-muted" /></span>
                <span class="pe-detail-text">{date_str}</span>
            </div>

            // Time
            <div class="pe-time-indent">
                <span class="pe-detail-secondary">{time_str}</span>
            </div>

            // Countdown / Completed / Live
            {move || {
                let completed = event_completed.get();
                if completed {
                    view! {
                        <div class="pe-detail-row">
                            <span><Icon icon=IconName::Party class="icon-sm icon-success" /></span>
                            <span class="pe-text-success">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.event_completed))}</span>
                        </div>
                    }.into_any()
                } else {
                    let cd = countdown.get();
                    if cd.is_empty() {
                        // Countdown ended but event not marked completed — event is live
                        view! {
                            <div class="pe-detail-row">
                                <span class="pe-emoji-icon">"🔴"</span>
                                <span class="pe-text-accent-bold">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.happening_now))}</span>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="pe-detail-row">
                                <span><Icon icon=IconName::Timer class="icon-sm icon-muted" /></span>
                                <span class="pe-countdown-capsule">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, event.starts_in))}" "{cd}
                                </span>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }.into_any()
}
