//! `/events` (.plans/045 R4.3, the prototype's events page): the head with
//! the open events beside it (or the empty state), the reader's own events
//! when signed in (the Discover list), learn from past events, how the
//! deposit works and the payers who came, then the doors. `/discover` (the
//! old address, in shared links and the boot summary) moves here.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_location, use_navigate};

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::DiscoverList;
use crate::pages::landing::AuthState;
use crate::pages::landing::frame::SiteFrame;

use super::deposit_walk::DepositWalk;
use super::doors::SitePage;
use super::head::PageHead;
use super::learn::Learn;
use super::open_events::OpenEvents;

/// `/events`.
#[component]
pub fn EventsPage() -> impl IntoView {
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.title_events)) />
        <SiteFrame here=SitePage::Events auth_state=auth_state>
            <PageHead
                kicker=|l| td_string!(l, landing.hero.kicker)
                title=|l| td_string!(l, landing.site.head_events_1)
                title_2=|l| td_string!(l, landing.site.head_events_2)
                sub=|l| td_string!(l, landing.site.head_events_sub)
                aside=view! { <OpenEvents /> }.into_any()
            />
            <section class="lp-events-sec">
                <DiscoverList set_auth_state=set_auth_state />
            </section>
            <Learn />
            <DepositWalk />
        </SiteFrame>
    }
}

/// `/discover` → `/events`, keeping the query and the hash. The history
/// entry is replaced, so Back does not land on the redirect again.
#[component]
pub fn DiscoverRedirect() -> impl IntoView {
    let navigate = use_navigate();
    let location = use_location();
    let search = location.search.get_untracked();
    let target = format!(
        "{}{}{}",
        super::doors::SitePage::Events.path(),
        if search.is_empty() {
            String::new()
        } else {
            format!("?{search}")
        },
        location.hash.get_untracked()
    );
    Effect::new(move |_| {
        navigate(
            &target,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });
}
