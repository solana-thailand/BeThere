//! Status hero banner for the ticket page.

use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Props for the ticket hero banner.
///
/// Text props are views rather than strings so a caller can pass catalog text
/// (`move || t!(…)`) that follows a language switch.
#[component]
pub fn TicketHero(
    /// CSS modifier class (e.g., "ticket-hero--checked-in", "ticket-hero--pending", "ticket-hero--ready", "ticket-hero--online")
    #[prop(into)]
    variant: String,
    /// Icon to display
    icon: IconName,
    /// Main title text
    #[prop(into)]
    title: ViewFn,
    /// Optional subtitle (`None` = hidden)
    #[prop(optional, into)]
    subtitle: Option<ViewFn>,
    /// Optional badge text (`None` = hidden)
    #[prop(optional, into)]
    badge: Option<ViewFn>,
) -> impl IntoView {
    let variant_class = variant.clone();
    view! {
        <div class=format!("ticket-hero {variant_class}")>
            {match badge {
                Some(badge) => view! {
                    <div class="ticket-hero-badge">
                        {badge.run()}
                    </div>
                }.into_any(),
                None => view! { <div></div> }.into_any(),
            }}
            <div class="ticket-hero-icon">
                <Icon icon=icon class="icon-xl" />
            </div>
            <div class="ticket-hero-title">{title.run()}</div>
            {match subtitle {
                Some(sub) => view! {
                    <div class="ticket-hero-sub">{sub.run()}</div>
                }.into_any(),
                None => view! { <div></div> }.into_any(),
            }}
        </div>
    }
}
