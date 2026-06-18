use crate::components::LightboxImage;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Public event page hero image.
///
/// Prefers the marketing `poster_url`; falls back to the NFT badge image
/// (`nft_image_url`); falls back to a Ticket icon when neither is set.
///
/// `poster_url` and `nft_image_url` are deliberately separate fields — the
/// NFT image is baked into the on-chain cNFT mint metadata, so overloading it
/// with a marketing poster would corrupt every claimed NFT. See Plan 009.
///
/// The rendered image is click-to-fullscreen via the shared `LightboxImage`
/// component — attendees often need to read dense agenda text on the poster.
pub fn event_hero(poster_url: &str, nft_image_url: &str) -> AnyView {
    // Prefer the marketing poster; fall back to the NFT badge image.
    let url = if !poster_url.is_empty() {
        poster_url
    } else {
        nft_image_url
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
                    alt="Event poster"
                    thumb_class="pe-hero-img"
                />
            </div>
        }
        .into_any()
    }
}
