//! Status hero banner for the ticket page.

use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Props for the ticket hero banner.
#[component]
pub fn TicketHero(
    /// CSS modifier class (e.g., "ticket-hero--checked-in", "ticket-hero--pending", "ticket-hero--ready", "ticket-hero--online")
    #[prop(into)]
    variant: String,
    /// Icon to display
    icon: IconName,
    /// Main title text
    #[prop(into)]
    title: String,
    /// Optional subtitle (empty string = hidden)
    #[prop(optional, into)]
    subtitle: Option<String>,
    /// Optional badge text (empty string = hidden)
    #[prop(optional, into)]
    badge: Option<String>,
) -> impl IntoView {
    let variant_class = variant.clone();
    let badge_text = badge.clone().unwrap_or_default();
    let sub_text = subtitle.clone().unwrap_or_default();
    view! {
        <div class=format!("ticket-hero {variant_class}")>
            {if !badge_text.is_empty() {
                view! {
                    <div class="ticket-hero-badge">
                        {badge_text}
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            <div class="ticket-hero-icon">
                <Icon icon=icon class="icon-xl" />
            </div>
            <div class="ticket-hero-title">{title}</div>
            {if !sub_text.is_empty() {
                view! {
                    <div class="ticket-hero-sub">{sub_text}</div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
}
