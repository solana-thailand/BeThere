//! `/me/record` (Super GOAT SG1, the prototype's `record.html`): the
//! signed-in person's own record. One seat per door event in time order, lit
//! where they came; the events listed; badges with explorer links; held
//! credit (R4.13's line). What counts is `domain::models::record` alone.

use event_checkin_domain::models::record::{MyRecord, settled};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::{t_string, td_string, use_i18n};
use crate::icons::{Icon, IconName};
use crate::locale::{fill, tr};
use crate::pages::landing::AuthState;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};

use super::doors::SitePage;
use super::open_events::CreditHeld;
use super::share_showed::ShareShowed;

#[component]
pub fn RecordPage() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.record_title_page)) />
        <SiteFrame here=SitePage::Events auth_state=auth_state>
            <section class="lp-record">
                <div class="lp-wrap">
                    <h1 class="lp-h2">{tr(|l| td_string!(l, landing.site.record_title))}</h1>
                    {move || match auth_state.get() {
                        AuthState::Checking => ().into_any(),
                        AuthState::SignedIn(_) => view! { <RecordBody /> }.into_any(),
                        _ => view! {
                            <p class="lp-lede">{tr(|l| td_string!(l, landing.site.record_signin))}</p>
                            <a class="lp-btn lp-btn-primary" href="/login?next=/me/record">
                                {tr(|l| td_string!(l, landing.site.record_signin_cta))}
                            </a>
                        }.into_any(),
                    }}
                </div>
            </section>
        </SiteFrame>
    }
}

#[component]
fn RecordBody() -> impl IntoView {
    let i18n = use_i18n();
    let record = LocalResource::new(|| async {
        crate::api::api_get_json_if_signed_in::<MyRecord>("/me/record").await
    });
    move || {
        match record.get() {
        None => ().into_any(),
        Some(None) => view! {
            <p class="lp-sub-error" role="alert">{tr(|l| td_string!(l, landing.site.record_error))}</p>
        }
        .into_any(),
        Some(Some(r)) => {
            let settled = settled(&r.events);
            let line = match settled == 0 {
                true => t_string!(i18n, landing.site.record_empty).to_string(),
                false => fill(
                    t_string!(i18n, landing.site.record_line),
                    &[
                        ("came", &r.door_checkins.to_string()),
                        ("events", &settled.to_string()),
                    ],
                ),
            };
            let seats = r
                .events
                .iter()
                .map(|e| {
                    let class = match (e.came, e.upcoming) {
                        (true, _) => "lp-seat lp-seat-on",
                        (false, true) => "lp-seat lp-seat-soon",
                        (false, false) => "lp-seat",
                    };
                    view! { <li class=class title=e.event_name.clone()></li> }
                })
                .collect::<Vec<_>>();
            let shared = r.shared.clone();
            let rows = r
                .events
                .iter()
                .rev()
                .map(|e| {
                    let share = e.came.then(|| {
                        let token = shared
                            .iter()
                            .find(|s| s.event_slug == e.event_slug)
                            .map(|s| s.token.clone());
                        view! {
                            <ShareShowed slug=e.event_slug.clone() event_name=e.event_name.clone() token=token />
                        }
                    });
                    let mark = match (e.came, e.upcoming) {
                        (true, _) => view! {
                            <span class="lp-record-came"><Icon icon=IconName::Check class="icon-sm" />
                                {tr(|l| td_string!(l, landing.site.record_came))}</span>
                        }
                        .into_any(),
                        (false, true) => view! {
                            <span class="lp-record-missed">{tr(|l| td_string!(l, landing.site.record_upcoming))}</span>
                        }
                        .into_any(),
                        (false, false) => view! {
                            <span class="lp-record-missed">{tr(|l| td_string!(l, landing.site.record_missed))}</span>
                        }
                        .into_any(),
                    };
                    view! {
                        <li>
                            <a href=format!("/e/{}", e.event_slug)>{e.event_name.clone()}</a>
                            <small>{crate::utils::format_event_day(e.start_ms)}</small>
                            {mark}
                            {share}
                        </li>
                    }
                })
                .collect::<Vec<_>>();
            let cluster = r.badge_cluster.clone();
            let badges = r
                .badges
                .iter()
                .map(|b| {
                    let href = crate::utils::orb_nft_url(&b.asset_id, &cluster);
                    view! {
                        <li><a href=href target="_blank" rel="noopener noreferrer">{b.event_name.clone()}</a></li>
                    }
                })
                .collect::<Vec<_>>();
            let has_badges = !badges.is_empty();
            view! {
                <p class="lp-lede">{line}</p>
                <ol class="lp-seats" aria-hidden="true">{seats}</ol>
                <ul class="lp-record-list">{rows}</ul>
                <CreditHeld />
                {has_badges.then(|| view! {
                    <h2 class="lp-record-h">{tr(|l| td_string!(l, landing.site.record_badges))}</h2>
                    <ul class="lp-record-list">{badges}</ul>
                })}
            }
            .into_any()
        }
    }
    }
}
