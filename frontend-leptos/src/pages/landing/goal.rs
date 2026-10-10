//! The goal (.plans/043 L10, ASKS-4 §17): BeThere's events against every
//! Solana event of the last 12 months, beside a globe of where those events
//! were. The numerator is live (`events_held` from `GET /api/public/stats`,
//! build plan rule 1); the denominator and the dots are a measured snapshot,
//! `globe/globe-data.json` (scripts/globe_data_import.py), labelled with its
//! date and source.
//!
//! Neither the data (~74 KB) nor the globe code is on the first load. When the
//! section first comes into view this fetches the JSON once, reads the two
//! numbers it needs, then injects `globe/globe.js` and hands it the same text.
//! Nothing at all is drawn for the goal bar until both numbers are in.

use leptos::html::Div;
use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen::JsValue;

use crate::i18n::{Locale, td_string};
use crate::locale::tr;

use super::sofar::watch_first_sight;
use super::stats::{group_thousands, use_landing_stats};

pub const GLOBE_DATA_URL: &str = "/globe/globe-data.json";
/// Bump `v` whenever `globe/globe.js` changes: the file is not content-hashed,
/// and the service worker keeps whatever URL it saw (stale-while-revalidate).
pub const GLOBE_JS_URL: &str = "/globe/globe.js?v=1";
/// Below this the bar would be invisible; the label carries the real figure.
const MIN_BAR_PERCENT: f64 = 1.5;

/// The part of `globe-data.json` the page itself reads; the rest is the
/// globe's.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GlobeSummary {
    /// `YYYY-MM-DD`, when the calendars were read.
    pub measured_at: String,
    /// Solana events in the window: a floor (other calendars exist), hence "+".
    pub solana_events: u32,
}

/// The bar's width in percent: `ours` of `all`, never thinner than
/// [`MIN_BAR_PERCENT`] and never past full.
pub fn goal_bar_percent(ours: u32, all: u32) -> f64 {
    match all {
        0 => 0.0,
        _ => (100.0 * f64::from(ours) / f64::from(all)).clamp(MIN_BAR_PERCENT, 100.0),
    }
}

/// Load globe.js if it is not already loaded, then mount it on `root`.
fn mount_globe(root: web_sys::HtmlElement, data: String) {
    crate::utils::lazy_script::with_mount(GLOBE_JS_URL, "bethereGlobe", move |mount| {
        if let Err(e) = mount.call2(&JsValue::NULL, &root, &JsValue::from_str(&data)) {
            log::warn!("[landing] globe mount: {e:?}");
        }
    });
}

async fn fetch_globe_data() -> Option<String> {
    let resp = crate::api::fetch::get(GLOBE_DATA_URL, &[]).await.ok()?;
    match resp.status() {
        200 => crate::api::fetch::response_text(&resp).await.ok(),
        status => {
            log::warn!("[landing] globe data returned {status}");
            None
        }
    }
}

fn lang_attr(locale: Locale) -> &'static str {
    match locale {
        Locale::th => "th",
        Locale::en => "en",
    }
}

#[component]
pub fn Goal() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let stats = use_landing_stats();
    let wrap = NodeRef::<Div>::new();
    let root = NodeRef::<Div>::new();
    let seen = RwSignal::new(false);
    let summary = RwSignal::new(None::<GlobeSummary>);
    let started = StoredValue::new(false);
    watch_first_sight(wrap, seen);

    Effect::new(move |_| {
        if !seen.get() || started.get_value() {
            return;
        }
        let Some(el) = root.get_untracked() else {
            return;
        };
        started.set_value(true);
        let el: web_sys::HtmlElement = el.into();
        leptos::task::spawn_local(async move {
            let Some(text) = fetch_globe_data().await else {
                return;
            };
            match serde_json::from_str::<GlobeSummary>(&text) {
                Ok(s) => summary.set(Some(s)),
                Err(e) => {
                    log::warn!("[landing] globe data parse: {e}");
                    return;
                }
            }
            mount_globe(el, text);
        });
    });

    let ours = move || stats.with(|s| s.as_ref().map(|s| s.events_held));
    let bar = move || {
        let (Some(ours), Some(all)) = (
            ours(),
            summary.with(|s| s.as_ref().map(|s| s.solana_events)),
        ) else {
            return None;
        };
        let source = summary.with(|s| {
            s.as_ref().map(|s| {
                let ms = js_sys::Date::parse(&s.measured_at);
                let at = match ms.is_nan() {
                    true => s.measured_at.clone(),
                    false => crate::utils::format_event_day(ms as i64),
                };
                crate::locale::fill(
                    td_string!(i18n.get_locale(), landing.goal.source),
                    &[("at", &at)],
                )
            })
        });
        let width = format!("{:.1}%", goal_bar_percent(ours, all));
        Some(view! {
            <div class="lp-bar" role="progressbar" aria-valuemin="0"
                aria-valuemax=all.to_string() aria-valuenow=ours.to_string()
                aria-label=tr(|l| td_string!(l, landing.goal.bar_aria))>
                <span style:width=width></span>
            </div>
            <p class="lp-barlabel">
                <strong>{group_thousands(u64::from(ours))}</strong>
                " / "{group_thousands(u64::from(all))}"+ "
                {tr(|l| td_string!(l, landing.goal.all_label))}
            </p>
            <div class="lp-bar lp-bar-next" aria-hidden="true"><span></span></div>
            <p class="lp-barlabel">{tr(|l| td_string!(l, landing.goal.next))}</p>
            <p class="lp-fineprint">{source}</p>
        })
    };

    view! {
        <div class="lp-wrap lp-goal-grid" node_ref=wrap>
            <div class="lp-globe-col" node_ref=root
                data-lang=move || lang_attr(i18n.get_locale())
                data-ours=move || ours().map(|n| n.to_string()).unwrap_or_default()>
                <canvas class="lp-globe" data-globe="canvas" width="440" height="440" role="img"
                    aria-label=tr(|l| td_string!(l, landing.goal.globe_aria))></canvas>
                <p class="lp-globe-hint">{tr(|l| td_string!(l, landing.goal.hint))}</p>
                <input class="lp-globe-search" data-globe="search" type="search"
                    list="lp-countries" autocomplete="off"
                    aria-label=tr(|l| td_string!(l, landing.goal.search_aria))
                    placeholder=tr(|l| td_string!(l, landing.goal.placeholder)) />
                <datalist id="lp-countries" data-globe="countries"></datalist>
                <div class="lp-globe-panel" data-globe="panel" aria-live="polite" hidden></div>
            </div>
            <div class="lp-goal-text">
                <p class="lp-kicker">{tr(|l| td_string!(l, landing.goal.kicker))}</p>
                <h2>{tr(|l| td_string!(l, landing.goal.title))}</h2>
                {bar}
                <div class="lp-communities">
                    <p class="lp-kicker">{tr(|l| td_string!(l, landing.sofar.communities))}</p>
                    <div class="lp-slots">
                        <span class="lp-slot">"Solana Developer Thailand"</span>
                        <a class="lp-slot lp-slot-open" href="/organizers#join">{tr(|l| td_string!(l, landing.sofar.your_community))}</a>
                    </div>
                </div>
            </div>
        </div>
    }
}
