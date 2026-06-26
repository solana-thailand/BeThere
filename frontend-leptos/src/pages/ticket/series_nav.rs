//! Series navigation (Plan 013) — "Related events" section on the ticket page.
//!
//! Surfaces the campaign/series an event belongs to (if any) as a
//! "Part of {Series}" badge plus prev/next links to neighboring events.
//! Loads once per ticket view via the public, cached event-series endpoint.
//! Renders nothing when the event has no campaign, on load, or on error — the
//! section is purely additive and must never block the rest of the ticket.

use leptos::prelude::*;

use crate::api::{EventSeries, SeriesEvent, get_event_series};
use crate::utils;

/// Compact, locale-aware date label for a neighbor card (e.g. "Jun 26").
/// Returns an em dash when the timestamp is missing/zero (matches the
/// codebase convention in `event_form::format_date_display`).
fn short_date(ms: i64) -> String {
    if ms == 0 {
        return "\u{2014}".to_string();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let locale = "en-US";
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &opts,
        &"month".into(),
        &wasm_bindgen::JsValue::from_str("short"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &"day".into(),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );
    date.to_locale_date_string(locale, &opts)
        .as_string()
        .unwrap_or_else(|| "\u{2014}".to_string())
}

/// "Part of {Series}" badge + prev/next navigation for the ticket page.
///
/// Hidden (renders nothing) when:
/// - `event_id` is empty,
/// - the event is not part of any campaign (404),
/// - the request fails (non-blocking — the ticket still works).
#[component]
pub fn SeriesNav(
    /// The current event id, used to look up its campaign + neighbors.
    #[prop(into)]
    event_id: String,
) -> impl IntoView {
    let (series, set_series) = signal(Option::<EventSeries>::None);

    // Fetch once when event_id is known. Re-runs only if event_id actually
    // changes (e.g. SPA navigation between tickets — though the ticket page
    // already remounts per navigation).
    Effect::new(move |_| {
        let eid = event_id.clone();
        if eid.is_empty() {
            return;
        }
        // Clear stale series while the new one loads so a previous event's
        // neighbors never bleed into the current view.
        set_series.set(None);
        leptos::task::spawn_local(async move {
            match get_event_series(&eid).await {
                Ok(s) => set_series.set(s),
                Err(e) => {
                    // Non-blocking: log and keep the section hidden.
                    log::warn!("[series_nav] failed to load series for {eid}: {e}");
                }
            }
        });
    });

    view! {
        <Show
            when=move || series.get().is_some()
            fallback=|| view! { <div></div> }
        >
            {move || {
                let Some(s) = series.get() else {
                    return view! { <div></div> }.into_any();
                };
                let campaign_title = s.campaign.title.clone();
                let campaign_desc = s.campaign.description.clone();
                let total = s.events.len();
                let position = if s.current_index >= 0 && total > 0 {
                    // 1-indexed "x of n" for display.
                    format!("{} of {}", s.current_index + 1, total)
                } else {
                    String::new()
                };
                let prev = s.previous.clone();
                let nxt = s.next.clone();

                view! {
                    <div class="ticket-series-nav">
                        <div class="ticket-series-badge">
                            <span class="ticket-series-badge-label">"Part of"</span>
                            <span class="ticket-series-badge-title">
                                {utils::escape_html(&campaign_title)}
                            </span>
                            {if !position.is_empty() {
                                view! {
                                    <span class="ticket-series-badge-position">
                                        {position}
                                    </span>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}
                        </div>
                        {if !campaign_desc.is_empty() {
                            let d = campaign_desc;
                            view! {
                                <p class="ticket-series-desc">
                                    {utils::escape_html(&d)}
                                </p>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }}
                        <div class="ticket-series-neighbors">
                            <SeriesNeighborCard event=prev direction="previous" />
                            <SeriesNeighborCard event=nxt direction="next" />
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>
    }
}

/// One prev/next card. Renders an empty placeholder when the neighbor is
/// missing (e.g. the current event is first or last in the series) so the
/// two-column layout stays balanced.
#[component]
fn SeriesNeighborCard(
    event: Option<SeriesEvent>,
    /// "previous" | "next" — controls the chevron direction + label.
    #[prop(into)]
    direction: String,
) -> impl IntoView {
    let is_next = direction == "next";
    let label = if is_next { "Up next" } else { "Previous" };

    match event {
        Some(e) => {
            let href = format!("/e/{}", e.slug);
            let date = short_date(e.event_start_ms);
            let name = e.name;
            view! {
                <a class="ticket-series-card" href=href>
                    <span class="ticket-series-card-label">{label}</span>
                    <span class="ticket-series-card-name">
                        {utils::escape_html(&name)}
                    </span>
                    <span class="ticket-series-card-date">{date}</span>
                    {if is_next {
                        view! { <span class="ticket-series-card-chevron">"→"</span> }.into_any()
                    } else {
                        view! { <span class="ticket-series-card-chevron">"←"</span> }.into_any()
                    }}
                </a>
            }.into_any()
        }
        // Keep the grid balanced with an inert spacer (no link affordance —
        // a real <a href=""> would navigate and confuse screen readers).
        None => {
            let label = if is_next { "Last in series" } else { "First in series" };
            view! {
                <div class="ticket-series-card ticket-series-card--empty">
                    <span class="ticket-series-card-label">{label}</span>
                </div>
            }.into_any()
        }
    }
}
