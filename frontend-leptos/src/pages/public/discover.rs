//! `/discover` — what is on, and what I am part of (`.issues/096`).
//!
//! The landing page opens with a pitch: hero, three stat tiles, "how it works",
//! an FAQ. That is the right page for someone who has never heard of BeThere and
//! the wrong one for everybody else. A person who has already registered arrives
//! wanting one of three things — when is the next event, where is my ticket, what
//! did I go to — and all three are below the fold behind an explanation they have
//! already read.
//!
//! This page answers those and nothing else. Two lists, dates on the left, one
//! tap per row.
//!
//! Splitting "mine" on `event_end_ms` rather than `event_start_ms` is deliberate:
//! an event that is running right now belongs under the heading that says it is
//! happening, not the one that says it already happened.

use leptos::prelude::*;
use serde::Deserialize;

use crate::pages::landing::{AuthState, SiteHeader};

#[derive(Clone, Deserialize)]
struct PublicEventItem {
    name: String,
    slug: String,
    event_start_ms: i64,
    #[serde(default)]
    time_tba: bool,
    #[serde(default)]
    location: String,
    #[serde(default)]
    nft_image_url: String,
    #[serde(default)]
    poster_url: String,
    /// Non-empty = postponed (migration 0053).
    #[serde(default)]
    postponed_note: String,
}

#[derive(Clone, Deserialize, Default)]
struct PublicEventsResponse {
    events: Vec<PublicEventItem>,
}

#[derive(Clone, Deserialize)]
struct MyRegistration {
    event_name: String,
    event_slug: String,
    event_start_ms: i64,
    #[serde(default)]
    event_end_ms: i64,
    #[serde(default)]
    poster_url: String,
    #[serde(default)]
    nft_image_url: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    time_tba: bool,
    status: String,
    /// Where this person's own journey continues — their ticket, their claim,
    /// their outstanding deposit.
    ///
    /// The rows used to link to `/e/{slug}`, the **public** event page, which
    /// tells someone holding a ticket that the event has ended. The landing
    /// page's own list has always used this field; `/discover` did not
    /// (`.issues/109`).
    #[serde(default)]
    next_step: NextStep,
}

#[derive(Clone, Default, Deserialize)]
struct NextStep {
    #[serde(default)]
    url: String,
}

/// One row, whichever list it came from.
#[derive(Clone)]
struct Row {
    title: String,
    href: String,
    start_ms: i64,
    time_tba: bool,
    location: String,
    image: String,
    /// Right-hand pill: registration status for my events, nothing for public.
    status: Option<String>,
    past: bool,
}

/// Poster first, badge second — the fallback `event_hero` documents.
fn pick_image(poster: &str, badge: &str) -> String {
    match poster.is_empty() {
        false => poster.to_string(),
        // The generic badge SVG is not a picture of anything. Better an empty
        // slot than four identical thumbnails down the page.
        true => match badge.contains("badge-hd.svg") {
            true => String::new(),
            false => badge.to_string(),
        },
    }
}

#[component]
#[allow(non_snake_case)]
pub fn Discover() -> impl IntoView {
    let (upcoming, set_upcoming) = signal(Vec::<Row>::new());
    let (mine_now, set_mine_now) = signal(Vec::<Row>::new());
    let (mine_past, set_mine_past) = signal(Vec::<Row>::new());
    let (loaded, set_loaded) = signal(false);
    // Drives the sign-in prompt. A signed-out visitor sees only the public list
    // and has no way to know the page has two more sections for them.
    let (signed_in, set_signed_in) = signal(false);
    // Fed to the shared header. The page already learns whether there is a
    // session from `/my-registrations`, so it does not need a second call.
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (user_role, set_user_role) = signal(String::new());

    leptos::task::spawn_local(async move {
        let now_ms = js_sys::Date::now() as i64;

        if let Ok(page) = crate::api::api_get_json::<PublicEventsResponse>("/public/events").await {
            set_upcoming.set(
                page.events
                    .into_iter()
                    .map(|e| Row {
                        title: e.name,
                        href: format!("/e/{}", e.slug),
                        start_ms: e.event_start_ms,
                        time_tba: e.time_tba,
                        location: e.location,
                        image: pick_image(&e.poster_url, &e.nft_image_url),
                        // Public rows have no registration status, so the
                        // pill is free to flag a postponed event.
                        status: (!e.postponed_note.trim().is_empty())
                            .then(|| "Postponed".to_string()),
                        past: false,
                    })
                    .collect(),
            );
        }

        // Signed out is a normal state here, and the API layer disagrees:
        // every helper in `api/mod.rs` calls `redirect_to_login_expired()` on a
        // 401, so `api_get_json` bounced a logged-out visitor to /login before
        // this function could treat the error as "no personal sections". The
        // low-level `fetch::get` does not redirect — the same escape the
        // landing page's own registrations list uses (`.issues/099`).
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default();
        let regs_url = format!("{origin}/api/my-registrations");
        if let Ok(resp) = crate::api::fetch::get(&regs_url, &[]).await
            && resp.status() == 200
            && let Ok(body) = crate::api::fetch::response_json::<
                crate::api::ApiResponse<Vec<MyRegistration>>,
            >(&resp)
            .await
        {
            set_signed_in.set(true);
            let rows = body.data.unwrap_or_default();
            let (past, current): (Vec<_>, Vec<_>) = rows
                .into_iter()
                .map(|r| {
                    // `event_end_ms` is 0 on the legacy KV path and on events
                    // with no end recorded; fall back to the start so such a row
                    // still sorts somewhere sensible instead of to 1970.
                    let ends = match r.event_end_ms > 0 {
                        true => r.event_end_ms,
                        false => r.event_start_ms,
                    };
                    (
                        ends < now_ms,
                        Row {
                            title: r.event_name,
                            // Their ticket, not the public page. Falls back to
                            // the event page only when the API has no next step
                            // to offer.
                            href: match r.next_step.url.is_empty() {
                                false => r.next_step.url.clone(),
                                true => format!("/e/{}", r.event_slug),
                            },
                            start_ms: r.event_start_ms,
                            time_tba: r.time_tba,
                            location: r.location,
                            image: pick_image(&r.poster_url, &r.nft_image_url),
                            status: Some(r.status),
                            past: ends < now_ms,
                        },
                    )
                })
                .partition(|(is_past, _)| *is_past);
            // Soonest first for what is still to come; most recent first for
            // what is done — in both cases the row you care about is at the top.
            let mut current: Vec<Row> = current.into_iter().map(|(_, row)| row).collect();
            current.sort_by_key(|r| r.start_ms);
            let mut past: Vec<Row> = past.into_iter().map(|(_, row)| row).collect();
            past.sort_by_key(|r| -r.start_ms);
            set_mine_now.set(current);
            set_mine_past.set(past);
        }

        // `fetch::get`, not `api_get` / `get_me` — those end a 401 with
        // `redirect_to_login_expired()`, which is what sent signed-out visitors
        // to the login page from this very function (`.issues/099`). Signed out
        // is a normal state here; the header just renders its signed-out half.
        let me_url = format!("{origin}/api/auth/me");
        let me = match crate::api::fetch::get(&me_url, &[]).await {
            Ok(resp) if resp.status() == 200 => crate::api::fetch::response_json::<
                crate::api::ApiResponse<crate::api::MeResponse>,
            >(&resp)
            .await
            .ok()
            .and_then(|b| b.data),
            _ => None,
        };
        match me {
            Some(me) => {
                set_user_role.set(me.role.clone());
                set_auth_state.set(AuthState::SignedIn(me.email));
            }
            None => set_auth_state.set(AuthState::NotSignedIn),
        }

        set_loaded.set(true);
    });

    view! {
        // Outside the container for the same reason as `/feedback`: the nav
        // wraps when squeezed into the reading width (`.issues/108`).
        <SiteHeader auth_state=auth_state user_role=user_role />
        <div class="container dv-page">

            <header class="dv-head">
                <h1>"ค้นพบอีเวนต์"</h1>
                <p class="subtitle">"ดูงานที่กำลังจะมาถึง และงานที่คุณลงทะเบียนไว้"</p>
            </header>

            <Show when=move || loaded.get() && !signed_in.get() fallback=|| ()>
                <p class="dv-signin-hint">
                    "เข้าสู่ระบบเพื่อดูงานที่คุณลงทะเบียนไว้ และงานที่ผ่านมา"
                </p>
            </Show>

            <Show when=move || loaded.get() fallback=|| view! { <p class="page-loading">"กำลังโหลด…"</p> }>
                <Section title="เร็ว ๆ นี้" rows=upcoming empty="ยังไม่มีงานที่เปิดรับอยู่ตอนนี้" />
                <Section title="งานของฉัน" rows=mine_now empty="" />
                <Section title="งานที่ผ่านมา" rows=mine_past empty="" />
            </Show>
        </div>
    }
}

/// A titled list. Renders nothing at all when it is empty and has no empty text —
/// an empty "งานของฉัน" heading tells a new visitor only that they are missing out.
#[component]
fn Section(title: &'static str, rows: ReadSignal<Vec<Row>>, empty: &'static str) -> impl IntoView {
    view! {
        <Show when=move || !rows.get().is_empty() || !empty.is_empty() fallback=|| ()>
            <section class="dv-section">
                <h2 class="dv-section-title">{title}</h2>
                <Show
                    when=move || !rows.get().is_empty()
                    fallback=move || view! { <p class="dv-empty">{empty}</p> }
                >
                    <For each=move || rows.get() key=|r| format!("{}|{}", r.title, r.href) let:row>
                        <a class="dv-row" href=row.href.clone()>
                            <DateChip ms=row.start_ms past=row.past />
                            <div class="dv-row-body">
                                <span class="dv-row-title">{row.title.clone()}</span>
                                <span class="dv-row-meta">
                                    {
                                        let when = match row.time_tba {
                                            true => "เวลาแจ้งภายหลัง".to_string(),
                                            false => crate::utils::format_event_day(row.start_ms),
                                        };
                                        [when, row.location.clone()]
                                            .into_iter()
                                            .filter(|p| !p.is_empty())
                                            .collect::<Vec<_>>()
                                            .join(" · ")
                                    }
                                </span>
                            </div>
                            {match row.status.clone() {
                                Some(status) => view! { <span class="dv-pill">{status}</span> }.into_any(),
                                None => view! { <div></div> }.into_any(),
                            }}
                            {match row.image.is_empty() {
                                true => view! { <div></div> }.into_any(),
                                false => view! {
                                    <img class="dv-thumb" src=row.image.clone() alt="" />
                                }.into_any(),
                            }}
                        </a>
                    </For>
                </Show>
            </section>
        </Show>
    }
}

/// Big day number over a short month, colour-keyed to past/upcoming.
///
/// The colour is the whole point: it says which list a row belongs to without
/// the reader parsing a date, which is what makes a mixed page scannable.
#[component]
fn DateChip(ms: i64, past: bool) -> impl IntoView {
    let (day, month) = crate::utils::format_event_day_parts(ms);
    view! {
        <div class=match past {
            true => "dv-chip is-past",
            false => "dv-chip",
        }>
            <span class="dv-chip-month">{month}</span>
            <span class="dv-chip-day">{day}</span>
        </div>
    }
}
