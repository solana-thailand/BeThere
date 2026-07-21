use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn details_card(
    data: &PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
) -> AnyView {
    let has_location = !data.location.is_empty();
    let location = data.location.clone();
    let date_str = format_event_date(data.event_start_ms);
    let time_str = if data.time_tba {
        "Time TBA".to_string()
    } else {
        format!(
            "{} — {}",
            format_event_time(data.event_start_ms),
            format_event_time(data.event_end_ms)
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
    let fmt_label = data.event_format.label();

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
                view! {
                    <div class="pe-detail-row">
                        <span><Icon icon=IconName::Pin class="icon-sm icon-muted" /></span>
                        <span class="pe-detail-text">{loc}</span>
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
                            <span class="pe-text-success">"Event Completed"</span>
                        </div>
                    }.into_any()
                } else {
                    let cd = countdown.get();
                    if cd.is_empty() {
                        // Countdown ended but event not marked completed — event is live
                        view! {
                            <div class="pe-detail-row">
                                <span class="pe-emoji-icon">"🔴"</span>
                                <span class="pe-text-accent-bold">"Happening now!"</span>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="pe-detail-row">
                                <span><Icon icon=IconName::Timer class="icon-sm icon-muted" /></span>
                                <span class="pe-countdown-capsule">
                                    "Starts in "{cd}
                                </span>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }.into_any()
}
