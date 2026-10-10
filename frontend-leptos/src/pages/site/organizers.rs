//! `/organizers` (.plans/045 R4.5, the prototype's organizers page): the
//! page head, the room narrowed in three steps (`landing/story.rs`), "How
//! it works" (the landing's swimlane, with the try line under it once the
//! devnet sandbox is live), then the planning tile beside the organizer card.
//! The story and the card left the home for this page, as in the prototype.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::td_string;
use crate::locale::tr;
use crate::pages::landing::frame::{SiteFrame, use_site_auth};
use crate::pages::landing::how::HowItWorks;
use crate::pages::landing::join::OrganizerCard;
use crate::pages::landing::story::Story;

use super::doors::{SitePage, TRY_LIVE, try_line};
use super::head::PageHead;
use super::plan::PlanTile;

#[component]
pub fn Organizers() -> impl IntoView {
    let (auth_state, user_role) = use_site_auth();
    view! {
        <Title text=tr(|l| td_string!(l, landing.site.title_organizers)) />
        <SiteFrame here=SitePage::Organizers auth_state=auth_state>
            <PageHead
                kicker=|l| td_string!(l, landing.site.head_org_kicker)
                title=|l| td_string!(l, landing.site.head_org_1)
                title_2=|l| td_string!(l, landing.site.head_org_2)
                sub=|l| td_string!(l, landing.site.head_org_sub)
            >
                <div class="lp-actions">
                    <a class="lp-btn lp-btn-primary" href="#join">
                        {tr(|l| td_string!(l, landing.site.head_org_cta))}
                    </a>
                </div>
            </PageHead>
            <Story />
            <HowItWorks />
            <div class="lp-wrap">{try_line(TRY_LIVE)}</div>
            <section id="join" class="lp-join">
                <div class="lp-wrap lp-plan-bento">
                    <PlanTile />
                    <OrganizerCard auth_state=auth_state user_role=user_role />
                </div>
            </section>
        </SiteFrame>
    }
}
