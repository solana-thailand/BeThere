//! Developer Dashboard — public page to view NFTs, ranking, and tier badges.
//!
//! Connects a Solana wallet (via browser adapter or manual address input),
//! fetches on-chain compressed NFTs via the DAS API, and displays:
//! - NFT gallery with image previews
//! - Weighted score breakdown (event NFTs × 1 + campaign NFTs × 3)
//! - Tier badge (Newcomer → Legend)
//! - Campaign progress for logged-in developers

use leptos::prelude::*;
use leptos_meta::Title;
use wasm_bindgen::prelude::*;

use crate::api::{
    self, DeveloperProgressItem,
    calculate_weighted_score, compute_tier, get_wallet_nfts, NftItem, Tier,
};
use crate::components::{show_toast, Toast, ToastMessage, ToastType};
use crate::icons::{wallet_icon_name, Icon, IconName};
use crate::utils::metaplex_explorer_url;

// ---------------------------------------------------------------------------
// JS interop — Solana wallet adapter (same bridge as claim.rs / deposit.rs)
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "isWalletAvailable")]
    fn is_wallet_available_js(wallet_name: &str) -> bool;
}

async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[dev_dashboard] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum DashboardState {
    /// Initial — no wallet connected yet.
    Connect,
    /// Loading NFTs from DAS API.
    Loading(String),
    /// NFTs loaded successfully.
    Loaded(WalletNftsData),
    /// Error loading NFTs.
    Error(String),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct WalletNftsData {
    wallet_address: String,
    nfts: Vec<NftItem>,
    tier: Tier,
    score: i64,
}

// ---------------------------------------------------------------------------
// Tier badge component
// ---------------------------------------------------------------------------

fn tier_badge_class(tier: &Tier) -> &'static str {
    match tier {
        Tier::Newcomer => "badge badge-muted",
        Tier::Participant => "badge badge-info",
        Tier::Collector => "badge badge-warning",
        Tier::Dedicated => "badge badge-success",
        Tier::Legend => "badge badge-accent",
    }
}

fn tier_description(tier: &Tier) -> &'static str {
    match tier {
        Tier::Newcomer => "Attend your first event to earn a badge!",
        Tier::Participant => "You've earned your first NFT. Keep going!",
        Tier::Collector => "Nice collection! You're building a portfolio.",
        Tier::Dedicated => "Impressive dedication! You're a regular attendee.",
        Tier::Legend => "Legendary status! You're a true community pillar.",
    }
}

// ---------------------------------------------------------------------------
// NFT card component
// ---------------------------------------------------------------------------

#[component]
fn NftCard(nft: NftItem, cluster: String) -> impl IntoView {
    let name = nft
        .name
        .clone()
        .unwrap_or_else(|| "Untitled NFT".to_string());
    let image_uri = nft.image_uri.clone().unwrap_or_default();
    let symbol = nft.symbol.clone().unwrap_or_default();
    let asset_id = nft.asset_id.clone();

    let explorer_url = metaplex_explorer_url(&asset_id, &cluster);

    let short_id = if asset_id.len() > 16 {
        format!("{}...{}", &asset_id[..8], &asset_id[asset_id.len() - 8..])
    } else {
        asset_id.clone()
    };

    view! {
        <div class="dev-nft-card">
            <div class="dev-nft-image-wrap">
                {if image_uri.is_empty() {
                    view! {
                        <div class="dev-nft-placeholder">
                            <Icon icon=IconName::Ticket class="icon-xl" />
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <img
                            class="dev-nft-image"
                            src=image_uri
                            alt=name.clone()
                            loading="lazy"
                        />
                    }.into_any()
                }}
            </div>
            <div class="dev-nft-info">
                <div class="dev-nft-name">{name}</div>
                {if !symbol.is_empty() {
                    view! { <div class="dev-nft-symbol">{symbol}</div> }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                <a
                    class="dev-nft-link"
                    href=explorer_url
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    <Icon icon=IconName::Link class="icon-sm" />
                    {short_id}
                </a>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Score breakdown component
// ---------------------------------------------------------------------------

#[component]
fn ScoreBreakdown(total_nfts: i64, tier: Tier, score: i64) -> impl IntoView {
    let next_tier = match tier {
        Tier::Newcomer => Some((Tier::Participant, 1)),
        Tier::Participant => Some((Tier::Collector, 3)),
        Tier::Collector => Some((Tier::Dedicated, 5)),
        Tier::Dedicated => Some((Tier::Legend, 10)),
        Tier::Legend => None,
    };

    view! {
        <div class="dev-score-card">
            <div class="dev-score-header">
                <span class=tier_badge_class(&tier)>
                    {tier.emoji()} " " {tier.label()}
                </span>
                <span class="dev-score-value">{score} " pts"</span>
            </div>
            <p class="dev-score-desc">{tier_description(&tier)}</p>
            <div class="dev-score-breakdown">
                <div class="dev-score-row">
                    <span>"Event NFTs"</span>
                    <span class="dev-score-row-val">{total_nfts} " × 1 = " {total_nfts}</span>
                </div>
                <div class="dev-score-row">
                    <span>"Campaign NFTs"</span>
                    <span class="dev-score-row-val">"× 3"</span>
                </div>
                <div class="dev-score-row dev-score-total">
                    <span>"Total"</span>
                    <span class="dev-score-row-val">{score}</span>
                </div>
            </div>
            {if let Some((next, needed)) = next_tier {
                let progress = ((score as f64) / (needed as f64) * 100.0).min(100.0) as i64;
                view! {
                    <div class="dev-score-progress">
                        <div class="dev-score-progress-label">
                            {format!("{} {} pts to {}", next.emoji(), needed, next.label())}
                        </div>
                        <div class="dev-score-progress-bar">
                            <div
                                class="dev-score-progress-fill"
                                style=format!("width: {progress}%")
                            ></div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <div class="dev-score-max">"You've reached the highest tier!"</div> }.into_any()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Campaign progress component (for logged-in users)
// ---------------------------------------------------------------------------

#[component]
fn CampaignProgress(
    items: Vec<DeveloperProgressItem>,
    wallet_address: String,
    set_toast: WriteSignal<Option<ToastMessage>>,
    set_campaign_progress: WriteSignal<Vec<DeveloperProgressItem>>,
) -> impl IntoView {
    let (claiming_id, set_claiming_id) = signal(None::<String>);
    let wallet = wallet_address;

    if items.is_empty() {
        return view! {
            <div class="dev-campaign-empty">
                <Icon icon=IconName::Target class="icon-lg icon-muted" />
                <p>"No campaign progress yet."</p>
                <p class="hint-text">"Check in at events that are part of campaigns to start earning!"</p>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="dev-campaign-list">
            {items.into_iter().map(|item| {
                let pct = if item.total_required > 0 {
                    ((item.events_completed as f64) / (item.total_required as f64) * 100.0).min(100.0) as i64
                } else {
                    0
                };
                let status_label = if item.is_complete {
                    "Completed"
                } else {
                    "In Progress"
                };
                let status_class = if item.is_complete {
                    "badge badge-success"
                } else {
                    "badge badge-warning"
                };
                let campaign_id = item.campaign_id.clone();

                view! {
                    <div class="dev-campaign-item">
                        <div class="dev-campaign-header">
                            <span class="dev-campaign-id">{item.campaign_id}</span>
                            <span class=status_class>{status_label}</span>
                        </div>
                        <div class="dev-campaign-progress">
                            <div class="dev-campaign-bar">
                                <div class="dev-campaign-fill" style=format!("width: {pct}%")></div>
                            </div>
                            <span class="dev-campaign-count">
                                {format!("{}/{}", item.events_completed, item.total_required)}
                            </span>
                        </div>
                        {if item.reward_claimed_at.is_some() {
                            view! {
                                <div class="dev-campaign-reward">
                                    <Icon icon=IconName::Gift class="icon-sm icon-success" />
                                    " Reward claimed"
                                </div>
                            }.into_any()
                        } else if item.is_complete {
                            let cid_click = campaign_id.clone();
                            let cid_label = campaign_id.clone();
                            let w = wallet.clone();
                            view! {
                                <div class="dev-campaign-reward dev-campaign-reward-pending">
                                    <button
                                        class="btn btn-primary btn-sm dev-campaign-claim-btn"
                                        disabled=move || claiming_id.get().is_some()
                                        on:click=move |_| {
                                            let cid = cid_click.clone();
                                            let w = w.clone();
                                            set_claiming_id.set(Some(cid.clone()));
                                            leptos::task::spawn_local(async move {
                                                match api::claim_campaign_reward(&cid, &w).await {
                                                    Ok(resp) => {
                                                        // Refresh progress so the row flips to "Reward claimed"
                                                        if let Ok(progress) = api::my_campaign_progress().await {
                                                            set_campaign_progress.set(progress);
                                                        }
                                                        set_claiming_id.set(None);
                                                        let short = if resp.asset_id.len() > 12 {
                                                            format!(
                                                                "{}...{}",
                                                                &resp.asset_id[..6],
                                                                &resp.asset_id[resp.asset_id.len() - 4..]
                                                            )
                                                        } else {
                                                            resp.asset_id.clone()
                                                        };
                                                        show_toast(
                                                            &set_toast,
                                                            &format!("Campaign reward claimed! NFT asset: {short}"),
                                                            ToastType::Success,
                                                        );
                                                    }
                                                    Err(e) => {
                                                        set_claiming_id.set(None);
                                                        let msg = if e.status == 422
                                                            && e.message.contains("already claimed")
                                                        {
                                                            "Reward already claimed for this campaign.".to_string()
                                                        } else if e.status == 502 {
                                                            format!(
                                                                "Reward service unavailable. Please retry. ({})",
                                                                e.message
                                                            )
                                                        } else {
                                                            format!("Claim failed: {}", e.message)
                                                        };
                                                        show_toast(&set_toast, &msg, ToastType::Error);
                                                    }
                                                }
                                            });
                                        }
                                    >
                                        {move || {
                                            if claiming_id.get().as_deref() == Some(cid_label.as_str()) {
                                                "Claiming..."
                                            } else {
                                                "Claim Reward"
                                            }
                                        }}
                                    </button>
                                </div>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }}
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
    .into_any()
}

// ---------------------------------------------------------------------------
// Main Dashboard component
// ---------------------------------------------------------------------------

#[component]
pub fn DevDashboard() -> impl IntoView {
    let (state, set_state) = signal(DashboardState::Connect);
    let (wallet_input, set_wallet_input) = signal(String::new());
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    let (connected_wallet, set_connected_wallet) =
        signal(None::<(String, String)>); // (wallet_name, public_key)
    let (cluster, set_cluster) = signal("devnet".to_string());
    let (campaign_progress, set_campaign_progress) = signal(Vec::<DeveloperProgressItem>::new());
    let (auth_checked, set_auth_checked) = signal(false);
    let (toast, set_toast) = signal(None::<ToastMessage>);

    // Fetch cluster on mount
    {
        let set_c = set_cluster;
        leptos::task::spawn_local(async move {
            let c = crate::utils::fetch_cluster().await;
            set_c.set(c);
        });
    }

    // Detect installed wallets on mount (poll for late injection)
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo_timers::future::TimeoutFuture::new(300).await;
                    wallets = get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            set_dw.set(wallets);
        });
    }

    // Check if user is logged in (to fetch campaign progress)
    {
        let set_cp = set_campaign_progress;
        let set_ac = set_auth_checked;
        leptos::task::spawn_local(async move {
            match api::my_campaign_progress().await {
                Ok(progress) => {
                    set_cp.set(progress);
                }
                Err(_) => {
                    // Not logged in — that's fine, just no campaign progress
                }
            }
            set_ac.set(true);
        });
    }

    // Load NFTs for a given wallet address
    let load_nfts = move |address: String| {
        let addr = address.trim().to_string();
        if addr.is_empty() {
            return;
        }
        let set_state = set_state;
        leptos::task::spawn_local(async move {
            set_state.set(DashboardState::Loading(addr.clone()));
            match get_wallet_nfts(&addr).await {
                Ok(resp) => {
                    // Classify NFTs as campaign vs event using campaign_mints from backend
                    let campaign_mint_set: std::collections::HashSet<&str> = resp
                        .campaign_mints
                        .iter()
                        .map(|s| s.as_str())
                        .collect();

                    let mut event_nfts = 0i64;
                    let mut campaign_nfts = 0i64;
                    for nft in &resp.nfts {
                        match nft.collection_mint.as_deref() {
                            Some(mint) if campaign_mint_set.contains(mint) => campaign_nfts += 1,
                            _ => event_nfts += 1,
                        }
                    }
                    let score = calculate_weighted_score(event_nfts, campaign_nfts);
                    let tier = compute_tier(score);

                    set_state.set(DashboardState::Loaded(WalletNftsData {
                        wallet_address: resp.wallet_address,
                        nfts: resp.nfts,
                        tier,
                        score,
                    }));
                }
                Err(e) => {
                    set_state.set(DashboardState::Error(format!(
                        "Failed to load NFTs: {}",
                        e.message
                    )));
                }
            }
        });
    };

    let on_connect_wallet = move |wallet_name: String| {
        let set_cw = set_connected_wallet;
        let load = load_nfts;
        leptos::task::spawn_local(async move {
            match connect_wallet_js(&wallet_name).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!(
                        "[dev_dashboard] wallet connected: {} ({})",
                        wallet_name,
                        pubkey
                    );
                    set_cw.set(Some((wallet_name, pubkey.clone())));
                    load(pubkey);
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    if e.raw_message.contains("Wallet not found") {
                        let url = match wallet_name.to_lowercase().as_str() {
                            "phantom" => "https://phantom.app/download",
                            "backpack" => "https://backpack.app/download",
                            "solflare" => "https://solflare.com/download",
                            _ => "https://phantom.app/download",
                        };
                        let _ = web_sys::window().map(|w| w.open_with_url(url));
                    } else {
                        set_state.set(DashboardState::Error(
                            crate::wallet_error::user_friendly_message(&e),
                        ));
                    }
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    set_state.set(DashboardState::Error(
                        "Failed to connect wallet. Please try again.".to_string(),
                    ));
                }
            }
        });
    };

    let on_manual_lookup = move || {
        let addr = wallet_input.get();
        load_nfts(addr);
    };

    let on_disconnect = move || {
        set_connected_wallet.set(None);
        set_state.set(DashboardState::Connect);
    };

    view! {
        <Title text="Developer Dashboard — BeThere" />
        <Toast toast_signal=toast />
        <div class="dev-dashboard">
            <div class="dev-dashboard-header">
                <h1 class="dev-dashboard-title">
                    <Icon icon=IconName::Trophy class="icon-lg" />
                    " Developer Dashboard"
                </h1>
                <p class="dev-dashboard-subtitle">
                    "Connect your Solana wallet to view your NFT collection, ranking, and tier."
                </p>
            </div>

            // Wallet connection section
            <div class="dev-dashboard-wallet">
                {move || {
                    let cw = connected_wallet.get();
                    match cw {
                        Some((ref wallet_name, ref public_key)) => {
                            let wallet_icon = wallet_icon_name(wallet_name);
                            let pk_short = if public_key.len() > 12 {
                                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                            } else {
                                public_key.clone()
                            };
                            view! {
                                <div class="wallet-connected-bar">
                                    <span class="wallet-icon-lg"><Icon icon=wallet_icon class="icon-lg" /></span>
                                    <div class="wallet-info-left">
                                        <div class="wallet-label">"Connected via " {wallet_name.clone()}</div>
                                        <div class="wallet-address-bold">{pk_short}</div>
                                    </div>
                                    <span class="badge badge-success u-ml-auto">
                                        <Icon icon=IconName::Check class="icon-sm icon-success" />
                                        " Connected"
                                    </span>
                                </div>
                                <button
                                    class="btn btn-outline btn-sm"
                                    on:click=move |_| on_disconnect()
                                >
                                    "Disconnect"
                                </button>
                            }.into_any()
                        }
                        None => {
                            let wallets = detected_wallets.get();
                            let mut wallets = wallets.clone();
                            if !wallets.iter().any(|w| w.eq_ignore_ascii_case("Phantom")) {
                                wallets.push("Phantom".to_string());
                            }

                            view! {
                                <div class="dev-wallet-buttons">
                                    {wallets.into_iter().map(|w| {
                                        let w_clone = w.clone();
                                        let wallet_icon = wallet_icon_name(&w);
                                        let is_available = is_wallet_available_js(&w);
                                        let label = if is_available { w.clone() } else { format!("Install {w}") };
                                        view! {
                                            <button
                                                class="btn btn-outline dev-wallet-btn"
                                                on:click=move |_| on_connect_wallet(w_clone.clone())
                                            >
                                                <Icon icon=wallet_icon class="icon-sm" />
                                                {label}
                                            </button>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                                <div class="claim-wallet-divider">"or enter wallet address"</div>
                                <div class="claim-wallet-row">
                                    <input
                                        class="claim-wallet-input"
                                        type="text"
                                        placeholder="Enter Solana wallet address..."
                                        prop:value=move || wallet_input.get()
                                        on:input=move |ev| {
                                            set_wallet_input.set(event_target_value(&ev));
                                        }
                                        on:keydown=move |ev| {
                                            if ev.key() == "Enter" {
                                                on_manual_lookup();
                                            }
                                        }
                                    />
                                    <button
                                        class="btn btn-primary btn-sm"
                                        on:click=move |_| on_manual_lookup()
                                    >
                                        "Lookup"
                                    </button>
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </div>

            // Main content
            {move || {
                let current_state = state.get();
                let c = cluster.get();
                match current_state {
                    DashboardState::Connect => {
                        view! {
                            <div class="dev-empty-state">
                                <Icon icon=IconName::Wallet class="icon-xl icon-muted" />
                                <p>"Connect your wallet or enter an address to view your NFTs."</p>
                            </div>
                        }.into_any()
                    }
                    DashboardState::Loading(addr) => {
                        let short = if addr.len() > 16 {
                            format!("{}...{}", &addr[..8], &addr[addr.len()-8..])
                        } else {
                            addr.clone()
                        };
                        view! {
                            <div class="dev-loading">
                                <div class="spinner"></div>
                                <p>"Loading NFTs for "{short}"..."</p>
                            </div>
                        }.into_any()
                    }
                    DashboardState::Error(msg) => {
                        view! {
                            <div class="dev-error">
                                <Icon icon=IconName::Warning class="icon-lg icon-warning" />
                                <p>{msg}</p>
                                <button class="btn btn-outline btn-sm" on:click=move |_| on_disconnect()>
                                    "Try Again"
                                </button>
                            </div>
                        }.into_any()
                    }
                    DashboardState::Loaded(data) => {
                        let wallet_address = data.wallet_address.clone();
                        let cp = campaign_progress.get();
                        let show_campaigns = auth_checked.get() && !cp.is_empty();
                        view! {
                            <div class="dev-content">
                                // Score card
                                <ScoreBreakdown
                                    total_nfts=data.nfts.len() as i64
                                    tier=data.tier
                                    score=data.score
                                />

                                // NFT Gallery
                                <div class="dev-section">
                                    <h2 class="dev-section-title">
                                        <Icon icon=IconName::Ticket class="icon-sm" />
                                        {format!(" NFT Collection ({})", data.nfts.len())}
                                    </h2>
                                    {if data.nfts.is_empty() {
                                        view! {
                                            <div class="dev-nft-empty">
                                                <p>"No NFTs found for this wallet."</p>
                                                <p class="hint-text">"Attend events and check in to earn NFT badges!"</p>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="dev-nft-grid">
                                                {data.nfts.into_iter().map(|nft| {
                                                    let c_clone = c.clone();
                                                    view! { <NftCard nft=nft cluster=c_clone /> }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }}
                                </div>

                                // Campaign Progress (only if logged in)
                                {if show_campaigns {
                                    view! {
                                        <div class="dev-section">
                                            <h2 class="dev-section-title">
                                                <Icon icon=IconName::Target class="icon-sm" />
                                                " Campaign Progress"
                                            </h2>
                                            <CampaignProgress
                                                items=cp
                                                wallet_address=wallet_address.clone()
                                                set_toast=set_toast
                                                set_campaign_progress=set_campaign_progress
                                            />
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                            </div>
                        }.into_any()
                    }
                }
            }}

            // Footer info
            <div class="dev-dashboard-footer">
                <p>
                    <Icon icon=IconName::Info class="icon-sm icon-muted" />
                    " NFT data is read from the Solana blockchain via Helius DAS API. "
                    "Scores are calculated locally: Event NFT = 1pt, Campaign NFT = 3pts."
                </p>
            </div>
        </div>
    }
}
