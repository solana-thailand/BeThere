//! The landing page component itself: the hero (the lit room and the
//! payers' hall), the reader's own registrations, two measured medians, then
//! the photos and so far. As in the prototype (.plans/045), the open events live on
//! `/events`, the story, how it works and the organizer card on
//! `/organizers`, the sponsor section on `/sponsors`.

use leptos::prelude::*;

use crate::pages::site::doors::SitePage;

use super::frame::{SiteFrame, use_site_auth};
use super::hero::Hero;
use super::proof::Proof;
use super::registrations::MyRegistrations;
use super::sofar::SoFar;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (auth_state, user_role) = use_site_auth();

    view! {
        <SiteFrame here=SitePage::Home auth_state=auth_state own_doors=true>
            <Hero auth_state=auth_state user_role=user_role />
            <MyRegistrations />
            // Two measured medians, then the photos and the running totals.
            // The three doors left the home page (.plans/049): the header
            // already links the same three places.
            <Proof />
            <SoFar />
        </SiteFrame>
    }
}
