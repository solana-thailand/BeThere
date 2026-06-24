use crate::components::LightboxImage;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Public event page hero image.
///
/// Shows the marketing `poster_url` when set; otherwise renders a Ticket icon
/// empty-state. We deliberately do NOT fall back to `nft_image_url` — that
/// asset is baked into the on-chain cNFT mint metadata and using it as a hero
/// is a semantic/aspect-ratio mismatch (see Plan 009).
///
/// The rendered image is click-to-fullscreen via the shared `LightboxImage`
/// component — attendees often need to read dense agenda text on the poster.
///
/// `nft_image_url` is kept in the signature for caller compatibility but is
/// intentionally unused here.
pub fn event_hero(poster_url: &str, _nft_image_url: &str) -> AnyView {
    let url = poster_url;
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
