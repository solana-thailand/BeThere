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
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// JS interop
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    /// Copy text to the system clipboard.
    ///
    /// Uses the Clipboard API with a textarea fallback for older browsers.
    /// Returns true if the copy operation was initiated successfully.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
}

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
fn SessionTimer(start_ms: i64, end_ms: i64) -> impl IntoView {
    let event_start_ms = start_ms as f64;
    let event_end_ms = end_ms as f64;

    let (time_display, set_time_display) = signal(String::new());
    let (status_label, set_status_label) = signal(String::new());

    Effect::new(move |_| {
        let set_t = set_time_display;
        let set_s = set_status_label;

        leptos::task::spawn_local(async move {
            loop {
                let now = js_sys::Date::now();
                if now < event_start_ms {
                    let diff = ((event_start_ms - now) / 1000.0) as i64;
                    set_s.set("Starts in".to_string());
                    set_t.set(format_duration(diff));
                } else if now < event_end_ms {
                    let diff = ((now - event_start_ms) / 1000.0) as i64;
                    set_s.set("Live".to_string());
                    set_t.set(format!("+{}", format_duration(diff)));
                } else {
                    set_s.set("Ended".to_string());
                    set_t.set("Thanks for coming!".to_string());
                    break; // stop polling after event ends
                }
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

/// Generative pixel art avatar — 8x8 grid with face features.
/// Deterministic from name hash: each person gets a unique cute face.
#[component]
fn ParticipantAvatar(name: String) -> impl IntoView {
    let hash = simple_hash(&name);

    let skin_hues = [30, 25, 35, 20, 40, 28];
    let skin_hue = skin_hues[(hash % 6) as usize];
    let skin_lightness = 70 + (hash % 15);
    let face_color = format!("hsl({skin_hue}, 60%, {skin_lightness}%)");

    let eye_style = (hash / 6) % 4;
    let mouth_style = (hash / 24) % 4;
    let has_blush = (hash / 96) % 3 == 0;
    let bg_hue = (hash / 288) % 360;
    let bg_color = format!("hsl({bg_hue}, 50%, 25%)");

    let grid = build_face_grid(eye_style, mouth_style, has_blush);

    let svg_cells = grid.iter().enumerate().flat_map(|(row, cells)| {
        let fc = face_color.clone();
        cells.iter().enumerate().filter_map(move |(col, &cell)| {
            if cell == 0 { return None; }
            let color = match cell {
                1 => fc.clone(),
                2 => "#1a1a2e".to_string(),
                3 => "#e74c3c".to_string(),
                4 => "rgba(255,150,150,0.6)".to_string(),
                _ => "#333".to_string(),
            };
            Some(format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\" fill=\"{color}\" rx=\"0.15\"/>",
                x = col,
                y = row,
                color = color
            ))
        }).collect::<Vec<_>>()
    }).collect::<Vec<_>>().join("");

    view! {
        <div class="participant-avatar-pixel" style=format!("background:{bg_color};")>
            <svg viewBox="0 0 8 8" width="56" height="56" style="image-rendering:pixelated;" inner_html=svg_cells></svg>
        </div>
    }
}

/// NFT badge preview placeholder — shows a stylized mystery badge card
/// until real NFT artwork is uploaded. Pure CSS/SVG, no external image.
#[component]
fn NftBadgePreview() -> impl IntoView {
    view! {
        <div class="nft-preview-card">
            <div class="nft-preview-badge">
                <svg viewBox="0 0 80 80" width="80" height="80">
                    // Outer hexagon
                    <polygon
                        points="40,4 72,22 72,58 40,76 8,58 8,22"
                        fill="none"
                        stroke="rgba(99,102,241,0.4)"
                        stroke-width="1.5"
                    />
                    // Inner diamond
                    <polygon
                        points="40,16 60,40 40,64 20,40"
                        fill="rgba(99,102,241,0.08)"
                        stroke="rgba(99,102,241,0.25)"
                        stroke-width="1"
                    />
                    // Center star
                    <circle cx="40" cy="40" r="6" fill="rgba(99,102,241,0.5)" />
                    <circle cx="40" cy="40" r="3" fill="rgba(129,140,248,0.8)" />
                </svg>
            </div>
            <div class="nft-preview-info">
                <div class="nft-preview-title">"Proof of Attendance"</div>
                <div class="nft-preview-sub">"Compressed NFT on Solana"</div>
            </div>
        </div>
    }
}

/// Build an 8x8 face grid with symmetric features.
fn build_face_grid(eye_style: u32, mouth_style: u32, has_blush: bool) -> [[u8; 8]; 8] {
    let mut grid: [[u8; 8]; 8] = [
        [0,0,1,1,1,1,0,0],
        [0,1,1,1,1,1,1,0],
        [1,1,1,1,1,1,1,1],
        [1,1,1,1,1,1,1,1],
        [1,1,1,1,1,1,1,1],
        [1,1,1,1,1,1,1,1],
        [0,1,1,1,1,1,1,0],
        [0,0,1,1,1,1,0,0],
    ];

    // Eyes (symmetric)
    match eye_style {
        0 => { grid[3][2] = 2; grid[3][5] = 2; }           // dot eyes
        1 => { grid[2][2] = 2; grid[2][5] = 2; grid[3][2] = 2; grid[3][5] = 2; } // tall eyes
        2 => { grid[3][2] = 2; grid[3][3] = 2; grid[3][4] = 2; grid[3][5] = 2; } // wide eyes
        _ => { grid[2][2] = 2; grid[3][3] = 2; grid[2][5] = 2; grid[3][4] = 2; } // anime eyes
    }

    // Mouth (centered)
    match mouth_style {
        0 => { grid[5][3] = 3; grid[5][4] = 3; }           // small smile
        1 => { grid[5][2] = 3; grid[5][3] = 3; grid[5][4] = 3; grid[5][5] = 3; } // wide smile
        2 => { grid[5][3] = 3; grid[5][4] = 3; grid[6][3] = 3; grid[6][4] = 3; } // open mouth
        _ => { grid[4][4] = 3; grid[5][3] = 3; grid[5][4] = 3; } // smirk
    }

    // Blush
    if has_blush {
        grid[4][1] = 4;
        grid[4][6] = 4;
    }

    grid
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

    // Dynamic event config (fetched from backend, replaces hardcoded values)
    let (evt_name, set_evt_name) = signal(String::new());
    let (evt_tagline, set_evt_tagline) = signal(String::new());
    let (evt_link, set_evt_link) = signal(String::new());
    let (evt_start, set_evt_start) = signal(0i64);
    let (evt_end, set_evt_end) = signal(0i64);

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
                    // Set dynamic event config from backend
                    set_evt_name.set(data.event.event_name.clone());
                    set_evt_tagline.set(data.event.event_tagline.clone());
                    set_evt_link.set(data.event.event_link.clone());
                    set_evt_start.set(data.event.event_start_ms);
                    set_evt_end.set(data.event.event_end_ms);

                    if data.claimed {
                        set_state.set(ClaimState::AlreadyClaimed(data));
                    } else if !data.nft_available {
                        set_state.set(ClaimState::NftComingSoon(data));
                    } else {
                        // Pre-fill wallet if locked to a pre-registered address
                        if let Some(ref wallet) = data.locked_wallet {
                            if !wallet.is_empty() {
                                set_wallet_input.set(wallet.clone());
                            }
                        }
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

    // One-tap paste from clipboard — big mobile UX win
    let handle_paste = move |_| {
        let set_w = set_wallet_input.clone();
        leptos::task::spawn_local(async move {
            if let Ok(promise_val) = js_sys::eval(
                "navigator.clipboard ? navigator.clipboard.readText() : Promise.resolve('')"
            ) {
                let promise = js_sys::Promise::from(promise_val);
                if let Ok(val) = js_sys::futures::JsFuture::from(promise).await {
                    if let Some(text) = val.as_string() {
                        let trimmed: String = text.trim().to_string();
                        if !trimmed.is_empty() {
                            set_w.set(trimmed);
                        }
                    }
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
                    {move || evt_name.get()}
                </p>
                <p class="claim-tagline">
                    {move || evt_tagline.get()}
                </p>
                <p class="claim-event-link">
                    <a href=move || evt_link.get() target="_blank" rel="noopener noreferrer">
                        {move || evt_link.get()}
                    </a>
                </p>

                // Live session timer (reactive — waits for event config from backend)
                {move || {
                    let start = evt_start.get();
                    let end = evt_end.get();
                    if start > 0 && end > 0 {
                        view! { <SessionTimer start_ms=start end_ms=end /> }.into_any()
                    } else {
                        view! { <div class="session-timer"></div> }.into_any()
                    }
                }}

                // State-dependent rendering
                {move || {
                    match state.get() {
                        // ---- Loading ----
                        ClaimState::Loading => {
                            view! {
                                <div style="width:100%;">
                                    // Shimmer: welcome card (avatar + 2 text lines)
                                    <div class="shimmer-card" style="display:flex;align-items:center;gap:1rem;margin-bottom:1rem;">
                                        <div class="shimmer shimmer-avatar" style="flex-shrink:0;"></div>
                                        <div style="flex:1;display:flex;flex-direction:column;gap:0.5rem;">
                                            <div class="shimmer shimmer-line" style="width:60%;"></div>
                                            <div class="shimmer shimmer-line-sm" style="width:40%;"></div>
                                        </div>
                                    </div>

                                    // Shimmer: NFT preview card (square + 2 text lines)
                                    <div class="shimmer-card" style="display:flex;align-items:center;gap:1rem;margin-bottom:1rem;">
                                        <div class="shimmer" style="width:72px;height:72px;border-radius:12px;flex-shrink:0;"></div>
                                        <div style="flex:1;display:flex;flex-direction:column;gap:0.5rem;">
                                            <div class="shimmer shimmer-line" style="width:75%;"></div>
                                            <div class="shimmer shimmer-line-sm" style="width:50%;"></div>
                                        </div>
                                    </div>

                                    // Shimmer: wallet input card (label + input bar + hint)
                                    <div class="shimmer-card" style="margin-bottom:1rem;">
                                        <div class="shimmer shimmer-line-sm" style="width:40%;margin-bottom:0.75rem;"></div>
                                        <div class="shimmer shimmer-line" style="width:100%;height:42px;border-radius:8px;margin-bottom:0.5rem;"></div>
                                        <div class="shimmer shimmer-line-sm" style="width:55%;"></div>
                                    </div>

                                    // Shimmer: claim button
                                    <div class="shimmer shimmer-btn" style="width:100%;"></div>
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

                                    // NFT badge preview
                                    <NftBadgePreview />

                                    // NFT coming soon with shimmer
                                    <div class="claim-nft-soon-card">
                                        <h3>"NFT Badge Coming Soon"</h3>
                                        <p>"Your proof-of-attendance NFT badge is being prepared."</p>
                                        <div class="nft-description">
                                            "You will receive a compressed NFT on Solana — a permanent, on-chain proof that you attended this event."
                                        </div>
                                    </div>

                                    // Compact wallet hint
                                    <p class="claim-bookmark-hint">
                                        "Get a "
                                        <a href="https://phantom.app/" target="_blank" rel="noopener noreferrer">"Solana wallet"</a>
                                        " ready — bookmark this page to claim your NFT later."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Ready: show wallet input ----
                        ClaimState::Ready(data) => {
                            let checked_in_display = format_timestamp(&data.checked_in_at);
                            let locked_wallet = data.locked_wallet.clone();
                            let locked_wallet_hint = data.locked_wallet.clone();
                            view! {
                                <div style="width:100%;">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">"Checked in "{checked_in_display}</p>
                                    </div>

                                    // NFT badge preview
                                    <NftBadgePreview />

                                    // Wallet input
                                    <div class="card">
                                        <label style="font-size:0.9rem;font-weight:600;color:var(--text-primary);display:block;margin-bottom:0.5rem;">
                                            "Solana Wallet Address"
                                        </label>
                                        // Locked wallet indicator — shown when pre-registered wallet exists
                                        {move || {
                                            match &locked_wallet {
                                                Some(w) if !w.is_empty() => view! {
                                                    <div class="claim-wallet-locked">
                                                        <span style="color:var(--accent);margin-right:0.25rem;">"Locked"</span>
                                                        " — this claim is tied to your pre-registered wallet"
                                                    </div>
                                                }.into_any(),
                                                _ => view! { <div></div> }.into_any(),
                                            }
                                        }}
                                        <div style="display:flex;gap:0.5rem;">
                                            <input
                                                class="claim-wallet-input"
                                                type="text"
                                                placeholder="Enter your Solana wallet address"
                                                prop:value=move || wallet_input.get()
                                                on:input=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    set_wallet_input.set(val);
                                                }
                                                style="flex:1;min-width:0;"
                                            />
                                            <button
                                                class="claim-paste-btn"
                                                on:click=handle_paste
                                                type="button"
                                            >
                                                "Paste"
                                            </button>
                                        </div>
                                        <p style="font-size:0.75rem;color:var(--text-muted);margin-top:0.5rem;">
                                            {move || {
                                                match &locked_wallet_hint {
                                                    Some(w) if !w.is_empty() => "Use the pre-filled wallet address to claim.".into_any(),
                                                    _ => "Tap Paste or type your Phantom, Solflare, or Backpack address.".into_any(),
                                                }
                                            }}
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
                                <div style="width:100%;display:flex;flex-direction:column;align-items:center;gap:1rem;padding:1.5rem 0;">
                                    // Pulsing minting indicator
                                    <div style="position:relative;width:64px;height:64px;">
                                        <div class="shimmer" style="width:64px;height:64px;border-radius:50%;position:absolute;top:0;left:0;"></div>
                                        <span class="spinner spinner-lg" style="position:absolute;top:50%;left:50%;transform:translate(-50%,-50%);"></span>
                                    </div>
                                    <h3 style="color:var(--text-primary);font-weight:600;">"Minting your NFT..."</h3>
                                    <p style="font-size:0.9rem;color:var(--text-secondary);">
                                        "Minting for "{escape_html(&data.name)}
                                    </p>
                                    <p style="font-size:0.8rem;color:var(--text-muted);">
                                        "This usually takes 3-5 seconds."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Success! ----
                        ClaimState::Success(data) => {
                            let cluster_param = if data.cluster == "mainnet-beta" {
                                String::new()
                            } else {
                                format!("?cluster={}", data.cluster)
                            };
                            let explorer_url = format!(
                                "https://solscan.io/tx/{}{cluster_param}",
                                data.signature
                            );
                            let asset_url = format!(
                                "https://solscan.io/token/{}{cluster_param}",
                                data.asset_id
                            );
                            let asset_id_display = {
                                let id = &data.asset_id;
                                if id.len() > 12 {
                                    format!("{}...{}", &id[..6], &id[id.len()-4..])
                                } else {
                                    id.clone()
                                }
                            };
                            let asset_id_full = data.asset_id.clone();
                            let share_url = asset_url.clone();
                            view! {
                                <div class="claim-success">
                                    <div class="success-check">
                                        <svg viewBox="0 0 24 24">
                                            <polyline points="20 6 9 17 4 12"></polyline>
                                        </svg>
                                    </div>
                                    <h2>"NFT Claimed"</h2>

                                    // Asset ID card
                                    <div class="claim-asset-card">
                                        <div class="claim-asset-header">
                                            <span class="claim-asset-label">"Asset ID"</span>
                                            <span class="claim-asset-status">
                                                <span class="claim-asset-status-dot"></span>
                                                "On-Chain"
                                            </span>
                                        </div>
                                        <div class="claim-asset-value-row">
                                            <span class="claim-asset-code">{asset_id_display}</span>
                                            <button
                                                class="claim-copy-btn"
                                                type="button"
                                                title="Copy Asset ID"
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
                                        <a
                                            href=share_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-block"
                                            style="border-color:var(--accent);color:var(--accent);"
                                        >
                                            "Share your NFT"
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

                // Fun: hearts reaction widget (only on loaded/engaged states)
                {move || {
                    match state.get() {
                        ClaimState::NftComingSoon(_) |
                        ClaimState::Ready(_) |
                        ClaimState::Success(_) |
                        ClaimState::AlreadyClaimed(_) => {
                            view! { <HeartsWidget /> }.into_any()
                        }
                        _ => view! { <div></div> }.into_any()
                    }
                }}

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
