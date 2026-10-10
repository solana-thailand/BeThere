//! The landing page component itself: the hero (the lit room and the
//! payers' hall), the reader's own registrations, the doors, then so far and
//! the goal. As in the prototype (.plans/045), the open events live on
//! `/events`, the story, how it works and the organizer card on
//! `/organizers`, the sponsor section on `/sponsors`.

use leptos::prelude::*;

use crate::pages::site::doors::{Doors, SitePage};

use super::frame::{SiteFrame, use_site_auth};
use super::hero::Hero;
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
            // The doors right under the opening, as in the prototype.
            <Doors here=SitePage::Home />
            <SoFar />
        </SiteFrame>
    }
}
