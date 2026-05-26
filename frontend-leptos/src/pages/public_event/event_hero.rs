use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn event_hero(nft_image_url: &str) -> AnyView {
    let has_image = !nft_image_url.is_empty();
    if has_image {
        let url = nft_image_url.to_string();
        view! {
            <div class="pe-hero">
                <img
                    src=url
                    alt="Event Badge"
                    class="pe-hero-img"
                />
            </div>
        }.into_any()
    } else {
        view! {
            <div class="pe-hero">
                <span><Icon icon=IconName::Ticket class="icon-2xl" /></span>
            </div>
        }.into_any()
    }
}
