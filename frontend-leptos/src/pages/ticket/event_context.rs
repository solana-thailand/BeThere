//! Event context badge — shows event image, tagline, location, and link.

use crate::utils;
use leptos::prelude::*;

/// Event context card showing event badge image, tagline, location, and sessions link.
#[component]
pub fn EventContext(
    /// NFT/event badge image URL (empty = hidden)
    #[prop(into)]
    nft_image_url: String,
    /// Event tagline / subtitle (empty = hidden)
    #[prop(into)]
    tagline: String,
    /// Event location (empty = hidden)
    #[prop(into)]
    location: String,
    /// External event page URL (empty = hidden)
    #[prop(into)]
    event_link: String,
    /// Display text for the event link (empty = default "Sessions & Slides ↗")
    #[prop(optional, into)]
    event_link_text: Option<String>,
) -> impl IntoView {
    let has_content = !nft_image_url.is_empty()
        || !tagline.is_empty()
        || !location.is_empty()
        || !event_link.is_empty();

    if !has_content {
        return view! { <div></div> }.into_any();
    }

    let link_label = event_link_text.unwrap_or_else(|| "📅 Sessions & Slides ↗".to_string());

    view! {
        <div class="ticket-event-context">
            {if !nft_image_url.is_empty() {
                let img = nft_image_url.clone();
                view! {
                    <img
                        src=img
                        alt="Event badge"
                        class="ticket-event-badge-img"
                    />
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            {if !tagline.is_empty() {
                let t = tagline.clone();
                view! {
                    <p class="ticket-event-tagline">
                        {utils::escape_html(&t)}
                    </p>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            {if !location.is_empty() {
                let loc = location.clone();
                view! {
                    <p class="ticket-event-location">
                        "📍 " {utils::escape_html(&loc)}
                    </p>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            {if !event_link.is_empty() {
                let link = event_link.clone();
                view! {
                    <a
                        href=link
                        target="_blank"
                        rel="noopener noreferrer"
                        class="ticket-event-link"
                    >
                        {link_label}
                    </a>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
    .into_any()
}
