//! The claim success screen: the minted badge, explorer links, share row and
//! the way back to the ticket. Split out of `page.rs` (.plans/037 §2), which
//! the i18n move pushed past the 1024-line limit.

use leptos::prelude::*;

use crate::i18n::{t_string, use_i18n};
use crate::utils::orb_nft_url;

use super::interop::*;
use crate::api::ClaimMintData;

/// The success screen for a fresh mint.
#[component]
pub(super) fn ClaimSuccess(
    data: ClaimMintData,
    /// Event name for the share text; empty for the generic text.
    event_name: String,
    /// The claim token, for the shareable claim-page link.
    claim_token: String,
    /// `/ticket/{id}[?event_id=…]`.
    ticket_href: String,
    /// Whether the event took a deposit (changes the ticket-link label).
    deposit_enabled: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let (share_copied, set_share_copied) = signal(false);
    let orb_url = orb_nft_url(&data.asset_id, &data.cluster);
    let asset_id_display = {
        let id = &data.asset_id;
        if id.len() > 12 {
            format!("{}...{}", &id[..6], &id[id.len() - 4..])
        } else {
            id.clone()
        }
    };
    let asset_id_full = data.asset_id.clone();

    // Build share text & URL
    // The attendee posts in their own language. `t_string!`
    // cannot interpolate, so the event name goes between
    // two catalog pieces.
    let tweet_text = {
        let event = event_name.clone();
        if event.is_empty() {
            t_string!(i18n, claim.success.tweet_generic).to_string()
        } else {
            let before = t_string!(i18n, claim.success.tweet_event_before);
            let after = t_string!(i18n, claim.success.tweet_event_after);
            format!("{before}{event}{after}")
        }
    };
    let share_to_x_url = format!(
        "https://twitter.com/intent/tweet?text={}",
        js_sys::encode_uri_component(&tweet_text)
    );
    let claim_page_url = format!("https://bethere.solana-thailand.workers.dev/claim/{claim_token}");

    let solscan_url = if data.signature.is_empty() {
        format!(
            "https://solscan.io/account/{}?cluster={}",
            data.asset_id, data.cluster
        )
    } else {
        format!(
            "https://solscan.io/tx/{}?cluster={}",
            data.signature, data.cluster
        )
    };

    view! {
        <div class="claim-success">
            // 1. Celebration: the mark flips over from its paper back (F2).
            <div class="claim-reveal">
                <div class="claim-reveal-inner">
                    <div class="claim-reveal-back" aria-hidden="true"></div>
                    <div class="claim-success-rings claim-reveal-front">
                        <div class="claim-success-ring claim-success-ring-3"></div>
                        <div class="claim-success-ring claim-success-ring-2"></div>
                        <div class="claim-success-ring claim-success-ring-1"></div>
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                    </div>
                </div>
            </div>
            <h2>{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.title))}</h2>

            // 2. Asset ID + View NFT
            <div class="claim-asset-card">
                <div class="claim-asset-header">
                    <span class="claim-asset-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.asset_id))}</span>
                    <span class="claim-asset-status">
                        <span class="claim-asset-status-dot"></span>
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.on_chain))}
                    </span>
                </div>
                <div class="claim-asset-value-row">
                    <span class="claim-asset-code">{asset_id_display}</span>
                    <button
                        class="claim-copy-btn"
                        type="button"
                        title=crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.copy_asset_id))
                        on:click=move |_| {
                            let _ = copy_to_clipboard_js(&asset_id_full);
                        }
                    >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                        </svg>
                    </button>
                </div>
            </div>

            <div class="success-actions" style="display: flex; flex-direction: column; gap: 10px;">
                <a
                    href=orb_url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="btn btn-primary btn-block"
                >
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.view_on_orb))}
                </a>
                <a
                    href=solscan_url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="btn btn-outline btn-block"
                    style="border-color: rgba(20, 241, 149, 0.4); color: #14F195; background: rgba(20, 241, 149, 0.06);"
                >
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.view_solscan))}
                </a>
            </div>

            // 3. Share (compact row)
            <div class="claim-share-section">
            <div class="claim-share-buttons">
                <a
                    href=share_to_x_url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="claim-share-x-btn"
                >
                    <svg viewBox="0 0 24 24" fill="currentColor">
                        <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
                    </svg>
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.post_to_x))}
                </a>
                <button
                    class="claim-share-copy-btn"
                    type="button"
                    title=crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.copy_link))
                    on:click=move |_| {
                        let _ = copy_to_clipboard_js(&claim_page_url);
                        set_share_copied.set(true);
                        leptos::task::spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            set_share_copied.set(false);
                        });
                    }
                >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
                        <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
                    </svg>
                </button>
            </div>
            <div class={move || {
                if share_copied.get() {
                    "claim-share-copied visible".to_string()
                } else {
                    "claim-share-copied".to_string()
                }
            }}>
                {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.link_copied))}
            </div>
            </div>

            // 4. Ticket link — the ticket page is the hub for
            // deposit & refund status (per-method actions live there;
            // the deposit page is only for PAYING a deposit, so linking
            // there post-claim dead-ended non-depositors on a pay form).
            {
                let label = if deposit_enabled {
                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.ticket_with_deposit)).into_any()
                } else {
                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.success.back_to_ticket)).into_any()
                };
                view! {
                    <div class="success-actions claim-success-actions-spaced">
                        <a
                            href=ticket_href
                            class="btn btn-outline btn-block"
                        >
                            {label}
                        </a>
                    </div>
                }.into_any()
            }
        </div>
    }
}
