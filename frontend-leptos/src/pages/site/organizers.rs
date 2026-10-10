//! `/organizers` (.plans/045 R4.5): the page head, then "How it works", the
//! landing's swimlane as is, with the try line under it once the devnet
//! sandbox is live.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};
use crate::pages::landing::how::HowItWorks;

use super::doors::{SitePage, TRY_LIVE, try_line};
use super::head::PageHead;

#[component]
pub fn Organizers() -> impl IntoView {
    let (auth_state, _) = use_site_auth();
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.title_organizers)) />
        <SiteFrame here=SitePage::Organizers auth_state=auth_state>
            <PageHead
                kicker=|l| td_string!(l, landing.site.head_org_kicker)
                title=|l| td_string!(l, landing.site.head_org_1)
                title_2=|l| td_string!(l, landing.site.head_org_2)
                sub=|l| td_string!(l, landing.site.head_org_sub)
            />
            <HowItWorks />
            <div class="lp-wrap">{try_line(TRY_LIVE)}</div>
        </SiteFrame>
    }
}
