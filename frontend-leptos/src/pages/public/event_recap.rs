//! Public recap page for a completed event (Plan 008 — Phase 2 §3.2.4).
//!
//! Route: `/events/:slug/recap` (unauthenticated).
//!
//! Renders the published recap for a completed event:
//!   - Hero image (recap_image_url when set, else the event's marketing poster,
//!     else the NFT badge image)
//!   - Event name + date + location + tagline
//!   - Headline funnel ("X registered · Y checked in · Z claimed")
//!   - Recap markdown body (rendered as preformatted text in v1 — a future
//!     phase can pull in `pulldown-cmark` for full HTML rendering)
//!   - "Frozen at {timestamp}" badge so readers know the numbers are point-
//!     in-time, not live
//!
//! Data source: `GET /api/public/event/{slug}/recap`, which 404s whenever the
//! event isn't found, isn't `Completed`, has no published recap, or has no
//! frozen summary. All four cases are indistinguishable from "no recap" by
//! design (don't leak the existence of unpublished drafts).

use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::components::A;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::api::{self, PublicRecapData};
use crate::icons::{Icon, IconName};

/// Route parameters for `/events/:slug/recap`.
#[derive(Params, PartialEq, Clone, Debug)]
struct EventRecapParams {
    slug: Option<String>,
}

/// Coarse load state for the recap page. Mirrors the public event page's
/// pattern: a transient fetch error doesn't blank the page once we have data.
#[derive(Debug, Clone, PartialEq)]
enum RecapLoadState {
    Loading,
    Loaded,
    NotFound,
    Failed(String),
}

/// Public recap page component.
#[component]
#[allow(non_snake_case)]
pub fn EventRecap() -> impl IntoView {
    let params = use_params::<EventRecapParams>();

    let slug_val: String = match params.get() {
        Ok(p) => p.slug.unwrap_or_default(),
        Err(_) => String::new(),
    };

    let (data, set_data) = signal(Option::<PublicRecapData>::None);
    let (load_state, set_load_state) = signal(RecapLoadState::Loading);

    Effect::new(move |_| {
        let slug = slug_val.clone();
        if slug.is_empty() {
            set_load_state.set(RecapLoadState::NotFound);
            return;
        }
        set_load_state.set(RecapLoadState::Loading);
        let set_d = set_data;
        let set_ls = set_load_state;
        leptos::task::spawn_local(async move {
            match api::get_public_recap(&slug).await {
                Ok(payload) => {
                    set_d.set(Some(payload));
                    set_ls.set(RecapLoadState::Loaded);
                }
                Err(e) => {
                    // 404 is the canonical "no public recap" signal — backend
                    // returns it for any of: not-found / not-completed /
                    // not-published / no-frozen-summary. Render the friendly
                    // "no recap" view rather than a hard error.
                    if e.status == 404 {
                        set_load_state.set(RecapLoadState::NotFound);
                        return;
                    }
                    log::error!("[event-recap] load failed: {e}");
                    set_load_state.set(RecapLoadState::Failed(format!("{e}")));
                }
            }
        });
    });

    let is_loading = move || matches!(load_state.get(), RecapLoadState::Loading) && data.get().is_none();
    let is_not_found = move || matches!(load_state.get(), RecapLoadState::NotFound);
    let is_hard_failure = move || {
        matches!(load_state.get(), RecapLoadState::Failed(_)) && data.get().is_none()
    };

    view! {
        <Title text="Event Recap — BeThere" />
        <Meta name="robots" content="index,follow" />
        <div class="center-page">
            <div class="container layout-col-center">
                // ---------- Loading ----------
                <Show when=move || is_loading() fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading recap..."
                    </div>
                </Show>

                // ---------- 404 (no recap published) ----------
                <Show when=move || is_not_found() fallback=|| view! { <div></div> }>
                    <div class="card layout-col-center">
                        <span style="margin-bottom:1rem;opacity:0.6;">
                            <Icon icon=IconName::Calendar class="icon-2xl" />
                        </span>
                        <h2 style="margin:0 0 0.5rem;">"No recap available"</h2>
                        <p class="subtitle" style="margin:0 0 1rem;text-align:center;">
                            "This event hasn't published a recap yet — check back later."
                        </p>
                        <A href="/past-events" attr:class="btn btn-outline btn-sm">"← All past events"</A>
                    </div>
                </Show>

                // ---------- Hard failure ----------
                <Show when=move || is_hard_failure() fallback=|| view! { <div></div> }>
                    <div class="card">
                        <h2>"Failed to load recap"</h2>
                        <p class="subtitle">
                            {move || match load_state.get() {
                                RecapLoadState::Failed(msg) => msg,
                                _ => String::new(),
                            }}
                        </p>
                        <A href="/past-events" attr:class="btn btn-primary">"Back to past events"</A>
                    </div>
                </Show>

                // ---------- Recap content ----------
                <Show when=move || data.get().is_some() fallback=|| view! { <div></div> }>
                    {move || {
                        let payload = data.get().unwrap_or_default();
                        render_recap(payload).into_any()
                    }}
                </Show>
            </div>
        </div>
    }
}

/// Render the full recap view from the loaded payload.
fn render_recap(payload: PublicRecapData) -> impl IntoView {
    let event = payload.event.clone();
    let image_url = if !payload.recap_image_url.is_empty() {
        payload.recap_image_url.clone()
    } else if !event.poster_url.is_empty() {
        event.poster_url.clone()
    } else {
        event.nft_image_url.clone()
    };
    let date_str = format_event_date_range(event.event_start_ms, event.event_end_ms);
    let funnel = payload.funnel.clone();
    let markdown = payload.recap_markdown.clone();
    let published_str = payload
        .recap_published_at
        .as_deref()
        .map(format_iso)
        .unwrap_or_default();
    let frozen_str = payload
        .frozen_at
        .as_deref()
        .map(format_iso)
        .unwrap_or_default();

    view! {
        // ── Back link ──
        <div class="flex-row-gap" style="margin-bottom:1rem;width:100%;justify-content:flex-start;">
            <A href="/past-events" attr:class="btn btn-outline btn-sm">"← All past events"</A>
        </div>

        // ── Hero image ──
        {if !image_url.is_empty() {
            view! {
                <div class="pe-hero" style="margin-bottom:1.5rem;">
                    <img src=image_url.clone() alt=&event.name class="pe-hero-img" />
                </div>
            }
                .into_any()
        } else {
            view! {
                <div class="pe-hero" style="margin-bottom:1.5rem;">
                    <span><Icon icon=IconName::Ticket class="icon-2xl" /></span>
                </div>
            }
                .into_any()
        }}

        // ── Title + meta + badges ──
        <div class="card" style="width:100%;margin-bottom:1.5rem;">
            <h1 style="margin:0 0 0.5rem;">{event.name.clone()}</h1>
            {move || {
                let tagline = event.tagline.clone();
                if tagline.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! {
                        <p class="subtitle" style="margin:0 0 1rem;">{tagline}</p>
                    }
                        .into_any()
                }
            }}

            <div class="flex-row-gap events-flex-wrap-center" style="margin-bottom:0.5rem;">
                {move || {
                    let date = date_str.clone();
                    if date.is_empty() {
                        view! { <div></div> }.into_any()
                    } else {
                        view! {
                            <span class="badge badge-info-xs">
                                <Icon icon=IconName::Calendar class="icon-sm" />
                                {format!(" {date}")}
                            </span>
                        }
                            .into_any()
                    }
                }}
                {move || {
                    let location = event.location.clone();
                    if location.is_empty() {
                        view! { <div></div> }.into_any()
                    } else {
                        view! {
                            <span class="badge badge-info-xs">
                                <Icon icon=IconName::Pin class="icon-sm" />
                                {format!(" {location}")}
                            </span>
                        }
                            .into_any()
                    }
                }}
                <span class="badge badge-success-xs">
                    <Icon icon=IconName::Check class="icon-sm" />
                    {format!(" Published {published_str}")}
                </span>
            </div>
        </div>

        // ── Headline funnel ──
        <div class="card" style="width:100%;margin-bottom:1.5rem;">
            <h2 style="margin:0 0 1rem;font-size:1.125rem;">"By the numbers"</h2>
            <div class="events-grid events-grid-3">
                <div class="stat-tile">
                    <div class="stat-tile-value">{funnel.registered_count}</div>
                    <div class="stat-tile-label">"Registered"</div>
                </div>
                <div class="stat-tile">
                    <div class="stat-tile-value">{funnel.checked_in_count}</div>
                    <div class="stat-tile-label">"Checked in"</div>
                </div>
                <div class="stat-tile">
                    <div class="stat-tile-value">{funnel.claimed_count}</div>
                    <div class="stat-tile-label">"Badges claimed"</div>
                </div>
            </div>
            {move || {
                if frozen_str.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! {
                        <div class="hint-info" style="margin-top:0.75rem;">
                            <Icon icon=IconName::Lock class="icon-sm" />
                            {format!(" Snapshot frozen {frozen_str} — later activity isn't reflected.")}
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>

        // ── Recap body (rendered as preformatted text in v1) ──
        {move || {
            let md = markdown.clone();
            if md.trim().is_empty() {
                view! { <div></div> }.into_any()
            } else {
                view! {
                    <div class="card" style="width:100%;margin-bottom:1.5rem;">
                        // Preformatted rather than rendered HTML — a future
                        // phase can pull in `pulldown-cmark` for full markdown
                        // rendering. Preformatted avoids a JS interop dependency
                        // and still preserves layout for the organizer-authored
                        // markdown body.
                        <pre class="recap-body" style="white-space:pre-wrap;font-family:inherit;margin:0;line-height:1.6;">{md}</pre>
                    </div>
                }
                    .into_any()
            }
        }}
    }
}

// ---------------------------------------------------------------------------
// Local formatting helpers
// ---------------------------------------------------------------------------

/// Format a millisecond timestamp as `YYYY-MM-DD`. Returns an empty string
/// when the timestamp is non-positive (unknown / unset).
fn format_event_date_range(start_ms: i64, end_ms: i64) -> String {
    let start = format_event_date(start_ms);
    if start.is_empty() {
        return String::new();
    }
    // Single-day events (or unknown end) collapse to the start date.
    let end = format_event_date(end_ms);
    if end.is_empty() || end == start {
        return start;
    }
    format!("{start} – {end}")
}

/// Format a millisecond timestamp as `YYYY-MM-DD`.
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

/// Format an ISO 8601 timestamp for display. Mirrors the pattern in
/// `event_summary.rs::format_iso` (no `chrono` dependency).
fn format_iso(iso: &str) -> String {
    let parsed = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    if parsed.get_time().is_nan() {
        return iso.to_string();
    }
    let year = parsed.get_full_year();
    let month = parsed.get_month() + 1; // 0-indexed
    let day = parsed.get_date();
    format!("{year:04}-{month:02}-{day:02}")
}
