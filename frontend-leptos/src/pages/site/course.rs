//! `/events/:series` (.plans/045 R4.4, the prototype's course page): a
//! course is an active campaign (`GET /api/public/courses/{id}`), its
//! episodes the campaign's public events by date, so a series that goes on
//! gets its next episode when an organizer adds the event. Register once
//! (sign in, then one click), then watch episode by episode; what you marked
//! as watched is kept by the worker (`/api/courses/{course}/…`). An episode
//! still to come is shown with its date and a link to register. The player
//! loads only when asked (youtube-nocookie).
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
use event_checkin_domain::models::course::{CourseDetail, CourseEpisode, is_course_id};

use super::doors::SitePage;

/// `GET /api/courses/{course}/progress`.
#[derive(Clone, Debug, Default, Deserialize)]
struct Progress {
    #[serde(default)]
    enrolled: bool,
    #[serde(default)]
    watched: Vec<String>,
}

/// The episode a `?ep=` (1-based, by date) asks for, else the first held one
/// not yet watched, else the first.
pub fn current_episode(
    eps: &[CourseEpisode],
    asked: Option<usize>,
    watched: &[String],
    now_ms: i64,
) -> Option<usize> {
    asked
        .and_then(|n| n.checked_sub(1))
        .filter(|i| *i < eps.len())
        .or_else(|| {
            eps.iter()
                .position(|e| !e.upcoming(now_ms) && !watched.contains(&e.slug))
        })
        .or((!eps.is_empty()).then_some(0))
}

#[component]
pub fn CoursePage() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    let slug = use_params_map().with_untracked(|p| p.get("series").unwrap_or_default());
    let id = slug.clone();
    let detail = LocalResource::new(move || {
        let id = id.clone();
        async move {
            match is_course_id(&id) {
                true => super::courses_data::course(&id).await,
                false => None,
            }
        }
    });
    let title = move || detail.get().flatten().map(|c| c.title).unwrap_or_default();
    view! {
        <Title text=title />
        <SiteFrame here=SitePage::Events auth_state=auth_state>
            {move || match detail.get() {
                None => view! { <section class="lp-course-sec"><div class="lp-wrap lp-open"></div></section> }.into_any(),
                Some(None) => view! {
                    <section class="lp-unsub">
                        <div class="lp-wrap">
                            <h1 class="lp-h2">{tr(|l| td_string!(l, landing.site.course_unknown))}</h1>
                            <p><a href="/events">{tr(|l| td_string!(l, landing.site.course_back))}</a></p>
                        </div>
                    </section>
                }
                .into_any(),
                Some(Some(course)) => view! {
                    <header class="lp-head" id="top">
                        <div class="lp-wrap">
                            <p class="lp-kicker">{tr(|l| td_string!(l, landing.site.course_kicker))}</p>
                            <h1 class="lp-head-h1"><span>{course.title.clone()}</span></h1>
                            <p class="lp-head-sub">{course.description.clone()}</p>
                        </div>
                    </header>
                    <section class="lp-course-sec">
                        <div class="lp-wrap">
                            <CourseBody slug=slug.clone() course=course auth_state=auth_state />
                        </div>
                    </section>
                }
                .into_any(),
            }}
        </SiteFrame>
    }
}

#[component]
fn CourseBody(
    slug: String,
    course: CourseDetail,
    auth_state: ReadSignal<AuthState>,
) -> impl IntoView {
    let i18n = use_i18n();
    let now_ms = js_sys::Date::now() as i64;
    let eps = StoredValue::new(course.episodes);
    let held = eps.with_value(|e| e.iter().filter(|x| !x.upcoming(now_ms)).count());
    let asked =
        use_query_map().with_untracked(|q| q.get("ep").and_then(|v| v.parse::<usize>().ok()));
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
        eps.with_value(|e| {
            current_episode(e, asked, &watched.get(), now_ms)
                .and_then(|i| e.get(i).cloned().map(|x| (i, x)))
        })
    };
    let meta = move || {
        let (from, to) = eps.with_value(|e| {
            let held: Vec<&CourseEpisode> = e.iter().filter(|x| !x.upcoming(now_ms)).collect();
            (
                held.first()
                    .map(|x| crate::utils::format_event_day(x.start_ms))
                    .unwrap_or_default(),
                held.last()
                    .map(|x| crate::utils::format_event_day(x.start_ms))
                    .unwrap_or_default(),
            )
        });
        let mut line = fill(
            t_string!(i18n, landing.site.course_meta),
            &[("n", &held.to_string()), ("from", &from), ("to", &to)],
        );
        if enrolled.get() {
            line.push_str(" · ");
            line.push_str(&fill(
                t_string!(i18n, landing.site.course_progress),
                &[
                    ("done", &watched.with(Vec::len).to_string()),
                    ("n", &held.to_string()),
                ],
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
    let mark = move |episode: String| {
        let path = format!("/courses/{}/watched", slug.get_value());
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "episode": episode });
            if crate::api::api_post_json::<serde_json::Value>(&path, &body)
                .await
                .is_ok()
            {
                watched.update(|w| {
                    if !w.contains(&episode) {
                        w.push(episode);
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
                <div class="lp-course-bar" role="progressbar" aria-valuemin="0" aria-valuemax=held aria-valuenow=done>
                    <i style=format!("width:{}%", (100 * done).checked_div(held).unwrap_or(0))></i>
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
        let Some((_, ep)) = current() else {
            return ().into_any();
        };
        let soon = ep.upcoming(now_ms);
        match (soon, ep.video.is_empty(), enrolled.get(), playing.get()) {
            (true, _, _, _) => view! {
                <div class="lp-player lp-player-lock">
                    <p>
                        {fill(t_string!(i18n, landing.site.course_soon), &[("date", &crate::utils::format_event_day(ep.start_ms))])}
                        " · "
                        <a href=format!("/e/{}", ep.slug)>{tr(|l| td_string!(l, landing.site.course_register_ep))}</a>
                    </p>
                </div>
            }
            .into_any(),
            (false, true, _, _) => view! {
                <div class="lp-player lp-player-lock"><p>{tr(|l| td_string!(l, landing.site.course_no_video))}</p></div>
            }
            .into_any(),
            (false, false, false, _) => view! {
                <div class="lp-player lp-player-lock"><p>{tr(|l| td_string!(l, landing.site.course_lock))}</p></div>
            }
            .into_any(),
            (false, false, true, false) => view! {
                <div class="lp-player">
                    <button class="lp-btn lp-btn-primary" type="button" on:click=move |_| playing.set(true)>
                        {tr(|l| td_string!(l, landing.site.course_play))}
                    </button>
                </div>
            }
            .into_any(),
            (false, false, true, true) => view! {
                <div class="lp-player">
                    <iframe
                        src=format!("https://www.youtube-nocookie.com/embed/{}?autoplay=1&rel=0", ep.video)
                        title=ep.name.clone()
                        allow="autoplay; encrypted-media; picture-in-picture"
                        allowfullscreen=true
                    ></iframe>
                </div>
            }
            .into_any(),
        }
    };
    let now_row = move || {
        let Some((i, ep)) = current() else {
            return ().into_any();
        };
        let has_next = eps.with_value(|e| e.len() > i + 1);
        let seen = watched.with(|w| w.contains(&ep.slug));
        let label = fill(
            t_string!(i18n, landing.site.course_ep),
            &[("ep", &(i + 1).to_string())],
        );
        let can_mark = enrolled.get() && !ep.upcoming(now_ms) && !ep.video.is_empty();
        let slug_for_mark = ep.slug.clone();
        view! {
            <h2 class="lp-course-now">{format!("{label}: {}", ep.name)}</h2>
            <p class="lp-course-when">{crate::utils::format_event_day(ep.start_ms)}</p>
            {enrolled.get().then(|| view! {
                <div class="lp-row">
                    {can_mark.then(|| view! {
                        <button class="lp-btn" type="button" disabled=seen on:click=move |_| mark(slug_for_mark.clone())>
                            {seen.then(|| view! { <Icon icon=IconName::Check class="icon-sm" /> })}
                            {match seen {
                                true => tr(|l| td_string!(l, landing.site.course_marked)).get(),
                                false => tr(|l| td_string!(l, landing.site.course_mark)).get(),
                            }}
                        </button>
                    })}
                    {match has_next {
                        true => view! {
                            <a class="lp-btn" href=format!("/events/{}?ep={}", slug.get_value(), i + 2)
                               on:click=move |_| playing.set(false)>
                                {tr(|l| td_string!(l, landing.site.course_next))}
                            </a>
                        }.into_any(),
                        false => view! { <span class="lp-plan-fine">{tr(|l| td_string!(l, landing.site.course_all_done))}</span> }.into_any(),
                    }}
                </div>
            })}
        }
        .into_any()
    };
    let list = move || {
        let cur = current().map(|(i, _)| i);
        eps.with_value(|all| {
            all.iter()
                .enumerate()
                .map(|(i, e)| {
                    let mark = match (
                        e.upcoming(now_ms),
                        enrolled.get(),
                        watched.with(|w| w.contains(&e.slug)),
                    ) {
                        (true, _, _) => Some(IconName::Calendar),
                        (false, false, _) => Some(IconName::Lock),
                        (false, true, true) => Some(IconName::Check),
                        (false, true, false) => None,
                    };
                    view! {
                        <li>
                            <a
                                href=format!("/events/{}?ep={}", slug.get_value(), i + 1)
                                aria-current=(cur == Some(i)).then_some("true")
                                on:click=move |_| playing.set(false)
                            >
                                <span class="lp-ep-n">{format!("{:02}", i + 1)}</span>
                                <span class="lp-ep-name">{e.name.clone()}<small>{crate::utils::format_event_day(e.start_ms)}</small></span>
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
