//! The landing page component itself.

use leptos::prelude::*;

use crate::pages::site::doors::SitePage;

use super::frame::{SiteFrame, use_site_auth};
use super::header::SideIndex;
use super::hero::Hero;
use super::how::HowItWorks;
use super::join::Join;
use super::registrations::MyRegistrations;
use super::sofar::SoFar;
use super::sponsors::Sponsors;
use super::story::Story;
use super::upcoming::UpcomingEvents;

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (auth_state, user_role) = use_site_auth();

    view! {
        <SiteFrame here=SitePage::Home auth_state=auth_state>
            <SideIndex />

            // ===== Hero (build plan 0.5, .plans/043 L2) =====
            <Hero auth_state=auth_state user_role=user_role />

            // ===== My Registrations (signed in) — straight under the hero =====
            <MyRegistrations />

            // ===== Upcoming Events (two cards + see all) =====
            <UpcomingEvents />

            // ===== The commitment ladder: one room of chairs (.plans/043 L8) =====
            <Story />

            // ===== How it works: the swimlane (.plans/043 L4) =====
            <HowItWorks />

            // ===== So far: the numbers from the system (.plans/043 L5) =====
            <SoFar />

            // ===== Sponsors: where a logo goes (.plans/043 L6) =====
            <Sponsors />

            // ===== Join: share, and the organizer card (.plans/043 L7) =====
            <Join auth_state=auth_state user_role=user_role />
        </SiteFrame>
    }
}
