//! NFT claimed badge — shows success card with asset ID and explorer link.

use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Green success card showing NFT claim status with asset ID and Orb link.
#[component]
pub fn NftClaimedBadge(
    /// Compressed NFT asset ID
    #[prop(into)]
    asset_id: String,
    /// Orb Markets viewer URL (empty = hidden)
    #[prop(optional, into)]
    orb_link: Option<String>,
    /// Callback to copy text to clipboard (wraps JS interop)
    on_copy: Box<dyn Fn(&str) -> bool>,
) -> impl IntoView {
    let asset_id_short = if asset_id.len() > 12 {
        format!("{}...{}", &asset_id[..6], &asset_id[asset_id.len() - 4..])
    } else {
        asset_id.clone()
    };
    let asset_id_full = asset_id.clone();
    let orb_url = orb_link.clone().unwrap_or_default();
    let i18n = use_i18n();

    view! {
        <div class="ticket-nft-claimed">
            <div class="ticket-nft-claimed-title">
                {t!(i18n, ticket.nft.claimed)}
            </div>
            {if !asset_id_short.is_empty() {
                let full = asset_id_full.clone();
                view! {
                    <div class="ticket-nft-asset-id">
                        <code>{asset_id_short.clone()}</code>
                        <button
                            class="ticket-nft-copy-btn"
                            title=move || t_string!(i18n, ticket.nft.copy_asset_id)
                            on:click=move |_| {
                                let _ = on_copy(&full);
                            }
                        >
                            <Icon icon=IconName::Copy class="icon-sm" />
                        </button>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            {if !orb_url.is_empty() {
                view! {
                    <a
                        href=orb_url
                        target="_blank"
                        rel="noopener noreferrer"
                        class="btn btn-primary ticket-nft-view-btn"
                    >
                        {t!(i18n, ticket.nft.view_on_orb)}
                    </a>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
}
