//! "Learn from past events" on `/events` (.plans/045 R4.3, prototype
//! events `#learn`): each series is a course, its recorded episodes in order
//! (`domain::models::catalogue`); each card and episode opens the course page
//! (`/events/<slug>`, R4.4), where registered readers watch and keep progress.

use leptos::prelude::*;

use crate::i18n::{Locale, t_string, td_string, use_i18n};
use crate::locale::{fill, tr};
use event_checkin_domain::models::catalogue::{Series, course_episodes};

pub(super) type Catalog = fn(Locale) -> &'static str;

/// A course's title, kind and one line.
pub(super) fn course_copy(series: Series) -> (Catalog, Catalog, Catalog) {
    match series {
        Series::RoadToMainnet => (
            |_| "Road to Mainnet",
            |l| td_string!(l, landing.site.course_rtm_kind),
            |l| td_string!(l, landing.site.course_rtm_about),
        ),
        Series::LatentSpace => (
            |_| "Solana in Latent Space",
            |l| td_string!(l, landing.site.course_latent_kind),
            |l| td_string!(l, landing.site.course_latent_about),
        ),
        Series::Single => (|_| "", |_| "", |_| ""),
    }
}

#[component]
pub fn Learn() -> impl IntoView {
    let i18n = use_i18n();
    let cards = Series::COURSES
        .into_iter()
        .map(|series| {
            let eps = course_episodes(series);
            let slug = series.course_slug().unwrap_or_default();
            let n = eps.len();
            let (title, kind, about) = course_copy(series);
            let rows = eps
                .into_iter()
                .map(|e| {
                    let line = move || {
                        fill(
                            t_string!(i18n, landing.site.course_ep),
                            &[
                                ("ep", &e.ep.to_string()),
                                ("date", &crate::utils::format_event_day(e.start_ms)),
                            ],
                        )
                    };
                    view! {
                        <li>
                            <a href=format!("/events/{slug}?ep={}", e.ep) title=e.name>{line}</a>
                        </li>
                    }
                })
                .collect::<Vec<_>>();
            let count = move || {
                fill(
                    t_string!(i18n, landing.site.course_eps),
                    &[("n", &n.to_string())],
                )
            };
            view! {
                <article class="lp-card lp-course">
                    <p class="lp-course-kind">{tr(kind)}</p>
                    <h3><a href=format!("/events/{slug}")>{tr(title)}</a></h3>
                    <p class="lp-course-about">{tr(about)}</p>
                    <details>
                        <summary>
                            <b>{count}</b>
                            " · "
                            {tr(|l| td_string!(l, landing.site.course_watch))}
                        </summary>
                        <ol class="lp-course-eps">{rows}</ol>
                    </details>
                    <a class="lp-course-open" href=format!("/events/{slug}")>
                        {tr(|l| td_string!(l, landing.site.course_open))}
                    </a>
                </article>
            }
        })
        .collect::<Vec<_>>();
    view! {
        <section id="learn" class="lp-learn">
            <div class="lp-wrap">
                <h2 class="lp-h2">{tr(|l| td_string!(l, landing.site.learn_title))}</h2>
                <p class="lp-lede">{tr(|l| td_string!(l, landing.site.learn_lede))}</p>
                <div class="lp-courses">{cards}</div>
            </div>
        </section>
    }
}
