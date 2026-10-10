//! The landing page component itself: the hero (the lit room and the
//! payers' hall), the reader's own registrations, upcoming events, the doors,
//! then so far and the goal. As in the prototype (.plans/045), the story,
//! how it works and the organizer card live on `/organizers`, the sponsor
//! section on `/sponsors`; upcoming events move to `/events` with R4.3.

use leptos::prelude::*;

use crate::pages::site::doors::{Doors, SitePage};

use super::frame::{SiteFrame, use_site_auth};
use super::hero::Hero;
use super::registrations::MyRegistrations;
use super::sofar::SoFar;
use super::upcoming::UpcomingEvents;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (auth_state, user_role) = use_site_auth();

    view! {
        <SiteFrame here=SitePage::Home auth_state=auth_state own_doors=true>
            <Hero auth_state=auth_state user_role=user_role />
            <MyRegistrations />
            <UpcomingEvents />
            // The doors right under the opening, as in the prototype.
            <Doors here=SitePage::Home />
            <SoFar />
        </SiteFrame>
    }
}
