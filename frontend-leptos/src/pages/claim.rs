//! Claim page — attendees mint their NFT badge after check-in.
//!
//! Public page (no auth required) accessed via claim URL generated at check-in.
//! Flow:
//! 1. Extract claim token from URL path
//! 2. GET /api/claim/{token} — look up attendee & claim status
//! 3. Show wallet input if eligible
//! 4. POST /api/claim/{token} with wallet address — mint cNFT
//! 5. Show success with asset ID + explorer link

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::api::{self, ClaimLookupData, ClaimMintData};
use crate::utils::{escape_html, format_timestamp};

// ---------------------------------------------------------------------------
// Route params
// ---------------------------------------------------------------------------

/// Route parameters for `/claim/:token`.
/// `token` is the UUID v7 claim token generated at check-in.
#[derive(Params, PartialEq, Clone)]
struct ClaimParams {
    token: Option<String>,
}

// ---------------------------------------------------------------------------
// Claim page states
// ---------------------------------------------------------------------------

/// Top-level state machine for the claim page flow.
#[derive(Clone, Debug)]
enum ClaimState {
    /// Loading claim info from backend.
    Loading,
    /// Claim token not found or lookup failed.
    NotFound(String),
    /// Attendee found, has not yet claimed. Ready for wallet input.
    Ready(ClaimLookupData),
    /// Minting in progress (POST /api/claim/{token} sent).
    Minting(ClaimLookupData),
    /// NFT minted successfully.
    Success(ClaimMintData),
    /// Already claimed previously.
    AlreadyClaimed(ClaimLookupData),
    /// Error during minting.
    MintError(ClaimLookupData, String),
}

// ---------------------------------------------------------------------------
// Claim page component
// ---------------------------------------------------------------------------

/// Claim page component — public route at `/claim/:token`.
///
/// Attendees scan their claim QR code (or follow the claim URL) to land here.
/// The page looks up their check-in record and allows them to mint a
/// compressed NFT badge to their Solana wallet.
#[component]
pub fn Claim() -> impl IntoView {
    let params = use_params::<ClaimParams>();

    // Reactive state
    let (state, set_state) = signal(ClaimState::Loading);
    let (wallet_input, set_wallet_input) = signal(String::new());

    // Extract token from URL params and fetch claim info on mount
    Effect::new(move |_| {
        let token = match params.get() {
            Ok(p) => p.token.unwrap_or_default(),
            Err(_) => {
                set_state.set(ClaimState::NotFound(
                    "Invalid claim link — missing token.".to_string(),
                ));
                return;
            }
        };

        if token.is_empty() {
            set_state.set(ClaimState::NotFound(
                "Invalid claim link — missing token.".to_string(),
            ));
            return;
        }

        // Fetch claim info
        leptos::task::spawn_local(async move {
            match api::get_claim(&token).await {
                Ok(data) => {
                    if data.claimed {
                        set_state.set(ClaimState::AlreadyClaimed(data));
                    } else {
                        set_state.set(ClaimState::Ready(data));
                    }
                }
                Err(e) => {
                    log::warn!("[claim] lookup failed for token {token}: {e}");
                    set_state.set(ClaimState::NotFound(format!(
                        "Claim token not found or lookup failed: {e}"
                    )));
                }
            }
        });
    });

    // Handle "Claim NFT" button click
    let handle_claim = move |_| {
        let wallet = wallet_input.get().trim().to_string();
        let token = match params.get() {
            Ok(p) => p.token.unwrap_or_default(),
            Err(_) => return,
        };

        // Basic client-side validation
        if wallet.is_empty() {
            return;
        }
        let wallet_len = wallet.len();
        if !(32..=44).contains(&wallet_len) {
            return;
        }

        // Transition to minting state
        let current_data = match state.get() {
            ClaimState::Ready(d) | ClaimState::MintError(d, _) => d,
            _ => return,
        };
        set_state.set(ClaimState::Minting(current_data.clone()));

        let current_data_clone = current_data.clone();
        leptos::task::spawn_local(async move {
            match api::post_claim(&token, &wallet).await {
                Ok(mint_data) => {
                    log::info!(
                        "[claim] minted nft: asset_id={} sig={}",
                        mint_data.asset_id,
                        mint_data.signature
                    );
                    set_state.set(ClaimState::Success(mint_data));
                }
                Err(e) => {
                    log::error!("[claim] mint failed: {e}");
                    set_state.set(ClaimState::MintError(current_data_clone, format!("{e}")));
                }
            }
        });
    };

    view! {
        <div class="center-page">
            <Title text="Claim Your NFT — BeThere" />
            <div class="container" style="display:flex;flex-direction:column;align-items:center;">
                // Logo
                <div class="logo">"🎫"</div>

                // Title
                <h1>"Claim Your NFT"</h1>

                <p class="subtitle">
                    "You checked in at "
                    <strong>"Road to Mainnet 1 — Bangkok"</strong>
                    ". Claim your proof-of-attendance NFT badge."
                </p>

                // State-dependent rendering
                {move || {
                    match state.get() {
                        // ---- Loading ----
                        ClaimState::Loading => {
                            view! {
                                <div class="claim-loading">
                                    <span class="spinner spinner-lg"></span>
                                    <p>"Loading claim info..."</p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Not Found / Error ----
                        ClaimState::NotFound(msg) => {
                            view! {
                                <div class="result-error" style="width:100%;">
                                    <div class="icon">"❌"</div>
                                    <h2>"Claim Not Found"</h2>
                                    <div class="result-details">
                                        <p>{escape_html(&msg)}</p>
                                    </div>
                                    <a href="/" class="btn btn-outline mt-2" style="margin-top:1rem;">
                                        "Go to Home"
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Ready: show wallet input ----
                        ClaimState::Ready(data) => {
                            let checked_in_display = format_timestamp(&data.checked_in_at);
                            view! {
                                <div class="claim-card" style="width:100%;">
                                    // Attendee info
                                    <div class="card" style="text-align:center;">
                                        <div style="font-size:2rem;margin-bottom:0.5rem;">"🎉"</div>
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="claim-checked-in" style="font-size:0.85rem;color:var(--text-secondary);margin-top:0.25rem;">
                                            "Checked in "{checked_in_display}
                                        </p>
                                    </div>

                                    // Wallet input
                                    <div class="card">
                                        <label style="font-size:0.9rem;font-weight:600;color:var(--text-primary);display:block;margin-bottom:0.5rem;">
                                            "Solana Wallet Address"
                                        </label>
                                        <input
                                            class="claim-wallet-input"
                                            type="text"
                                            placeholder="Enter your Solana wallet address (e.g. 7xK...pKT)"
                                            prop:value=move || wallet_input.get()
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                set_wallet_input.set(val);
                                            }
                                            style="width:100%;padding:0.7rem 0.85rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--text-primary);font-size:0.85rem;font-family:monospace;outline:none;transition:border-color 0.2s;"
                                        />
                                        <p style="font-size:0.75rem;color:var(--text-muted);margin-top:0.5rem;">
                                            "Paste your Phantom, Solflare, or Backpack wallet address."
                                        </p>
                                    </div>

                                    // Claim button
                                    <button
                                        class="btn btn-primary btn-block"
                                        style="margin-top:0.5rem;padding:0.85rem;font-size:1rem;"
                                        on:click=handle_claim
                                        disabled=move || {
                                            let w = wallet_input.get();
                                            let w_trimmed = w.trim();
                                            w_trimmed.is_empty() || !(32..=44).contains(&w_trimmed.len())
                                        }
                                    >
                                        "Claim NFT Badge"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Minting in progress ----
                        ClaimState::Minting(data) => {
                            view! {
                                <div class="claim-loading" style="width:100%;">
                                    <span class="spinner spinner-lg"></span>
                                    <h3 style="margin-top:1rem;">"Minting your NFT..."</h3>
                                    <p style="font-size:0.9rem;color:var(--text-secondary);margin-top:0.5rem;">
                                        "Minting for "{escape_html(&data.name)}
                                    </p>
                                    <p style="font-size:0.8rem;color:var(--text-muted);margin-top:0.25rem;">
                                        "This usually takes 3-5 seconds."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Success! ----
                        ClaimState::Success(data) => {
                            let explorer_url = format!(
                                "https://solscan.io/tx/{}?cluster=devnet",
                                data.signature
                            );
                            let asset_url = format!(
                                "https://solscan.io/token/{}?cluster=devnet",
                                data.asset_id
                            );
                            view! {
                                <div class="result-success" style="width:100%;">
                                    <div class="icon">"🎊"</div>
                                    <h2>"NFT Claimed!"</h2>
                                    <div class="result-details" style="text-align:left;margin-top:1rem;">
                                        <p><strong>"Name:"</strong>" "{escape_html(&data.name)}</p>
                                        <p><strong>"Wallet:"</strong>
                                            <code style="font-size:0.75rem;color:var(--accent);word-break:break-all;">
                                                {escape_html(&data.wallet_address)}
                                            </code>
                                        </p>
                                        <p><strong>"Claimed:"</strong>" "{format_timestamp(&data.claimed_at)}</p>
                                    </div>
                                    <div style="margin-top:1.25rem;display:flex;flex-direction:column;gap:0.5rem;">
                                        <a
                                            href=explorer_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-primary btn-block"
                                        >
                                            "View on Solscan"
                                        </a>
                                        <a
                                            href=asset_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-block"
                                        >
                                            "View NFT Asset"
                                        </a>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Already claimed ----
                        ClaimState::AlreadyClaimed(data) => {
                            let claimed_display = data
                                .claimed_at
                                .as_deref()
                                .map(format_timestamp)
                                .unwrap_or_else(|| "previously".to_string());
                            view! {
                                <div class="result-warning" style="width:100%;">
                                    <div class="icon">"✅"</div>
                                    <h2>"Already Claimed"</h2>
                                    <div class="result-details">
                                        <p>
                                            <strong>{escape_html(&data.name)}</strong>
                                            " — your NFT was claimed "{claimed_display}"."
                                        </p>
                                        <p style="margin-top:0.5rem;font-size:0.85rem;color:var(--text-secondary);">
                                            "Check your Solana wallet for the NFT badge."
                                        </p>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Mint error ----
                        ClaimState::MintError(data, error) => {
                            view! {
                                <div class="result-error" style="width:100%;">
                                    <div class="icon">"⚠️"</div>
                                    <h2>"Minting Failed"</h2>
                                    <div class="result-details">
                                        <p>{escape_html(&error)}</p>
                                    </div>
                                    <button
                                        class="btn btn-primary mt-2"
                                        style="margin-top:1rem;"
                                        on:click=move |_| {
                                            set_state.set(ClaimState::Ready(data.clone()));
                                        }
                                    >
                                        "Try Again"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}

                // Footer
                <div class="footer">
                    "Built with 🦀 Rust — BeThere × Solana Thailand"
                </div>
            </div>
        </div>
    }
}
