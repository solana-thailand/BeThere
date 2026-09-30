use crate::components::LightboxImage;
use crate::i18n::{td_string, use_i18n};
use leptos::prelude::*;

/// Public event page hero image.
///
/// 3-tier fallback (Plan 009 AC6): marketing `poster_url` → `nft_image_url`
/// (NFT badge) → a generative poster drawn from the slug (F4; was a bare
/// Ticket icon, so every poster-less event looked the same). The `nft_image_url` tier is kept so
/// existing events with a badge image but no marketing poster still render an
/// image instead of a bare icon — matching the past-events listing card
/// (`past_events.rs`) and avoiding a cross-surface inconsistency.
///
/// The rendered image is click-to-fullscreen via the shared `LightboxImage`
/// component — attendees often need to read dense agenda text on the poster.
pub fn event_hero(poster_url: &str, nft_image_url: &str, slug: &str) -> AnyView {
    let (url, is_poster) = if !poster_url.is_empty() {
        (poster_url, true)
    } else {
        (nft_image_url, false)
    };
    if url.is_empty() {
        // Decorative: it identifies the event visually, it does not depict it.
        let src = crate::utils::poster::poster_data_url(slug);
        view! {
            <div class="pe-hero">
                <img class="pe-hero-img" src=src alt="" width="400" height="500" />
            </div>
        }
        .into_any()
    } else {
        let i18n = use_i18n();
        let url = url.to_string();
        // `LightboxImage` takes a plain `String` alt, so re-render it on a
        // language switch to keep the alt text in the reader's language.
        let lightbox = move || {
            let alt = match is_poster {
                true => td_string!(i18n.get_locale(), event.hero_alt_poster),
                false => td_string!(i18n.get_locale(), event.hero_alt_badge),
            };
            view! { <LightboxImage src=url.clone() alt=alt thumb_class="pe-hero-img" /> }
        };
        view! { <div class="pe-hero">{lightbox}</div> }.into_any()
    }
}
