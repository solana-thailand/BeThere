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
    /// Attendee found but NFT minting is not configured yet.
    NftComingSoon(ClaimLookupData),
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
// Helper functions
// ---------------------------------------------------------------------------

/// Format seconds into "Xh Xm Xs" or "Xm Xs" or "Xs".
fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Get initials from a name (first letter of first and last word).
fn get_initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.len() {
        0 => "?".to_string(),
        1 => parts[0].chars().next().unwrap_or('?').to_uppercase().collect(),
        _ => {
            let first = parts[0].chars().next().unwrap_or('?');
            let last = parts.last().unwrap().chars().next().unwrap_or('?');
            format!("{}{}", first.to_uppercase(), last.to_uppercase())
        }
    }
}

/// Simple deterministic hash for generating avatar colors from name.
fn simple_hash(s: &str) -> u32 {
    let mut hash: u32 = 0;
    for b in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    hash
}

// ---------------------------------------------------------------------------
// Interactive widgets (client-side only)
// ---------------------------------------------------------------------------

/// Floating hearts widget — audience taps to send hearts.
/// Purely cosmetic, client-side only. Hearts float up and fade out.
#[component]
fn HeartsWidget() -> impl IntoView {
    let (hearts, set_hearts) = signal(Vec::<u32>::new());
    let (count, set_count) = signal(0u32);
    let heart_id = std::cell::Cell::new(0u32);

    let send_heart = move |_: web_sys::MouseEvent| {
        let id = heart_id.get();
        heart_id.set(id + 1);
        set_hearts.update(|h| h.push(id));
        set_count.update(|c| *c += 1);

        // Remove heart after animation (3 seconds)
        let set_h = set_hearts;
        set_timeout(move || {
            set_h.update(|h| h.retain(|&x| x != id));
        }, std::time::Duration::from_secs(3));
    };

    view! {
        <div class="hearts-widget">
            <button class="heart-btn" on:click=send_heart>
                <svg viewBox="0 0 24 24" width="28" height="28">
                    <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" fill="#ef4444"/>
                </svg>
                <span class="heart-count">{move || count.get()}</span>
            </button>
            <div class="hearts-container">
                {move || hearts.get().iter().map(|&id| {
                    let left = (id % 5) as f64 * 15.0 + 10.0;
                    let delay = (id % 3) as f64 * 0.2;
                    let style = format!(
                        "left:{}%;animation-delay:{}s;",
                        left, delay
                    );
                    view! {
                        <span class="floating-heart" style=style>
                            <svg viewBox="0 0 24 24" width="20" height="20">
                                <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" fill="#ef4444"/>
                            </svg>
                        </span>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

/// Live session timer showing event progress.
/// Shows elapsed time since event start or countdown to start.
#[component]
fn SessionTimer() -> impl IntoView {
    // Event start: April 26, 2026, 09:30 Bangkok time (UTC+7)
    let event_start_ms: f64 = 1_778_160_000_000.0; // approximate UTC ms
    let event_end_ms: f64 = 1_778_173_800_000.0; // 13:00 Bangkok

    let (time_display, set_time_display) = signal(String::new());
    let (status_label, set_status_label) = signal(String::new());

    Effect::new(move |_| {
        let set_t = set_time_display;
        let set_s = set_status_label;

        leptos::task::spawn_local(async move {
            loop {
                let now = js_sys::Date::now();
                let (label, display) = if now < event_start_ms {
                    let diff = ((event_start_ms - now) / 1000.0) as i64;
                    let label = "Starts in".to_string();
                    let display = format_duration(diff);
                    (label, display)
                } else if now < event_end_ms {
                    let diff = ((now - event_start_ms) / 1000.0) as i64;
                    let label = "Live".to_string();
                    let display = format!("+{}", format_duration(diff));
                    (label, display)
                } else {
                    let label = "Ended".to_string();
                    let display = "Thanks for coming!".to_string();
                    (label, display)
                };
                set_s.set(label);
                set_t.set(display);
                gloo::timers::future::TimeoutFuture::new(1000).await;
            }
        });
    });

    view! {
        <div class="session-timer">
            <span class="timer-label">{move || status_label.get()}</span>
            <span class="timer-value">{move || time_display.get()}</span>
        </div>
    }
}

/// Simple avatar showing initials with a gradient background.
#[component]
fn ParticipantAvatar(name: String) -> impl IntoView {
    let initials = get_initials(&name);
    let hue = simple_hash(&name) % 360;
    let bg = format!("hsl({hue}, 60%, 35%)");
    let border = format!("hsl({hue}, 70%, 50%)");

    view! {
        <div class="participant-avatar" style=format!("background:{bg};border-color:{border};")>
            {initials}
        </div>
    }
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
                    } else if !data.nft_available {
                        set_state.set(ClaimState::NftComingSoon(data));
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
                // Brand header
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">"Proof of Attendance"</div>

                // Title
                <h1 class="claim-title">"Claim Your NFT"</h1>

                <p class="claim-subtitle">
                    "Road to Mainnet #1 — Bangkok"
                </p>
                <p class="claim-event-link">
                    <a href="https://solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/" target="_blank" rel="noopener noreferrer">
                        "solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/"
                    </a>
                </p>
                <div class="powered-badge">
                    <span class="sol-dot"></span>
                    "Powered by Solana"
                </div>

                // Live session timer
                <SessionTimer />

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
                                <div class="claim-error">
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

                        // ---- NFT Coming Soon ----
                        ClaimState::NftComingSoon(data) => {
                            let checked_in_display = format_timestamp(&data.checked_in_at);
                            view! {
                                <div style="width:100%;">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">"Checked in "{checked_in_display}</p>
                                    </div>

                                    // NFT coming soon with shimmer
                                    <div class="claim-nft-soon-card">
                                        <h3>"NFT Badge Coming Soon"</h3>
                                        <p>"Your proof-of-attendance NFT badge is being prepared."</p>
                                        <div class="nft-description">
                                            "You will receive a compressed NFT on Solana — a permanent, on-chain proof that you attended this event."
                                        </div>
                                    </div>

                                    // Event information with timeline
                                    <div class="claim-event-card">
                                        <div class="event-title">"Road to Mainnet #1 — Bangkok"</div>
                                        <div class="event-meta">
                                            <div><strong>"Date: "</strong>"Sunday, 26 April 2026"</div>
                                            <div><strong>"Time: "</strong>"9:30 AM - 1:00 PM (ICT)"</div>
                                            <div><strong>"Venue: "</strong>"ContributeDAO (CDAO), 3rd Floor CP Tower, Phaya Thai"</div>
                                        </div>

                                        <div class="schedule-section">
                                            <div class="schedule-label">"Schedule"</div>
                                            <div class="timeline">
                                                <div class="timeline-item">
                                                    <span class="time-slot">"09:30"</span>" — Registration"
                                                </div>
                                                <div class="timeline-item">
                                                    <span class="time-slot">"10:00"</span>" — Opening & Community Roadmap"
                                                </div>
                                                <div class="timeline-item highlight">
                                                    <span class="time-slot">"10:10"</span>" — Rust, AI & Gaming (Ep. 2) — "<span class="speaker">"Katopz"</span>
                                                </div>
                                                <div class="timeline-item">
                                                    <span class="time-slot">"11:00"</span>" — Group Photo"
                                                </div>
                                                <div class="timeline-item highlight">
                                                    <span class="time-slot">"11:10"</span>" — NFT Engine Workshop — "<span class="speaker">"Golf (ByteCat)"</span>
                                                </div>
                                                <div class="timeline-item highlight">
                                                    <span class="time-slot">"11:40"</span>" — Ephemeral Rollups — "<span class="speaker">"Andy (Magicblock)"</span>
                                                </div>
                                                <div class="timeline-item highlight">
                                                    <span class="time-slot">"11:55"</span>" — APAC Ecosystem Spotlight — "<span class="speaker">"Chaerin (Solana Foundation)"</span>
                                                </div>
                                                <div class="timeline-item">
                                                    <span class="time-slot">"12:10"</span>" — Networking Session"
                                                </div>
                                            </div>
                                        </div>

                                        <a
                                            href="https://solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/"
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-sm"
                                            style="margin-top:0.75rem;width:100%;"
                                        >
                                            "View Full Event Details"
                                        </a>
                                    </div>

                                    // Wallet preparation
                                    <div class="claim-wallet-prep-card">
                                        <p>
                                            "You will need a "
                                            <a href="https://phantom.app/" target="_blank" rel="noopener noreferrer">
                                                "Solana wallet"
                                            </a>
                                            " to claim your NFT badge. Download one before you return."
                                        </p>
                                    </div>

                                    <p class="claim-bookmark-hint">
                                        "Bookmark this page and come back to claim your NFT."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Ready: show wallet input ----
                        ClaimState::Ready(data) => {
                            let checked_in_display = format_timestamp(&data.checked_in_at);
                            view! {
                                <div style="width:100%;">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">"Checked in "{checked_in_display}</p>
                                    </div>

                                    // Wallet input
                                    <div class="card">
                                        <label style="font-size:0.9rem;font-weight:600;color:var(--text-primary);display:block;margin-bottom:0.5rem;">
                                            "Solana Wallet Address"
                                        </label>
                                        <input
                                            class="claim-wallet-input"
                                            type="text"
                                            placeholder="Enter your Solana wallet address"
                                            prop:value=move || wallet_input.get()
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                set_wallet_input.set(val);
                                            }
                                        />
                                        <p style="font-size:0.75rem;color:var(--text-muted);margin-top:0.5rem;">
                                            "Paste your Phantom, Solflare, or Backpack wallet address."
                                        </p>
                                    </div>

                                    // Claim button
                                    <button
                                        class="claim-btn-mint"
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
                                <div class="claim-success">
                                    <div class="success-check">
                                        <svg viewBox="0 0 24 24">
                                            <polyline points="20 6 9 17 4 12"></polyline>
                                        </svg>
                                    </div>
                                    <h2>"NFT Claimed"</h2>
                                    <div class="success-details">
                                        <p><strong>"Name:"</strong>" "{escape_html(&data.name)}</p>
                                        <p><strong>"Wallet:"</strong>
                                            <code>{escape_html(&data.wallet_address)}</code>
                                        </p>
                                        <p><strong>"Claimed:"</strong>" "{format_timestamp(&data.claimed_at)}</p>
                                    </div>
                                    <div class="success-actions">
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
                                <div class="claim-warning">
                                    <ParticipantAvatar name=data.name.clone() />
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
                                <div class="claim-error">
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

                // Fun: hearts reaction widget
                <HeartsWidget />

                // Footer
                <div class="claim-footer">
                    <div class="brand-line">
                        <span class="accent">"BeThere"</span>
                        " x Solana Thailand"
                    </div>
                </div>
            </div>
        </div>
    }
}
