//! `/showed/:showed` (Super GOAT SG4): what a shared "I showed up" link opens.
//! The event, its card and the promise; nothing about the person who shared
//! it. A revoked or unknown link (the worker already answered 404) says so.

use event_checkin_domain::models::record::ShowedPage;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};

use super::doors::SitePage;

async fn fetch_page(token: String) -> Option<ShowedPage> {
    let resp = crate::api::fetch::get(&format!("/api/public/showed/{token}"), &[])
        .await
        .ok()?;
    if resp.status() != 200 {
        return None;
    }
    crate::api::fetch::response_json::<crate::api::ApiResponse<ShowedPage>>(&resp)
        .await
        .ok()?
        .data
}

#[component]
pub fn ShowedPageView() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    let token = use_params_map().with_untracked(|p| p.get("showed").unwrap_or_default());
    let page = LocalResource::new(move || fetch_page(token.clone()));
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.showed_title_page)) />
        <SiteFrame here=SitePage::Events auth_state=auth_state>
            <section class="lp-record">
                <div class="lp-wrap">
                    {move || match page.get() {
                        None => ().into_any(),
                        Some(None) => view! {
                            <h1 class="lp-h2">{tr(|l| td_string!(l, landing.site.showed_gone))}</h1>
                            <p><a href="/events">{tr(|l| td_string!(l, landing.site.showed_next))}</a></p>
                        }.into_any(),
                        Some(Some(p)) => {
                            let card = (!p.card.is_empty()).then(|| view! {
                                <img class="lp-showed-card" src=p.card.clone() alt=format!("I showed up: {}", p.event_name) width="1200" height="630" />
                            });
                            view! {
                                <p class="lp-kicker">{tr(|l| td_string!(l, landing.site.showed_kicker))}</p>
                                <h1 class="lp-h2">{p.event_name.clone()}</h1>
                                <p class="lp-lede">{crate::utils::format_event_day(p.start_ms)}</p>
                                {card}
                                <p>{tr(|l| td_string!(l, landing.site.showed_line))}</p>
                                <p>
                                    <a href=format!("/e/{}", p.event_slug)>{tr(|l| td_string!(l, landing.site.showed_event))}</a>
                                    " · "
                                    <a href="/events">{tr(|l| td_string!(l, landing.site.showed_next))}</a>
                                </p>
                            }.into_any()
                        }
                    }}
                </div>
            </section>
        </SiteFrame>
    }
}
