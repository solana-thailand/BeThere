//! How it works as a swimlane (.plans/043 L4, ASKS-4 §13): columns are the
//! four steps, rows the routes (baht LIVE, USDC on Solana DEVNET, an AI agent
//! DEVNET). Unticking a route fades its row; at least one stays on. Phones
//! show one route at a time behind tabs. `?track=sol|ai` opens on that route.
//!
//! Labels say only what ships (build plan rule 2): the agent's claim back is
//! `bethere-mcp`'s `claim_refund`, on devnet. The baht timings are medians
//! from `GET /api/public/stats` with their sample size; with too few cases,
//! or no stats, the cell says nothing about time.

use leptos::prelude::*;

use crate::i18n::{Locale, td_string, use_i18n};

use super::hero::Markup;
use super::stats::{duration_label, measured_at_label, use_landing_stats};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Thb,
    Sol,
    Ai,
}

impl Route {
    pub const ALL: [Route; 3] = [Route::Thb, Route::Sol, Route::Ai];

    fn index(self) -> usize {
        match self {
            Route::Thb => 0,
            Route::Sol => 1,
            Route::Ai => 2,
        }
    }

    fn class(self) -> &'static str {
        match self {
            Route::Thb => "lp-lane lp-lane-thb",
            Route::Sol => "lp-lane lp-lane-sol",
            Route::Ai => "lp-lane lp-lane-ai",
        }
    }

    /// `?track=` value → the route it opens on alone.
    pub fn from_track(value: &str) -> Option<Route> {
        match value {
            "sol" => Some(Route::Sol),
            "ai" => Some(Route::Ai),
            _ => None,
        }
    }
}

/// Flip one route; refuses to turn off the last one on.
pub fn toggle_route(on: [bool; 3], route: Route) -> [bool; 3] {
    let i = route.index();
    let mut next = on;
    next[i] = !on[i];
    match next.iter().any(|&v| v) {
        true => next,
        false => on,
    }
}

/// The honest label on a route (build plan rule 2).
#[derive(Clone, Copy)]
enum Status {
    Live,
    Devnet,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Live => "LIVE",
            Status::Devnet => "DEVNET",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Status::Live => "lp-tag lp-live",
            Status::Devnet => "lp-tag lp-devnet",
        }
    }
}

#[derive(Clone, Copy)]
enum Role {
    You,
    Organizer,
    Program,
    Agent,
}

/// A catalog string, as `crate::locale::tr` takes it.
type Catalog = fn(Locale) -> &'static str;

#[derive(Clone, Copy)]
enum Note {
    Text(Catalog),
    SlipMedian,
    RefundMedian,
}

#[derive(Clone, Copy)]
struct Cell {
    role: Role,
    text: Catalog,
    note: Option<Note>,
}

fn cells(route: Route) -> [Cell; 4] {
    let c = |role, text, note| Cell { role, text, note };
    match route {
        Route::Thb => [
            c(
                Role::You,
                |l| td_string!(l, landing.how.thb_1),
                Some(Note::SlipMedian),
            ),
            c(
                Role::Organizer,
                |l| td_string!(l, landing.how.thb_2),
                Some(Note::Text(|l| td_string!(l, landing.how.one_scan))),
            ),
            c(
                Role::Organizer,
                |l| td_string!(l, landing.how.thb_3),
                Some(Note::RefundMedian),
            ),
            c(Role::You, |l| td_string!(l, landing.how.thb_4), None),
        ],
        Route::Sol => [
            c(
                Role::You,
                |l| td_string!(l, landing.how.sol_1),
                Some(Note::Text(|l| td_string!(l, landing.how.seconds))),
            ),
            c(
                Role::Organizer,
                |l| td_string!(l, landing.how.sol_2),
                Some(Note::Text(|l| td_string!(l, landing.how.one_scan))),
            ),
            c(
                Role::You,
                |l| td_string!(l, landing.how.sol_3),
                Some(Note::Text(|l| td_string!(l, landing.how.after_end))),
            ),
            c(Role::Program, |l| td_string!(l, landing.how.sol_4), None),
        ],
        Route::Ai => [
            c(
                Role::Agent,
                |l| td_string!(l, landing.how.ai_1),
                Some(Note::Text(|l| td_string!(l, landing.how.seconds))),
            ),
            c(
                Role::You,
                |l| td_string!(l, landing.how.ai_2),
                Some(Note::Text(|l| td_string!(l, landing.how.one_scan))),
            ),
            c(
                Role::Agent,
                |l| td_string!(l, landing.how.ai_3),
                Some(Note::Text(|l| td_string!(l, landing.how.after_end))),
            ),
            c(Role::Program, |l| td_string!(l, landing.how.ai_4), None),
        ],
    }
}

const STEPS: [Catalog; 4] = [
    |l| td_string!(l, landing.how.step_1),
    |l| td_string!(l, landing.how.step_2),
    |l| td_string!(l, landing.how.step_3),
    |l| td_string!(l, landing.how.step_4),
];

fn role_tag(role: Role) -> impl IntoView {
    let (class, label): (&str, Catalog) = match role {
        Role::You => ("lp-r lp-r-you", |l| td_string!(l, landing.how.role_you)),
        Role::Organizer => ("lp-r lp-r-org", |l| td_string!(l, landing.how.role_org)),
        Role::Program => ("lp-r lp-r-prog", |l| td_string!(l, landing.how.role_prog)),
        Role::Agent => ("lp-r lp-r-agent", |l| td_string!(l, landing.how.role_agent)),
    };
    view! { <b class=class>{crate::locale::tr(label)}</b> }
}

fn route_head(route: Route) -> (Catalog, Catalog, Status) {
    match route {
        Route::Thb => (
            |l| td_string!(l, landing.how.thb_name),
            |l| td_string!(l, landing.how.live_today),
            Status::Live,
        ),
        Route::Sol => (
            |l| td_string!(l, landing.how.sol_name),
            |l| td_string!(l, landing.how.coming),
            Status::Devnet,
        ),
        Route::Ai => (
            |l| td_string!(l, landing.how.ai_name),
            |l| td_string!(l, landing.how.coming),
            Status::Devnet,
        ),
    }
}

fn route_tab(route: Route) -> Catalog {
    match route {
        Route::Thb => |l| td_string!(l, landing.how.tab_thb),
        Route::Sol => |l| td_string!(l, landing.how.tab_sol),
        Route::Ai => |l| td_string!(l, landing.how.tab_ai),
    }
}

#[component]
pub fn HowItWorks(
    /// Rendered at the foot of the section (the try line on /organizers), so
    /// it sits on the section's own background.
    #[prop(optional)]
    footer: Option<AnyView>,
) -> impl IntoView {
    let i18n = use_i18n();
    let stats = use_landing_stats();
    let track = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|q| web_sys::UrlSearchParams::new_with_str(&q).ok())
        .and_then(|p| p.get("track"))
        .and_then(|v| Route::from_track(&v));
    let on = RwSignal::new(match track {
        Some(r) => Route::ALL.map(|x| x == r),
        None => [true; 3],
    });
    let shown = RwSignal::new(track.unwrap_or(Route::Thb));

    let note_view = move |note: Note| -> AnyView {
        match note {
            Note::Text(t) => view! { <em>{crate::locale::tr(t)}</em> }.into_any(),
            Note::SlipMedian | Note::RefundMedian => {
                let text = move || {
                    let s = stats.get()?;
                    let locale = i18n.get_locale();
                    let (timing, template) = match note {
                        Note::SlipMedian => (s.slip_check?, td_string!(locale, landing.how.median)),
                        _ => (
                            s.refund_after_end?,
                            td_string!(locale, landing.how.median_after),
                        ),
                    };
                    Some(crate::locale::fill(
                        template,
                        &[("d", &duration_label(timing.median_minutes, locale))],
                    ))
                };
                view! { {move || text().map(|t| view! { <em>{t}</em> })} }.into_any()
            }
        }
    };

    let lane = move |route: Route| {
        let (name, sub, tag) = route_head(route);
        let class = move || {
            let mut c = route.class().to_string();
            if !on.get()[route.index()] {
                c.push_str(" lp-off");
            }
            if shown.get() == route {
                c.push_str(" lp-shown");
            }
            c
        };
        view! {
            <div class=class>
                <button
                    type="button"
                    class="lp-lane-toggle"
                    aria-pressed=move || on.get()[route.index()].to_string()
                    on:click=move |_| on.update(|v| *v = toggle_route(*v, route))
                >
                    <i class="lp-box" aria-hidden="true"></i>
                    <span class="lp-ln">
                        <b>{crate::locale::tr(name)}</b>
                        <small>{crate::locale::tr(sub)}</small>
                    </span>
                    <span class=tag.class()>{tag.label()}</span>
                </button>
                {cells(route).into_iter().zip(STEPS).map(|(cell, step)| view! {
                    <div class="lp-cell">
                        <span class="lp-stp">{crate::locale::tr(step)}</span>
                        <p>
                            <span>{role_tag(cell.role)}{crate::locale::tr(cell.text)}</span>
                            {cell.note.map(note_view)}
                        </p>
                    </div>
                }).collect::<Vec<_>>()}
            </div>
        }
    };

    // Name only the timings actually published, with their sample sizes.
    let footnote = move || {
        let locale = i18n.get_locale();
        let devnet = td_string!(locale, landing.how.devnet_note);
        let Some(s) = stats.get() else {
            return devnet.to_string();
        };
        let n = |t: Option<event_checkin_domain::models::public_stats::Timing>,
                 key: &'static str| {
            t.map(|t| crate::locale::fill(key, &[("n", &t.samples.to_string())]))
        };
        let parts: Vec<String> = [
            n(s.slip_check, td_string!(locale, landing.how.part_slips)),
            n(
                s.refund_after_end,
                td_string!(locale, landing.how.part_refunds),
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        match parts.is_empty() {
            true => devnet.to_string(),
            false => {
                let measured = crate::locale::fill(
                    td_string!(locale, landing.how.measured),
                    &[
                        ("parts", &parts.join(", ")),
                        ("at", &measured_at_label(&s.measured_at)),
                    ],
                );
                format!("{measured} · {devnet}")
            }
        }
    };

    view! {
        <section id="how" class="lp-how">
            <div class="lp-wrap">
                <h2 class="lp-h2">{crate::locale::tr(|l| td_string!(l, landing.how.title))}</h2>
                <p class="lp-lede"><Markup text=crate::locale::tr(|l| td_string!(l, landing.how.lede)) /></p>
                <div class="lp-lane-tabs" role="tablist">
                    {Route::ALL.into_iter().map(|route| view! {
                        <button
                            type="button"
                            role="tab"
                            aria-selected=move || (shown.get() == route).to_string()
                            on:click=move |_| shown.set(route)
                        >
                            {crate::locale::tr(route_tab(route))}
                        </button>
                    }).collect::<Vec<_>>()}
                </div>
                <div class="lp-lanes">
                    <div class="lp-shead lp-corner"></div>
                    {STEPS.into_iter().enumerate().map(|(i, step)| {
                        let n = i + 1;
                        view! {
                        <div class="lp-shead">
                            <span class="lp-n">{format!("{n:02}")}</span>
                            <h3>{crate::locale::tr(step)}</h3>
                        </div>
                        }
                    }).collect::<Vec<_>>()}
                    {Route::ALL.into_iter().map(lane).collect::<Vec<_>>()}
                </div>
                <p class="lp-fineprint">{footnote}</p>
                <UsdcPanel />
                {footer}
            </div>
        </section>
    }
}

/// The thesis the panel quotes (ASKS-4 §11, the prototype's link).
const SOLANA_AI_THESIS: &str = "https://solana.com/news/solana-ai-the-democratization-layer";

/// "Why USDC on Solana": where the product is going, labelled DEVNET because
/// that is where the escrow runs. The prototype's sandbox block waits for
/// build plan 0.4.
#[component]
fn UsdcPanel() -> impl IntoView {
    let tr = crate::locale::tr;
    view! {
        <div class="lp-card lp-usdc" id="usdc">
            <div>
                <p class="lp-kicker">{tr(|l| td_string!(l, landing.how.usdc_kicker))}" "<span class="lp-tag lp-devnet">"DEVNET"</span></p>
                <h3>{tr(|l| td_string!(l, landing.how.usdc_title))}</h3>
                <p class="lp-ref">
                    {tr(|l| td_string!(l, landing.how.usdc_ref))}" "
                    <a class="lp-kw" href=SOLANA_AI_THESIS target="_blank" rel="noopener noreferrer">
                        {tr(|l| td_string!(l, landing.how.usdc_ref_link))}" ↗"
                    </a>
                </p>
            </div>
            <ol>
                <li>
                    <b>{tr(|l| td_string!(l, landing.how.usdc_1_t))}</b>
                    <span>{tr(|l| td_string!(l, landing.how.usdc_1_d))}</span>
                </li>
                <li>
                    <b>{tr(|l| td_string!(l, landing.how.usdc_2_t))}</b>
                    <span><Markup text=tr(|l| td_string!(l, landing.how.usdc_2_d)) /></span>
                </li>
            </ol>
        </div>
    }
}
