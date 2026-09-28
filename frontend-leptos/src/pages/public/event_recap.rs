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
use crate::i18n::{t, t_string, use_i18n};
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

    let is_loading =
        move || matches!(load_state.get(), RecapLoadState::Loading) && data.get().is_none();
    let is_not_found = move || matches!(load_state.get(), RecapLoadState::NotFound);
    let is_hard_failure =
        move || matches!(load_state.get(), RecapLoadState::Failed(_)) && data.get().is_none();

    view! {
        <Title text=crate::locale::tr(|l| crate::i18n::td_string!(l, recap.page_title)) />
        <Meta name="robots" content="index,follow" />
        <div class="center-page">
            <div class="container layout-col-center">
                // ---------- Loading ----------
                <Show when=move || is_loading() fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.loading))}
                    </div>
                </Show>

                // ---------- 404 (no recap published) ----------
                <Show when=move || is_not_found() fallback=|| view! { <div></div> }>
                    <div class="card layout-col-center">
                        <span style="margin-bottom:1rem;opacity:0.6;">
                            <Icon icon=IconName::Calendar class="icon-2xl" />
                        </span>
                        <h2 style="margin:0 0 0.5rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.none_title))}</h2>
                        <p class="subtitle" style="margin:0 0 1rem;text-align:center;">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.none_body))}
                        </p>
                        <A href="/past-events" attr:class="btn btn-outline btn-sm">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.all_past))}</A>
                    </div>
                </Show>

                // ---------- Hard failure ----------
                <Show when=move || is_hard_failure() fallback=|| view! { <div></div> }>
                    <div class="card">
                        <h2>{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.failed_title))}</h2>
                        <p class="subtitle">
                            {move || match load_state.get() {
                                RecapLoadState::Failed(msg) => msg,
                                _ => String::new(),
                            }}
                        </p>
                        <A href="/past-events" attr:class="btn btn-primary">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.back_past))}</A>
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
    let i18n = use_i18n();
    let event = payload.event.clone();
    let image_url = if !payload.recap_image_url.is_empty() {
        payload.recap_image_url.clone()
    } else if !event.poster_url.is_empty() {
        event.poster_url.clone()
    } else {
        event.nft_image_url.clone()
    };
    let (start_ms, end_ms) = (event.event_start_ms, event.event_end_ms);
    let funnel = payload.funnel.clone();
    let markdown = payload.recap_markdown.clone();
    let video_url = event.video_url.clone();
    let learning_resources = event.learning_resources.clone();
    // Dates are formatted inside closures below so they follow a language
    // switch.
    let published_at = payload.recap_published_at.clone().unwrap_or_default();
    let frozen_at = payload.frozen_at.clone().unwrap_or_default();

    view! {
        // ── Back link ──
        <div class="flex-row-gap" style="margin-bottom:1rem;width:100%;justify-content:flex-start;">
            <A href="/past-events" attr:class="btn btn-outline btn-sm">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.all_past))}</A>
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
                    let date = format_event_date_range(start_ms, end_ms);
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
                    " "
                    {t!(i18n, recap.published, date = move || format_iso(&published_at))}
                </span>
            </div>
        </div>

        // ── Headline funnel ──
        <div class="card" style="width:100%;margin-bottom:1.5rem;">
            <h2 style="margin:0 0 1rem;font-size:1.125rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.by_numbers))}</h2>
            <div class="events-grid events-grid-3">
                <div class="stat-tile">
                    <div class="stat-tile-value">{funnel.registered_count}</div>
                    <div class="stat-tile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.registered))}</div>
                </div>
                <div class="stat-tile">
                    <div class="stat-tile-value">{funnel.checked_in_count}</div>
                    <div class="stat-tile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.checked_in))}</div>
                </div>
                <div class="stat-tile">
                    <div class="stat-tile-value">{funnel.claimed_count}</div>
                    <div class="stat-tile-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.claimed))}</div>
                </div>
            </div>
            {move || {
                if frozen_at.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! {
                        <div class="hint-info" style="margin-top:0.75rem;">
                            <Icon icon=IconName::Lock class="icon-sm" />
                            " "
                            {
                                let frozen_at = frozen_at.clone();
                                t!(i18n, recap.frozen, date = move || format_iso(&frozen_at))
                            }
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>

        // Reuse the ticket recording component so current attendees and people
        // catching up later receive identical URL handling and presentation.
        {if video_url.is_empty() {
            ().into_any()
        } else {
            view! {
                <div style="width:100%;margin-bottom:1.5rem;">
                    <crate::pages::ticket::video_section::VideoSection
                        video_url=video_url
                        variant="card".to_string()
                    />
                </div>
            }.into_any()
        }}

        {render_learning_resources(learning_resources)}

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

        // ── Post-event registration CTA (Plan 008 — Phase 3) ──
        // Shown only when the organizer has opened lead capture for this
        // completed event. Links to the stripped registration form.
        {move || {
            let slug = event.slug.clone();
            let open = event.post_event_registration_open;
            if !open {
                view! { <div></div> }.into_any()
            } else {
                view! {
                    <div class="card" style="width:100%;margin-bottom:1.5rem;text-align:center;">
                        <span style="display:block;margin-bottom:0.5rem;opacity:0.7;">
                            <Icon icon=IconName::Lightbulb class="icon-lg" />
                        </span>
                        <h2 style="margin:0 0 0.5rem;font-size:1.125rem;">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.missed_title))}
                        </h2>
                        <p class="subtitle" style="margin:0 0 1rem;">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.missed_body))}
                        </p>
                        <A href=format!("/events/{slug}/post-event-register")
                            attr:class="btn btn-primary"
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.join_cta))}
                        </A>
                    </div>
                }
                    .into_any()
            }
        }}
    }
}

fn render_learning_resources(links: Vec<crate::api::CommunityLink>) -> AnyView {
    if links.is_empty() {
        return ().into_any();
    }

    let i18n = use_i18n();
    let items = links
        .into_iter()
        .enumerate()
        .map(|(index, link)| {
            // `platform` is a stored code; only the label is translated. The
            // organizer's own label is shown as written.
            let platform = link.platform.clone();
            let label = link.label.clone();
            let display_label = move || {
                let kind = match platform.as_str() {
                    "slides" => t_string!(i18n, recap.kind.slides),
                    "source" => t_string!(i18n, recap.kind.source),
                    "download" => t_string!(i18n, recap.kind.download),
                    _ => t_string!(i18n, recap.kind.resource),
                };
                match label.trim().is_empty() {
                    true => kind.to_string(),
                    false => format!("{label} · {kind}"),
                }
            };
            view! {
                <a
                    href=link.url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="pe-community-link-item"
                >
                    <span class="recap-resource-order" aria-hidden="true">{index + 1}</span>
                    <span class="pe-community-link-label">{display_label}</span>
                    <span aria-hidden="true">"↗"</span>
                </a>
            }
        })
        .collect::<Vec<_>>();

    view! {
        <div class="card" style="width:100%;margin-bottom:1.5rem;">
            <h2 style="margin:0 0 0.5rem;font-size:1.125rem;">{crate::locale::tr(|l| crate::i18n::td_string!(l, recap.resources_title))}</h2>
            <p class="subtitle" style="margin:0 0 1rem;">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, recap.resources_body))}
            </p>
            <div class="pe-community-links-list">{items}</div>
        </div>
    }
    .into_any()
}

// ---------------------------------------------------------------------------
// Local formatting helpers
// ---------------------------------------------------------------------------

/// Format a start/end pair as a date range in the reader's language. Returns
/// an empty string when the start is non-positive (unknown / unset). Call it
/// inside a reactive closure so it follows a language switch.
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

/// Format a millisecond timestamp as a date in the reader's language
/// (`utils::format_event_day`, `locale::current_date_tag()`).
fn format_event_date(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&(ms as f64).into());
    if date.get_time().is_nan() {
        return ms.to_string();
    }
    crate::utils::format_event_day(ms)
}

/// Format an ISO 8601 timestamp as a date in the reader's language (no
/// `chrono` dependency). An unparseable value is shown as sent.
fn format_iso(iso: &str) -> String {
    let parsed = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    match parsed.get_time() {
        ms if ms.is_nan() => iso.to_string(),
        ms => crate::utils::format_event_day(ms as i64),
    }
}
