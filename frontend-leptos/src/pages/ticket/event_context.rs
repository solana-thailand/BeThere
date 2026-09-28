//! Event context badge — shows event image, tagline, location, and link.

use crate::i18n::{t, t_string, use_i18n};
use crate::utils;
use event_checkin_domain::models::event::safe_map_url;
use leptos::prelude::*;

/// Event context card showing event badge image, tagline, location, and sessions link.
#[component]
pub fn EventContext(
    /// NFT/event badge image URL (empty = hidden)
    #[prop(into)]
    nft_image_url: String,
    /// Event name, shown as the card heading (empty = hidden)
    #[prop(optional, into)]
    name: String,
    /// Event tagline / subtitle (empty = hidden)
    #[prop(into)]
    tagline: String,
    /// Event location (empty = hidden)
    #[prop(into)]
    location: String,
    /// Venue map link (empty = location shown as plain text)
    #[prop(optional, into)]
    location_map_url: String,
    /// External event page URL (empty = hidden)
    #[prop(into)]
    event_link: String,
    /// Display text for the event link (empty = default "Sessions & Slides ↗")
    #[prop(optional, into)]
    event_link_text: Option<String>,
) -> impl IntoView {
    let has_content = !nft_image_url.is_empty()
        || !name.is_empty()
        || !tagline.is_empty()
        || !location.is_empty()
        || !event_link.is_empty();

    if !has_content {
        return view! { <div></div> }.into_any();
    }

    let i18n = use_i18n();
    let link_label = move || match &event_link_text {
        Some(text) => text.clone(),
        None => t_string!(i18n, ticket.context.sessions_link).to_string(),
    };

    view! {
        <div class="ticket-event-context">
            {if !nft_image_url.is_empty() {
                let img = nft_image_url.clone();
                view! {
                    <img
                        src=img
                        alt=move || t_string!(i18n, ticket.context.badge_alt)
                        class="ticket-event-badge-img"
                    />
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            {if !name.is_empty() {
                let n = name.clone();
                view! {
                    <h2 class="ticket-event-name">
                        {utils::escape_html(&n)}
                    </h2>
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
                let map_link = safe_map_url(&location_map_url).map(|href| view! {
                    " "
                    <a
                        href=href
                        target="_blank"
                        rel="noopener noreferrer"
                        class="ticket-event-link"
                    >
                        {t!(i18n, ticket.context.map)}
                    </a>
                });
                view! {
                    <p class="ticket-event-location">
                        "📍 " {utils::escape_html(&loc)} {map_link}
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
