//! `/events` (.plans/045 R4.0). Until R4.3 builds the events-and-courses
//! page, `/events` shows the existing Discover list, and `/discover` (the old
//! address, in shared links and the boot summary) moves there.

use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_location, use_navigate};

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
