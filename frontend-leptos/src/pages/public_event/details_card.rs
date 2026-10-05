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

    let format_icon = match data.event_format {
        crate::api::EventFormat::Online => IconName::Globe,
        // Pin is the Where row and Ticket the Capacity row: no icon twice.
        crate::api::EventFormat::Hybrid => IconName::Link,
        crate::api::EventFormat::InPerson => IconName::User,
    };
    let event_format = data.event_format.clone();
    let fmt_label = move || format_label(i18n.get_locale(), &event_format);
    // Times render in the viewer's zone; name it so a visitor abroad knows.
    let tz_label = crate::utils::local_tz_label(start_ms);
    let time_line = move || match (time_tba, tz_label.is_empty()) {
        (false, false) => format!("{} ({tz_label})", time_str()),
        _ => time_str(),
    };
    // The organizer's map link when set, otherwise a search for the venue text.
    let map_href = location_map_url.clone().unwrap_or_else(|| {
        let query = js_sys::encode_uri_component(&location);
        format!("https://www.google.com/maps/search/?api=1&query={query}")
    });

    view! {
        <div class="pe-card pe-meta">
            // When
            <div class="pe-meta-row">
                <Icon icon=IconName::Calendar class="icon-sm icon-muted" />
                <div class="pe-meta-body">
                    <span class="pe-meta-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.meta_when))}</span>
                    <span class="pe-detail-text">{date_str}</span>
                    <span class="pe-detail-secondary">{time_line}</span>
                    {move || {
                        if event_completed.get() {
                            return view! {
                                <span class="pe-text-success">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.event_completed))}</span>
                            }.into_any();
                        }
                        let cd = countdown.get();
                        match cd.is_empty() {
                            // Countdown ended but event not marked completed: it is live.
                            true => view! {
                                <span class="pe-text-accent-bold"><span class="pe-live-dot" aria-hidden="true"></span>{crate::locale::tr(|l| crate::i18n::td_string!(l, event.happening_now))}</span>
                            }.into_any(),
                            false => view! {
                                <span class="pe-countdown-capsule">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, event.starts_in))}" "{cd}
                                </span>
                            }.into_any(),
                        }
                    }}
                </div>
            </div>

            // Where: only with a real location. An online-only event says so in
            // the format row, so a "Virtual Event" row would be noise.
            {has_location.then(|| view! {
                <div class="pe-meta-row">
                    <Icon icon=IconName::Pin class="icon-sm icon-muted" />
                    <div class="pe-meta-body">
                        <span class="pe-meta-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.meta_where))}</span>
                        <span class="pe-detail-text">{location.clone()}</span>
                        <a href=map_href.clone() target="_blank" rel="noopener noreferrer" class="pe-map-link">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.open_in_maps))}
                        </a>
                    </div>
                </div>
            })}

            // Format
            <div class="pe-meta-row">
                <Icon icon=format_icon class="icon-sm icon-muted" />
                <div class="pe-meta-body">
                    <span class="pe-meta-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.meta_format))}</span>
                    <span class="pe-detail-text">{fmt_label}</span>
                </div>
            </div>

            // Attendance: who has registered, and the room left on a capped track.
            {super::attendance::attendance_row(data, event_completed)}
        </div>
    }.into_any()
}
