//! The landing page component itself: the hero (the payers' hall as a band,
//! then the words), the reader's own registrations, then the photos. As in the prototype (.plans/045), the open events live on
//! `/events`, the story, how it works and the organizer card on
//! `/organizers`, the sponsor section on `/sponsors`.

use leptos::prelude::*;

use crate::pages::site::doors::SitePage;

use super::frame::{SiteFrame, use_site_auth};
use super::hero::Hero;
use super::photos::PhotoReel;
use super::registrations::MyRegistrations;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (auth_state, user_role) = use_site_auth();

    view! {
        <SiteFrame here=SitePage::Home auth_state=auth_state own_doors=true>
            <Hero auth_state=auth_state user_role=user_role />
            <MyRegistrations />
            // Nothing else: the numbers live where they are asked for (the
            // medians on /organizers and /events, the running totals on
            // /sponsors), and the three doors are the header's links
            // (.plans/049, owner: lean, less is more).
            <section class="lp-goal">
                <PhotoReel />
            </section>
        </SiteFrame>
    }
}
