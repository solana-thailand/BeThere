//! `/events/:series` (.plans/045 R4.4, the prototype's course page): a past
//! series as a course. Register once (sign in, then one click), then watch
//! episode by episode; what you marked as watched is kept by the worker
//! (`/api/courses/{course}/…`). The player loads only when asked
//! (youtube-nocookie), so the page fetches nothing from YouTube until then.
//! `.issues/068`: watching is never check-in, attendance, a deposit or a badge.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::{use_params_map, use_query_map};
use serde::Deserialize;

use crate::i18n::{t_string, td_string, use_i18n};
use crate::icons::{Icon, IconName};
use crate::locale::{fill, tr};
use crate::pages::landing::AuthState;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};
use event_checkin_domain::models::catalogue::{PastEvent, Series, course_episodes};

use super::doors::SitePage;
use super::head::PageHead;
use super::learn::course_copy;

/// `GET /api/courses/{course}/progress`.
#[derive(Clone, Debug, Default, Deserialize)]
struct Progress {
    #[serde(default)]
    enrolled: bool,
    #[serde(default)]
    watched: Vec<String>,
}

/// The episode a `?ep=` asks for, else the first not yet watched, else the first.
pub fn current_episode(eps: &[PastEvent], asked: Option<u32>, watched: &[String]) -> Option<usize> {
    asked
        .and_then(|n| eps.iter().position(|e| e.ep == n))
        .or_else(|| {
            eps.iter()
                .position(|e| !watched.iter().any(|w| w == e.slug))
        })
        .or((!eps.is_empty()).then_some(0))
}

#[component]
pub fn CoursePage() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    let slug = use_params_map().with_untracked(|p| p.get("series").unwrap_or_default());
    let Some(series) = Series::from_course_slug(&slug) else {
        return view! {
            <Title text=tr(|l| td_string!(l, landing.site.title_events)) />
            <SiteFrame here=SitePage::Events auth_state=auth_state>
                <section class="lp-unsub">
                    <div class="lp-wrap">
                        <h1 class="lp-h2">{tr(|l| td_string!(l, landing.site.course_unknown))}</h1>
                        <p><a href="/events">{tr(|l| td_string!(l, landing.site.course_back))}</a></p>
                    </div>
                </section>
            </SiteFrame>
        }
        .into_any();
    };
    let (title, kind, about) = course_copy(series);
    view! {
        <Title text=tr(title) />
        <SiteFrame here=SitePage::Events auth_state=auth_state>
            <PageHead kicker=kind title=title sub=about />
            <section class="lp-course-sec">
                <div class="lp-wrap">
                    <CourseBody slug=slug series=series auth_state=auth_state />
                </div>
            </section>
        </SiteFrame>
    }
    .into_any()
}

#[component]
fn CourseBody(slug: String, series: Series, auth_state: ReadSignal<AuthState>) -> impl IntoView {
    let i18n = use_i18n();
    let eps = StoredValue::new(course_episodes(series));
    let n = eps.with_value(Vec::len);
    let asked = use_query_map().with_untracked(|q| q.get("ep").and_then(|v| v.parse::<u32>().ok()));
    let enrolled = RwSignal::new(false);
    let watched = RwSignal::new(Vec::<String>::new());
    let playing = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let slug = StoredValue::new(slug);

    // Progress once signed in (nothing when signed out: the API is authed).
    Effect::new(move |_| {
        if !matches!(auth_state.get(), AuthState::SignedIn(_)) {
            return;
        }
        let path = format!("/courses/{}/progress", slug.get_value());
        leptos::task::spawn_local(async move {
            if let Some(p) = crate::api::api_get_json_if_signed_in::<Progress>(&path).await {
                enrolled.set(p.enrolled);
                watched.set(p.watched);
            }
        });
    });

    let current = move || {
        eps.with_value(|e| current_episode(e, asked, &watched.get()))
            .and_then(|i| eps.with_value(|e| e.get(i).copied()))
    };
    let meta = move || {
        let (from, to) = eps.with_value(|e| {
            (
                e.first()
                    .map(|x| crate::utils::format_event_day(x.start_ms))
                    .unwrap_or_default(),
                e.last()
                    .map(|x| crate::utils::format_event_day(x.start_ms))
                    .unwrap_or_default(),
            )
        });
        let mut line = fill(
            t_string!(i18n, landing.site.course_meta),
            &[("n", &n.to_string()), ("from", &from), ("to", &to)],
        );
        if enrolled.get() {
            let done = watched.with(Vec::len);
            line.push_str(" · ");
            line.push_str(&fill(
                t_string!(i18n, landing.site.course_progress),
                &[("done", &done.to_string()), ("n", &n.to_string())],
            ));
        }
        line
    };
    let register = move |_| {
        busy.set(true);
        let path = format!("/courses/{}/enrol", slug.get_value());
        leptos::task::spawn_local(async move {
            if crate::api::api_post_json::<serde_json::Value>(&path, &serde_json::json!({}))
                .await
                .is_ok()
            {
                enrolled.set(true);
            }
            busy.set(false);
        });
    };
    let mark = move |episode: &'static str| {
        let path = format!("/courses/{}/watched", slug.get_value());
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "episode": episode });
            if crate::api::api_post_json::<serde_json::Value>(&path, &body)
                .await
                .is_ok()
            {
                watched.update(|w| {
                    if !w.iter().any(|x| x == episode) {
                        w.push(episode.to_string());
                    }
                });
            }
        });
    };
    let signin_href = move || format!("/login?next=/events/{}", slug.get_value());

    let action = move || {
        match (auth_state.get(), enrolled.get()) {
        (_, true) => {
            let done = watched.with(Vec::len);
            view! {
                <div class="lp-course-bar" role="progressbar" aria-valuemin="0" aria-valuemax=n aria-valuenow=done>
                    <i style=format!("width:{}%", (100 * done).checked_div(n).unwrap_or(0))></i>
                </div>
            }
            .into_any()
        }
        (AuthState::SignedIn(_), false) => view! {
            <div class="lp-actions">
                <button class="lp-btn lp-btn-primary" type="button" disabled=move || busy.get() on:click=register>
                    {tr(|l| td_string!(l, landing.site.course_register))}
                </button>
            </div>
        }
        .into_any(),
        _ => view! {
            <div class="lp-actions">
                <a class="lp-btn lp-btn-primary" href=signin_href>{tr(|l| td_string!(l, landing.site.course_signin))}</a>
            </div>
        }
        .into_any(),
    }
    };

    let player = move || {
        let Some(ep) = current() else {
            return ().into_any();
        };
        match (enrolled.get(), playing.get()) {
            (false, _) => view! {
                <div class="lp-player lp-player-lock"><p>{tr(|l| td_string!(l, landing.site.course_lock))}</p></div>
            }
            .into_any(),
            (true, false) => view! {
                <div class="lp-player">
                    <button class="lp-btn lp-btn-primary" type="button" on:click=move |_| playing.set(true)>
                        {tr(|l| td_string!(l, landing.site.course_play))}
                    </button>
                </div>
            }
            .into_any(),
            (true, true) => view! {
                <div class="lp-player">
                    <iframe
                        src=format!("https://www.youtube-nocookie.com/embed/{}?autoplay=1&rel=0", ep.video)
                        title=ep.name
                        allow="autoplay; encrypted-media; picture-in-picture"
                        allowfullscreen=true
                    ></iframe>
                </div>
            }
            .into_any(),
        }
    };
    let now_row = move || {
        let Some(ep) = current() else {
            return ().into_any();
        };
        let next = eps.with_value(|e| e.iter().find(|x| x.ep == ep.ep + 1).copied());
        let seen = watched.with(|w| w.iter().any(|x| x == ep.slug));
        let label = fill(
            t_string!(i18n, landing.site.course_ep),
            &[("ep", &ep.ep.to_string())],
        );
        view! {
            <h2 class="lp-course-now">{format!("{label}: {}", ep.name)}</h2>
            <p class="lp-course-when">{crate::utils::format_event_day(ep.start_ms)}</p>
            {enrolled.get().then(|| view! {
                <div class="lp-row">
                    <button class="lp-btn" type="button" disabled=seen on:click=move |_| mark(ep.slug)>
                        {seen.then(|| view! { <Icon icon=IconName::Check class="icon-sm" /> })}
                        {move || match seen {
                            true => tr(|l| td_string!(l, landing.site.course_marked)).get(),
                            false => tr(|l| td_string!(l, landing.site.course_mark)).get(),
                        }}
                    </button>
                    {match next {
                        Some(nx) => view! {
                            <a class="lp-btn" href=format!("/events/{}?ep={}", slug.get_value(), nx.ep)
                               on:click=move |_| playing.set(false)>
                                {tr(|l| td_string!(l, landing.site.course_next))}
                            </a>
                        }.into_any(),
                        None => view! { <span class="lp-plan-fine">{tr(|l| td_string!(l, landing.site.course_all_done))}</span> }.into_any(),
                    }}
                </div>
            })}
        }
        .into_any()
    };
    let list = move || {
        let cur = current().map(|e| e.ep);
        eps.with_value(|all| {
            all.iter()
                .map(|e| {
                    let mark = match (enrolled.get(), watched.with(|w| w.iter().any(|x| x == e.slug))) {
                        (false, _) => Some(IconName::Lock),
                        (true, true) => Some(IconName::Check),
                        (true, false) => None,
                    };
                    let ep = e.ep;
                    view! {
                        <li>
                            <a
                                href=format!("/events/{}?ep={ep}", slug.get_value())
                                aria-current=(cur == Some(ep)).then_some("true")
                                on:click=move |_| playing.set(false)
                            >
                                <span class="lp-ep-n">{format!("{ep:02}")}</span>
                                <span class="lp-ep-name">{e.name}<small>{crate::utils::format_event_day(e.start_ms)}</small></span>
                                <span class="lp-ep-st">{mark.map(|icon| view! { <Icon icon=icon class="icon-sm" /> })}</span>
                            </a>
                        </li>
                    }
                })
                .collect::<Vec<_>>()
        })
    };

    view! {
        <p class="lp-course-meta">{meta}</p>
        {action}
        <div class="lp-course-body">
            <div class="lp-course-main">{player}{now_row}</div>
            <ol class="lp-course-list">{list}</ol>
        </div>
    }
}
