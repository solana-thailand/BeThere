//! "Learn from past events" on `/events` (.plans/045 R4.4): each course (an
//! active campaign) as a card, from `GET /api/public/courses`; each opens
//! its course page (`/events/<id>`), where registered readers watch and keep
//! progress. A new episode shows up when an organizer adds the event to the
//! campaign.

use leptos::prelude::*;

use crate::i18n::{t_string, td_string, use_i18n};
use crate::locale::{fill, tr};

#[component]
pub fn Learn() -> impl IntoView {
    let i18n = use_i18n();
    let courses = LocalResource::new(super::courses_data::courses);
    let cards = move || {
        courses
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.held > 0)
            .map(|c| {
                let href = format!("/events/{}", c.id);
                let n = c.held;
                let count = move || fill(t_string!(i18n, landing.site.course_eps_n), &[("n", &n.to_string())]);
                view! {
                    <article class="lp-card lp-course">
                        <p class="lp-course-kind">{tr(|l| td_string!(l, landing.site.course_kicker))}</p>
                        <h3><a href=href.clone()>{c.title}</a></h3>
                        <p class="lp-course-about">{c.description}</p>
                        <p class="lp-course-about"><b>{count}</b></p>
                        <a class="lp-course-open" href=href>
                            {tr(|l| td_string!(l, landing.site.course_open))}
                        </a>
                    </article>
                }
            })
            .collect::<Vec<_>>()
    };
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
