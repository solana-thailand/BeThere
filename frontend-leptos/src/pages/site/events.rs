//! `/events` (.plans/045 R4.0). Until R4.3 builds the events-and-courses
//! page, `/events` shows the existing Discover list, and `/discover` (the old
//! address, in shared links and the boot summary) moves there.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_location, use_navigate};

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::DiscoverList;
use crate::pages::landing::AuthState;
use crate::pages::landing::frame::SiteFrame;

use super::doors::SitePage;

/// `/events`: the Discover list in the site frame, so the header and doors
/// match the other site pages until R4.3 replaces the list.
#[component]
pub fn EventsPage() -> impl IntoView {
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.title_events)) />
        <SiteFrame here=SitePage::Events auth_state=auth_state>
            <section class="lp-events-sec">
                <DiscoverList set_auth_state=set_auth_state />
            </section>
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
