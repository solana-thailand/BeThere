use crate::components::LightboxImage;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Public event page hero image.
///
/// 3-tier fallback (Plan 009 AC6): marketing `poster_url` → `nft_image_url`
/// (NFT badge) → Ticket icon empty-state. The `nft_image_url` tier is kept so
/// existing events with a badge image but no marketing poster still render an
/// image instead of a bare icon — matching the past-events listing card
/// (`past_events.rs`) and avoiding a cross-surface inconsistency.
///
/// The rendered image is click-to-fullscreen via the shared `LightboxImage`
/// component — attendees often need to read dense agenda text on the poster.
pub fn event_hero(poster_url: &str, nft_image_url: &str) -> AnyView {
    let (url, alt) = if !poster_url.is_empty() {
        (poster_url, "Event poster")
    } else if !nft_image_url.is_empty() {
        (nft_image_url, "Event badge")
    } else {
        ("", "")
    };
    if url.is_empty() {
        view! {
            <div class="pe-hero">
                <span><Icon icon=IconName::Ticket class="icon-2xl" /></span>
            </div>
        }
        .into_any()
    } else {
        let url = url.to_string();
        view! {
            <div class="pe-hero">
                <LightboxImage
                    src=url
                    alt=alt
                    thumb_class="pe-hero-img"
                />
            </div>
        }
        .into_any()
    }
}
