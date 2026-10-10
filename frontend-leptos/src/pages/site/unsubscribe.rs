//! `/unsubscribe/:token` (.plans/045 R4.12): the link at the foot of every
//! announcement. Nothing happens on load (mail scanners follow links); the
//! button posts the token. No sign-in.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};

use super::doors::SitePage;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Ask,
    Sending,
    Done,
    Failed,
}

#[component]
pub fn UnsubscribePage() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    let token = use_params_map().with_untracked(|p| p.get("token").unwrap_or_default());
    let step = RwSignal::new(Step::Ask);
    let unsubscribe = move |_| {
        step.set(Step::Sending);
        let token = token.clone();
        leptos::task::spawn_local(async move {
            let origin = web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_default();
            let url = format!("{origin}/api/unsubscribe/{}", urlencoding::encode(&token));
            let ok = matches!(
                crate::api::fetch::post(&url, &[], None).await,
                Ok(resp) if (200..300).contains(&resp.status())
            );
            step.set(if ok { Step::Done } else { Step::Failed });
        });
    };
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.unsub_title_page)) />
        <SiteFrame here=SitePage::Home auth_state=auth_state>
            <section class="lp-unsub">
                <div class="lp-wrap">
                    <h1 class="lp-h2">{tr(|l| td_string!(l, landing.site.unsub_title))}</h1>
                    {move || match step.get() {
                        Step::Done => view! {
                            <p class="lp-lede">{tr(|l| td_string!(l, landing.site.unsub_done))}</p>
                            <p><a href="/events">{tr(|l| td_string!(l, landing.site.unsub_again))}</a></p>
                        }.into_any(),
                        s => view! {
                            <p class="lp-lede">{tr(|l| td_string!(l, landing.site.unsub_line))}</p>
                            <button
                                class="lp-btn lp-btn-primary"
                                type="button"
                                disabled=s == Step::Sending
                                on:click=unsubscribe.clone()
                            >
                                {tr(|l| td_string!(l, landing.site.unsub_cta))}
                            </button>
                            {(s == Step::Failed).then(|| view! {
                                <p class="lp-sub-error" role="alert">{tr(|l| td_string!(l, landing.site.unsub_error))}</p>
                            })}
                        }.into_any(),
                    }}
                </div>
            </section>
        </SiteFrame>
    }
}
