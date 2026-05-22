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

use crate::api::{self, AdventureStatusType, ClaimLookupData, ClaimMintData, QuizQuestionsData, QuizStatus, QuizSubmitData, RefundTxRequest};
use crate::components::{self, Toast, ToastType};
use crate::icons::{Icon, IconName, wallet_icon_name};
use crate::utils::{escape_html, format_timestamp, get_cluster, metaplex_explorer_url, solanafm_asset_url, solscan_tx_url};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Quiz answers: question_id → selected option text.
type QuizAnswers = std::collections::HashMap<String, String>;

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

#[wasm_bindgen(module = "/js/confetti.js")]
extern "C" {
    /// Launch a burst of festive confetti particles across the viewport.
    #[wasm_bindgen(js_name = "launchConfetti")]
    fn launch_confetti();
}

// ===== Navigation JS Interop =====
// Uses wasm_bindgen module imports from /js/navigation.js instead of js_sys::eval().
#[wasm_bindgen(module = "/js/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = "readClipboardText")]
    fn read_clipboard_text_js() -> js_sys::Promise;
}

// ---------------------------------------------------------------------------
// JS interop — Solana wallet adapter (shared with deposit.rs)
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "isWalletAvailable")]
    fn is_wallet_available_js(wallet_name: &str) -> bool;
}

async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> crate::wallet_error::WalletResult {
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

// ---------------------------------------------------------------------------
// Inline refund state machine (lightweight version of deposit.rs refund)
// ---------------------------------------------------------------------------

/// Simplified refund state for the inline claim-page flow.
#[derive(Clone, Debug)]
enum ClaimRefundState {
    /// Initial — no action taken yet.
    Idle,
    /// Wallet connection in progress.
    Connecting,
    /// Wallet connected, TX signing in progress.
    Signing(String, String), // (wallet_name, public_key)
    /// Refund TX confirmed on-chain.
    Confirmed(String), // tx_signature
    /// Error occurred (message shown inline).
    Error(String),
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
    /// Quiz required — attendee must complete quiz before claiming.
    /// Holds claim data + fetched quiz questions.
    Quiz(ClaimLookupData, QuizQuestionsData),
    /// Quiz submitted — showing results. If passed, transition to Ready.
    QuizSubmitted(ClaimLookupData, QuizQuestionsData, QuizSubmitData),
    /// Adventure required — attendee must complete adventure before claiming.
    /// Holds claim data + adventure status.
    Adventure(ClaimLookupData, AdventureStatusType),
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

/// Check if participation type indicates an online attendee.
fn is_online_participant(participation_type: &str) -> bool {
    let lower = participation_type.trim().to_lowercase();
    lower.contains("online")
}

/// Build the appropriate label for check-in status.
/// For online attendees without check-in: "Registered".
/// For checked-in attendees: "Checked in {timestamp}".
/// For others without check-in: "Not yet checked in".
fn checked_in_label(checked_in_at: &str, participation_type: &str) -> String {
    if checked_in_at.is_empty() || checked_in_at == "N/A" {
        if is_online_participant(participation_type) {
            return "Registered".to_string();
        }
        return "Not yet checked in".to_string();
    }
    format!("Checked in {}", format_timestamp(checked_in_at))
}

// ---------------------------------------------------------------------------
// Progress Stepper
// ---------------------------------------------------------------------------

/// Determines the current step number for the claim flow progress indicator.
/// Returns (current_step, total_steps) where steps are 1-indexed.
fn claim_step(state: &ClaimState) -> (usize, usize) {
    match state {
        // Loading = step 0 (before flow starts)
        ClaimState::Loading => (0, 3),
        // Step 1: Verified (attendee found + checked in)
        ClaimState::NotFound(_) => (0, 3),
        ClaimState::NftComingSoon(_) => (1, 3),
        // Step 2: Quiz (if required)
        ClaimState::Quiz(_, _) | ClaimState::QuizSubmitted(_, _, _) => (2, 3),
        // Step 2: Adventure gate (alternative to quiz)
        ClaimState::Adventure(_, _) => (2, 3),
        // Step 3: Claim (enter wallet + mint)
        ClaimState::Ready(_) => (3, 3),
        ClaimState::Minting(_) => (3, 3),
        // Completed
        ClaimState::Success(_) => (4, 3),
        ClaimState::AlreadyClaimed(_) => (4, 3),
        ClaimState::MintError(_, _) => (3, 3),
    }
}

/// Progress stepper for the claim flow.
/// Shows: Verified → Quiz → Claim NFT
/// The Quiz step is only shown when relevant.
#[component]
fn ClaimStepper(current: usize, total: usize, show_quiz: bool) -> impl IntoView {
    // Build step labels based on whether quiz is shown
    let _ = total; // used for context, steps are hardcoded
    let steps: Vec<(&'static str, &'static str, usize)> = if show_quiz {
        vec![
            ("✓", "Verified", 1),
            ("?", "Quiz", 2),
            ("", "Claim", 3),
        ]
    } else {
        vec![
            ("✓", "Verified", 1),
            ("", "Claim", 2),
        ]
    };

    view! {
        <div class="claim-stepper">
            <div class="claim-stepper-track">
                {steps.into_iter().map(|(icon, label, step_num)| {
                    let is_completed = current > step_num;
                    let is_current = current == step_num;

                    let circle_class = match (is_completed, is_current) {
                        (true, _) => "claim-step-circle completed",
                        (_, true) => "claim-step-circle current",
                        _ => "claim-step-circle upcoming",
                    };

                    view! {
                        <div class="claim-step">
                            <div class=circle_class>
                                {if is_completed {
                                    view! {
                                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" style="width:14px;height:14px;">
                                            <polyline points="20 6 9 17 4 12"></polyline>
                                        </svg>
                                    }.into_any()
                                } else {
                                    view! { <span>{icon}</span> }.into_any()
                                }}
                            </div>
                            <span class=if is_current || is_completed { "claim-step-label active" } else { "claim-step-label" }>
                                {label}
                            </span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
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
                // 5s interval — reduces re-renders vs 1s; sufficient granularity
                // for countdown/elapsed display on event time scales (hours).
                gloo::timers::future::TimeoutFuture::new(5000).await;
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
    let has_blush = (hash / 96).is_multiple_of(3);
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
            <svg viewBox="0 0 8 8" width="56" height="56" class="claim-avatar-svg" inner_html=svg_cells></svg>
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

// Claim page component — public route at `/claim/:token`.
//
// Attendees scan their claim QR code (or follow the claim URL) to land here.
// ---------------------------------------------------------------------------
// Quiz rendering helpers (extracted to avoid nested view! macro issues)
// ---------------------------------------------------------------------------

/// Build quiz question cards as pre-rendered views.
fn build_quiz_questions(
    questions: &[crate::api::QuizQuestionPublic],
    total_q: usize,
    quiz_answers: ReadSignal<QuizAnswers>,
    set_quiz_answers: WriteSignal<QuizAnswers>,
) -> Vec<AnyView> {
    questions.iter().enumerate().map(|(idx, q)| {
        let q_id = q.id.clone();
        let q_text = q.text.clone();
        let q_num = idx + 1;
        let options = q.options.clone();

        let option_views: Vec<AnyView> = options.iter().map(|opt| {
            let opt_display = opt.clone();
            // Clones for class:dyn closure
            let qid_c = q_id.clone();
            let opt_c = opt.clone();
            let qa_c = quiz_answers;
            // Clones for radio closure
            let qid_r = q_id.clone();
            let opt_r = opt.clone();
            let qa_r = quiz_answers;
            // Clones for click handler
            let qid_click = q_id.clone();
            let opt_click = opt.clone();
            let set_qa = set_quiz_answers;
            let qa_click = quiz_answers;

            view! {
                <button
                    class="claim-quiz-opt"
                    class:claim-quiz-opt-selected=move || qa_c.get().get(&qid_c).map(|s| s == &opt_c).unwrap_or(false)
                    on:click=move |_| {
                        let mut answers = qa_click.get();
                        answers.insert(qid_click.clone(), opt_click.clone());
                        set_qa.set(answers);
                    }
                >
                    <span class="claim-quiz-opt-radio">
                        {move || match qa_r.get().get(&qid_r).map(|s| s == &opt_r).unwrap_or(false) {
                            true => "●",
                            _ => "○",
                        }}
                    </span>
                    <span>{opt_display}</span>
                </button>
            }.into_any()
        }).collect();

        view! {
            <div class="card claim-quiz-question">
                <div class="claim-quiz-q-header">
                    <span class="claim-quiz-q-num">{format!("{q_num}")}</span>
                    <span class="claim-quiz-q-of">"of "{total_q}</span>
                </div>
                <p class="claim-quiz-q-text">{q_text}</p>
                <div class="claim-quiz-options">{option_views}</div>
            </div>
        }.into_any()
    }).collect()
}

/// Build quiz explanation cards as a pre-rendered view.
fn build_quiz_explanations(
    explanations: &[crate::api::QuestionExplanation],
    questions: &[crate::api::QuizQuestionPublic],
) -> AnyView {
    if explanations.is_empty() {
        return view! { <div></div> }.into_any();
    }

    let items: Vec<AnyView> = explanations.iter().enumerate().map(|(idx, exp)| {
        let q_text = questions.iter().find(|q| q.id == exp.question_id)
            .map(|q| q.text.clone())
            .unwrap_or_default();
        let icon = match exp.correct { true => "✓", _ => "✗" };
        let exp_class = match exp.correct {
            true => "claim-quiz-exp-correct",
            _ => "claim-quiz-exp-wrong",
        };
        let exp_text = exp.explanation.clone();
        let num = idx + 1;

        view! {
            <div class="claim-quiz-exp-item">
                <div class="claim-quiz-exp-header">
                    <span class=exp_class>{icon}</span>
                    <span class="claim-quiz-exp-q">{format!("{num}. {q_text}")}</span>
                </div>
                {match exp_text {
                    Some(t) => view! { <p class="claim-quiz-exp-text">{t}</p> }.into_any(),
                    None => view! { <div></div> }.into_any(),
                }}
            </div>
        }.into_any()
    }).collect();

    view! {
        <div class="card claim-quiz-explanations">
            <h4>"Answer Review"</h4>
            {items}
        </div>
    }.into_any()
}

/// Allowed quiz actions after submission.
enum QuizAction {
    Passed,
    Retry,
    Exhausted,
}

/// Build the action section for quiz results (wallet+claim, retry, or exhausted).
/// Extracted to avoid nested view! macros inside conditional blocks.
#[allow(clippy::too_many_arguments)]
fn build_quiz_action(
    action: QuizAction,
    claim_data_for_claim: ClaimLookupData,
    claim_data_for_retry: ClaimLookupData,
    quiz_data_for_retry: crate::api::QuizQuestionsData,
    set_quiz_answers: WriteSignal<QuizAnswers>,
    _wallet_input: ReadSignal<String>,
    set_wallet_input: WriteSignal<String>,
    locked_wallet: Option<String>,
    _locked_wallet_hint: Option<String>,
    _handle_paste: impl Fn(leptos::ev::MouseEvent) + Clone + 'static,
    claim_token: String,
    set_state: WriteSignal<ClaimState>,
) -> AnyView {
    match action {
        QuizAction::Passed => {
            let claim_data_c = claim_data_for_claim;
            let lw = locked_wallet;
            let set_wi = set_wallet_input;
            let token = claim_token;
            let ss = set_state;

            // After quiz passes, check adventure gate before showing wallet input
            let check_adventure_and_proceed = move || {
                let claim_data_adv = claim_data_c.clone();
                let token_adv = token.clone();
                let set_wi_c = set_wi.clone();
                let lw_c = lw.clone();
                let ss_c = ss.clone();
                leptos::task::spawn_local(async move {
                    match api::get_adventure_status(&token_adv).await {
                        Ok(status_data) => {
                            match status_data.status {
                                AdventureStatusType::NotRequired | AdventureStatusType::Passed => {
                                    // Pre-fill locked wallet before going to Ready
                                    if let Some(ref wallet) = lw_c
                                        && !wallet.is_empty()
                                    {
                                        set_wi_c.set(wallet.clone());
                                    }
                                    ss_c.set(ClaimState::Ready(claim_data_adv));
                                }
                                AdventureStatusType::NotStarted | AdventureStatusType::InProgress => {
                                    log::info!("[claim] quiz passed but adventure required, showing adventure gate");
                                    ss_c.set(ClaimState::Adventure(
                                        claim_data_adv,
                                        status_data.status,
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[claim] failed to check adventure status after quiz: {e}, proceeding to Ready");
                            if let Some(ref wallet) = lw_c
                                && !wallet.is_empty()
                            {
                                set_wi_c.set(wallet.clone());
                            }
                            ss_c.set(ClaimState::Ready(claim_data_adv));
                        }
                    }
                });
            };

            view! {
                // NFT badge preview
                <NftBadgePreview />

                <div class="card claim-quiz-adventure-check">
                    <p class="claim-quiz-passed-msg"><Icon icon=IconName::Check class="icon-sm icon-success" />" Quiz passed! Verifying adventure progress..."</p>
                </div>

                <button
                    class="claim-btn-mint"
                    on:click=move |_| {
                        check_adventure_and_proceed();
                    }
                >
                    "Continue to Claim"
                </button>
            }.into_any()
        }
        QuizAction::Retry => {
            let claim_d = claim_data_for_retry;
            let quiz_d = quiz_data_for_retry;
            view! {
                <button
                    class="claim-btn-mint claim-quiz-retry-btn"
                    on:click=move |_| {
                        set_quiz_answers.set(QuizAnswers::new());
                        set_state.set(ClaimState::Quiz(claim_d.clone(), quiz_d.clone()));
                    }
                >
                    "Try Again"
                </button>
            }.into_any()
        }
        QuizAction::Exhausted => {
            view! {
                <div class="card claim-quiz-exhausted">
                    <p>"You've used all your attempts. Please contact event staff for assistance."</p>
                </div>
            }.into_any()
        }
    }
}

// ---------------------------------------------------------------------------
// Extracted quiz components (reduce nesting in main view! macro)
// ---------------------------------------------------------------------------

/// Quiz view — handles the ClaimState::Quiz state.
/// Extracted from the main Claim component to avoid the unclosed delimiter
/// caused by deeply nested view! macro content.
#[component]
fn QuizView(
    claim_data: ClaimLookupData,
    quiz_data: QuizQuestionsData,
    quiz_answers: ReadSignal<QuizAnswers>,
    set_quiz_answers: WriteSignal<QuizAnswers>,
    set_state: WriteSignal<ClaimState>,
) -> impl IntoView {
    let checked_in_display = checked_in_label(&claim_data.checked_in_at, &claim_data.participation_type);
    let total_q = quiz_data.questions.len();
    let answered = move || quiz_answers.get().len();
    let all_answered = move || quiz_answers.get().len() == total_q;
    let passing = quiz_data.passing_score_percent;
    let max_att = quiz_data.max_attempts;
    let questions_clone = quiz_data.questions.clone();
    let attempts_label = format!("{max_att} attempt{}", if max_att != 1 { "s" } else { "" });
    let claim_token = claim_data.claim_token.clone();

    // Pre-render question cards to avoid nested view! macro issues
    let question_views = build_quiz_questions(
        &quiz_data.questions, total_q, quiz_answers, set_quiz_answers,
    );

    view! {
        <div class="claim-state-full">
            // Attendee welcome
            <div class="claim-welcome-card">
                <ParticipantAvatar name=claim_data.name.clone() />
                <h3>"Welcome, "{escape_html(&claim_data.name)}"!"</h3>
                <p class="checked-in-label">{checked_in_display}</p>
            </div>

            // Quiz intro card
            <div class="card claim-quiz-intro">
                <div class="claim-quiz-icon">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="10"></circle>
                        <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"></path>
                        <line x1="12" y1="17" x2="12.01" y2="17"></line>
                    </svg>
                </div>
                <h3>"Complete the Quiz"</h3>
                <p class="claim-quiz-desc">
                    "Answer all questions to unlock your badge. You need "
                    <strong>{passing}"%"</strong>" correct to pass."
                </p>
                <p class="claim-quiz-meta">
                    <span>{total_q}" questions"</span>
                    <span class="claim-quiz-sep">"·"</span>
                    <span>{attempts_label}</span>
                </p>
            </div>

            // Questions (pre-rendered to avoid nested view! macro)
            {question_views}

            // Submit button
            <button
                class="claim-btn-mint claim-quiz-submit"
                disabled=move || !all_answered()
                on:click=move |_| {
                    let answers_map = quiz_answers.get();
                    let answers_vec: Vec<crate::api::QuizAnswer> = questions_clone.iter().filter_map(|q| {
                        answers_map.get(&q.id).map(|text| crate::api::QuizAnswer {
                            question_id: q.id.clone(),
                            selected_text: text.clone(),
                        })
                    }).collect();
                    let token = claim_token.clone();
                    let claim_data_c = claim_data.clone();
                    let quiz_data_c = quiz_data.clone();
                    leptos::task::spawn_local(async move {
                        match api::submit_quiz(&token, &answers_vec).await {
                            Ok(result) => {
                                if result.passed {
                                    log::info!("[quiz] passed! score={}%", result.score_percent);
                                } else {
                                    log::info!("[quiz] not passed. score={}%, attempts remaining={}", result.score_percent, result.remaining_attempts);
                                }
                                set_state.set(ClaimState::QuizSubmitted(claim_data_c, quiz_data_c, result));
                            }
                            Err(e) => {
                                log::error!("[quiz] submit failed: {e}");
                            }
                        }
                    });
                }
            >
                "Submit Answers"
                <span class="claim-quiz-submit-count">
                    "("{answered}"/"{total_q}")"
                </span>
            </button>
        </div>
    }
}

/// Quiz submitted view — handles the ClaimState::QuizSubmitted state.
/// Extracted from the main Claim component to avoid the unclosed delimiter
/// caused by deeply nested view! macro content.
#[component]
fn QuizSubmittedView(
    claim_data: ClaimLookupData,
    quiz_data: QuizQuestionsData,
    submit_result: QuizSubmitData,
    set_quiz_answers: WriteSignal<QuizAnswers>,
    wallet_input: ReadSignal<String>,
    set_wallet_input: WriteSignal<String>,
    set_state: WriteSignal<ClaimState>,
) -> impl IntoView {
    let checked_in_display = checked_in_label(&claim_data.checked_in_at, &claim_data.participation_type);
    let passed = submit_result.passed;
    let score = submit_result.score_percent;
    let remaining = submit_result.remaining_attempts;
    let correct = submit_result.correct_count;
    let total_q = submit_result.total_questions;
    let result_class = match passed {
        true => "card claim-quiz-result claim-quiz-passed",
        false => "card claim-quiz-result claim-quiz-failed",
    };
    let locked_wallet = claim_data.locked_wallet.clone();
    let locked_wallet_hint = claim_data.locked_wallet.clone();
    let quiz_data_for_retry = quiz_data.clone();
    let claim_data_for_retry = claim_data.clone();
    let claim_data_for_claim = claim_data.clone();
    let claim_token = claim_data.claim_token.clone();
    let retry_info = match remaining {
        0 => "No attempts remaining. Contact event staff for help.".to_string(),
        n => format!("{n} attempt{} left.", if n != 1 { "s" } else { "" }),
    };
    let score_label = format!("{score}% — {correct} of {total_q} correct");
    let action = match passed {
        true => QuizAction::Passed,
        false if remaining > 0 => QuizAction::Retry,
        false => QuizAction::Exhausted,
    };

    // One-tap paste from clipboard — recreated for this component
    let handle_paste = move |_| {
        let set_w = set_wallet_input;
        leptos::task::spawn_local(async move {
            let promise = read_clipboard_text_js();
            if let Ok(val) = js_sys::futures::JsFuture::from(promise).await
                && let Some(text) = val.as_string()
            {
                let trimmed: String = text.trim().to_string();
                if !trimmed.is_empty() {
                    set_w.set(trimmed);
                }
            }
        });
    };

    // Pre-build conditional views outside view! macro to avoid delimiter counting issues
    let result_icon: AnyView = match passed {
        true => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <polyline points="16 9 10.5 14.5 8 12"></polyline>
            </svg>
        }.into_any(),
        false => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="var(--warning)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="15" y1="9" x2="9" y2="15"></line>
                <line x1="9" y1="9" x2="15" y2="15"></line>
            </svg>
        }.into_any(),
    };

    let result_title: &str = match passed { true => "Quiz Passed!", false => "Not Quite..." };

    let retry_info_view: AnyView = match passed {
        true => view! { <div></div> }.into_any(),
        false => view! {
            <p class="claim-quiz-retry-info">{retry_info}</p>
        }.into_any(),
    };

    let explanations_view = build_quiz_explanations(&submit_result.explanations, &quiz_data.questions);

    let action_view = build_quiz_action(
        action,
        claim_data_for_claim,
        claim_data_for_retry,
        quiz_data_for_retry,
        set_quiz_answers,
        wallet_input,
        set_wallet_input,
        locked_wallet,
        locked_wallet_hint,
        handle_paste,
        claim_token,
        set_state,
    );

    view! {
        <div class="claim-state-full">
            // Attendee welcome
            <div class="claim-welcome-card">
                <ParticipantAvatar name=claim_data.name.clone() />
                <h3>"Welcome, "{escape_html(&claim_data.name)}"!"</h3>
                <p class="checked-in-label">{checked_in_display}</p>
            </div>

            // Quiz result card
            <div class=result_class>
                <div class="claim-quiz-result-icon">
                    {result_icon}
                </div>
                <h3>{result_title}</h3>
                <div class="claim-quiz-score">
                    <span class="claim-quiz-score-num">{format!("{score}")}</span>
                    <span class="claim-quiz-score-pct">"%"</span>
                </div>
                <p class="claim-quiz-score-detail">{score_label}</p>
                {retry_info_view}
            </div>

            // Explanations (pre-rendered to avoid nested view! macro)
            {explanations_view}

            // Actions: retry or proceed to claim
            {action_view}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Deposit info component (shown after NFT claim)
// ---------------------------------------------------------------------------

/// Renders a deposit info card with link to the deposit page.
/// Shown on the claim page when the event has deposits enabled.
#[component]
fn DepositRefundSection(
    api_id: String,
    event_id: String,
    deposit_amount_usdc: u64,
    #[prop(default = 0)] deposit_amount_thb: u64,
    /// Whether the attendee has a verified USDC deposit on-chain.
    #[prop(default = false)]
    has_usdc_deposit: bool,
    /// Whether the attendee has already claimed their NFT (changes messaging).
    #[prop(default = false)]
    has_claimed: bool,
) -> impl IntoView {
    let usdc_display = format!("{:.2}", deposit_amount_usdc as f64 / 1_000_000.0);
    let deposit_link = if event_id.is_empty() {
        format!("/deposit/{api_id}")
    } else {
        format!("/deposit/{api_id}?event_id={event_id}")
    };

    // Inline refund state
    let (refund_state, set_refund_state) = signal(ClaimRefundState::Idle);
    let (toast, set_toast) = signal(None::<crate::components::ToastMessage>);

    // Fetch cluster once for Solscan links
    let cluster = get_cluster();

    // Store the refund handler in StoredValue so reactive closures can clone it
    let api_id_stored = StoredValue::new(api_id.clone());
    let event_id_stored = StoredValue::new(event_id.clone());

    let do_refund = move |wallet_name: String| {
        let wn = wallet_name.clone();
        let api_id_for_tx = api_id_stored.get_value();
        let event_id_for_tx = event_id_stored.get_value();

        set_refund_state.set(ClaimRefundState::Connecting);
        leptos::task::spawn_local(async move {
            // Step 1: Connect wallet
            let pubkey = match connect_wallet_js(&wn).await {
                crate::wallet_error::WalletResult::Success(pk) => pk,
                crate::wallet_error::WalletResult::Error(e) => {
                    let msg = crate::wallet_error::user_friendly_message(&e);
                    log::error!("[claim-refund] wallet connect error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(&set_toast, &msg, ToastType::Error);
                    set_refund_state.set(ClaimRefundState::Error(msg));
                    return;
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    let msg = "Failed to connect wallet. Please try again.";
                    log::error!("[claim-refund] wallet connect failed");
                    components::show_toast(&set_toast, msg, ToastType::Error);
                    set_refund_state.set(ClaimRefundState::Error(msg.to_string()));
                    return;
                }
            };
            log::info!("[claim-refund] wallet connected: {wn} ({pubkey})");

            let wallet_name_for_tx = wn.clone();
            let pk_for_tx = pubkey.clone();

            // Transition to signing
            set_refund_state.set(ClaimRefundState::Signing(
                wallet_name_for_tx.clone(),
                pk_for_tx.clone(),
            ));

            // Step 2: Build refund TX
            let body = RefundTxRequest {
                event_id: event_id_for_tx,
                attendee_id: api_id_for_tx,
                wallet_address: pk_for_tx.clone(),
            };
            let refund_resp = match api::build_refund_tx(&body).await {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("Failed to build refund: {e}");
                    log::error!("[claim-refund] {msg}");
                    components::show_toast(&set_toast, &msg, ToastType::Error);
                    set_refund_state.set(ClaimRefundState::Error(msg));
                    return;
                }
            };

            let tx_b64 = refund_resp.transaction;
            if tx_b64.is_empty() {
                let msg = "Refund transaction was empty. Please try again later.";
                log::error!("[claim-refund] {msg}");
                components::show_toast(&set_toast, msg, ToastType::Error);
                set_refund_state.set(ClaimRefundState::Error(msg.to_string()));
                return;
            }

            // Step 3: Sign and send
            match sign_and_send_tx_js(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[claim-refund] TX sent, sig: {signature}");
                    set_refund_state.set(ClaimRefundState::Confirmed(signature));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    let msg = crate::wallet_error::user_friendly_message(&e);
                    log::error!("[claim-refund] sign+send error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(&set_toast, &msg, ToastType::Error);
                    set_refund_state.set(ClaimRefundState::Error(msg));
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    let msg = "Refund transaction failed. Please try again.";
                    log::error!("[claim-refund] sign+send failed");
                    components::show_toast(&set_toast, msg, ToastType::Error);
                    set_refund_state.set(ClaimRefundState::Error(msg.to_string()));
                }
            }
        });
    };

    view! {
        <div class="card dep-card">
            <Toast toast_signal=toast />
            // Header
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::Coin class="icon-md" />" Deposit & Refund"</h2>
                {if deposit_amount_usdc > 0 {
                    view! {
                        <span class="badge badge-info">
                            {format!("{} USDC", usdc_display)}
                        </span>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>

            // Refund flow — only show when attendee has a verified USDC deposit
            {move || {
                let current = refund_state.get();
                if has_usdc_deposit && deposit_amount_usdc > 0 {
                    match &current {
                        ClaimRefundState::Idle | ClaimRefundState::Error(_) => {
                            let wallets = get_detected_wallets_js();
                            if let Some(err) = match &current {
                                ClaimRefundState::Error(e) => Some(e.clone()),
                                _ => None,
                            } {
                                let err_msg = err.clone();
                                view! {
                                    <div class="claim-refund-error">
                                        <p class="hint-desc" style="color:#ef4444">{escape_html(&err_msg)}</p>
                                        <button
                                            class="btn btn-outline btn-sm"
                                            on:click=move |_| set_refund_state.set(ClaimRefundState::Idle)
                                        >
                                            "Try Again"
                                        </button>
                                    </div>
                                }.into_any()
                            } else if wallets.is_empty() {
                                view! {
                                    <p class="hint-desc">
                                        "No Solana wallet detected. Install "
                                        <a href="https://phantom.app/" target="_blank" rel="noopener noreferrer">"Phantom"</a>
                                        ", Backpack, or Solflare and refresh."
                                    </p>
                                    // Fallback link to deposit page
                                    <a href=&deposit_link class="btn btn-outline btn-sm">"Go to Deposit Page →"</a>
                                }.into_any()
                            } else {
                                view! {
                                    <p class="hint-desc">
                                        "You deposited " <strong>{format!("{} USDC", usdc_display)}</strong>
                                        ". Claim your refund below:"
                                    </p>
                                    <div class="wallet-list">
                                        {wallets.into_iter().map(|w| {
                                            let w_label = w.clone();
                                            let w_click = w.clone();
                                            let wallet_icon = wallet_icon_name(&w);
                                            view! {
                                                <button
                                                    class="btn btn-primary btn-block wallet-btn-inner"
                                                    on:click=move |_| do_refund(w_click.clone())
                                                >
                                                    <span><Icon icon=wallet_icon class="icon-sm" /></span>
                                                    <span>{format!("Connect {} & Claim Refund", &w_label)}</span>
                                                </button>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_any()
                            }
                        }
                        ClaimRefundState::Connecting => {
                            view! {
                                <div class="spinner-wrap">
                                    <span class="spinner spinner-lg"></span>
                                </div>
                                <p class="hint-sm">"Connecting wallet..."</p>
                            }.into_any()
                        }
                        ClaimRefundState::Signing(_wallet_name, _public_key) => {
                            view! {
                                <div class="spinner-wrap">
                                    <span class="spinner spinner-lg"></span>
                                </div>
                                <p class="hint-sm">"Please approve the transaction in your wallet..."</p>
                            }.into_any()
                        }
                        ClaimRefundState::Confirmed(tx_sig) => {
                            let sig_display = if tx_sig.len() > 20 {
                                format!("{}...{}", &tx_sig[..8], &tx_sig[tx_sig.len()-8..])
                            } else {
                                tx_sig.clone()
                            };
                            let solscan_url = solscan_tx_url(tx_sig, &cluster);
                            let usdc_fmt = format!("{:.2}", deposit_amount_usdc as f64 / 1_000_000.0);
                            view! {
                                <div class="celebration-emoji"><Icon icon=IconName::Coin class="icon-3xl" /><Icon icon=IconName::Recycle class="icon-3xl" /></div>
                                <p class="success-title">
                                    {format!("{usdc_fmt} USDC refunded to your wallet!")}
                                </p>
                                <p class="hint-desc">
                                    "Your refund has been confirmed on Solana. Funds should appear in your wallet shortly."
                                </p>
                                <div class="tx-hash-box">
                                    {format!("TX: {}", &sig_display)}
                                </div>
                                <a href=&solscan_url target="_blank" class="tx-explorer-link">
                                    "View on Solscan ↗"
                                </a>
                            }.into_any()
                        }
                    }
                } else {
                    // No verified USDC deposit on-chain
                    if has_claimed {
                        // Post-claim: informational only — attendee already got their NFT
                        view! {
                            <p class="hint-sm">
                                "This event required a deposit to attend."
                            </p>
                            <div class="badge-row">
                                {if deposit_amount_usdc > 0 {
                                    view! {
                                        <div class="badge" style="background:var(--badge-bg,#1e1b4b);color:#c084fc;padding:0.5rem 1rem;border-radius:0.5rem">
                                            {format!("{} USDC", usdc_display)}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                                {if deposit_amount_thb > 0 {
                                    view! {
                                        <div class="badge" style="background:var(--badge-bg,#1e1b4b);color:#c084fc;padding:0.5rem 1rem;border-radius:0.5rem">
                                            {format!("{} THB", deposit_amount_thb)}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                            </div>
                            <p class="hint-desc" style="margin-top:0.5rem">
                                "If you submitted a deposit (USDC or THB), visit the deposit page to check your status or request a refund."
                            </p>
                            <a href=&deposit_link class="btn btn-outline btn-sm">
                                "Deposit & Refund Details →"
                            </a>
                        }.into_any()
                    } else {
                        // Pre-claim: attendee hasn't claimed yet, deposit still pending
                        view! {
                            <p class="hint-sm">
                                "This event requires a deposit to confirm your spot."
                            </p>
                            <div class="badge-row">
                                {if deposit_amount_usdc > 0 {
                                    view! {
                                        <div class="badge" style="background:var(--badge-bg,#1e1b4b);color:#c084fc;padding:0.5rem 1rem;border-radius:0.5rem">
                                            {format!("{} USDC", usdc_display)}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                                {if deposit_amount_thb > 0 {
                                    view! {
                                        <div class="badge" style="background:var(--badge-bg,#1e1b4b);color:#c084fc;padding:0.5rem 1rem;border-radius:0.5rem">
                                            {format!("{} THB", deposit_amount_thb)}
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}
                            </div>
                            <a href=&deposit_link class="btn btn-primary">
                                "Go to Deposit Page"
                            </a>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

/// The page looks up their check-in record and allows them to mint a
/// compressed NFT badge to their Solana wallet.
#[component]
pub fn Claim() -> impl IntoView {
    let params = use_params::<ClaimParams>();

    // Reactive state
    let (state, set_state) = signal(ClaimState::Loading);
    let (wallet_input, set_wallet_input) = signal(String::new());

    // Quiz state — selected answer text per question (question_id → option text)
    let (quiz_answers, set_quiz_answers): (ReadSignal<QuizAnswers>, WriteSignal<QuizAnswers>) = signal(QuizAnswers::new());

    // Dynamic event config (fetched from backend, replaces hardcoded values)
    let (evt_name, set_evt_name) = signal(String::new());
    let (evt_tagline, set_evt_tagline) = signal(String::new());
    let (evt_link, set_evt_link) = signal(String::new());
    let (evt_start, set_evt_start) = signal(0i64);
    let (evt_end, set_evt_end) = signal(0i64);

    // Share feedback
    let (share_copied, set_share_copied) = signal(false);

    // Claim counter (fetched from backend on initial lookup)
    let (total_checked_in, set_total_checked_in) = signal(0usize);
    let (total_claimed, set_total_claimed) = signal(0usize);

    // Deposit info (persisted across state transitions)
    let (deposit_api_id, set_deposit_api_id) = signal(String::new());
    let (deposit_event_id, set_deposit_event_id) = signal(String::new());
    let (deposit_enabled, set_deposit_enabled) = signal(false);
    let (deposit_amount_usdc, set_deposit_amount_usdc) = signal(0u64);
    let (deposit_amount_thb, set_deposit_amount_thb) = signal(0u64);
    // Whether the attendee has a verified USDC deposit (fetched separately).
    let (has_usdc_deposit, set_has_usdc_deposit) = signal(false);

    // Wallet adapter state — detected wallets and connected wallet info
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    let (connected_wallet, set_connected_wallet) = signal(None::<(String, String)>); // (wallet_name, public_key)

    // Detect installed wallets on mount (poll with delay for late injection)
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo::timers::future::TimeoutFuture::new(300).await;
                    wallets = get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[claim] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

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
                    set_total_checked_in.set(data.total_checked_in);
                    set_total_claimed.set(data.total_claimed);

                    // Store deposit info for use across state transitions
                    set_deposit_api_id.set(data.api_id.clone());
                    set_deposit_event_id.set(data.event_id.clone());
                    set_deposit_enabled.set(data.deposit_enabled);
                    set_deposit_amount_usdc.set(data.deposit_amount_usdc);
                    set_deposit_amount_thb.set(data.deposit_amount_thb);

                    // Check if attendee has a verified USDC deposit
                    // (needed for inline refund flow on Success/AlreadyClaimed)
                    if data.deposit_enabled && !data.api_id.is_empty() && data.deposit_amount_usdc > 0 {
                        let api_id_for_deposit = data.api_id.clone();
                        let event_id_for_deposit = data.event_id.clone();
                        let set_has_deposit = set_has_usdc_deposit;
                        leptos::task::spawn_local(async move {
                            match api::get_deposit_status(
                                &api_id_for_deposit,
                                if event_id_for_deposit.is_empty() { None } else { Some(&event_id_for_deposit) },
                            ).await {
                                Ok(deposit_resp) => {
                                    let is_usdc_deposited = deposit_resp.status
                                        .as_ref()
                                        .map(|s| s.verified && s.currency == "USDC")
                                        .unwrap_or(false);
                                    log::info!("[claim] deposit status: has_usdc_deposit={is_usdc_deposited}");
                                    set_has_deposit.set(is_usdc_deposited);
                                }
                                Err(e) => {
                                    log::warn!("[claim] deposit status fetch failed: {e}");
                                }
                            }
                        });
                    }

                    if data.claimed {
                        set_state.set(ClaimState::AlreadyClaimed(data));
                    } else if !data.nft_available {
                        set_state.set(ClaimState::NftComingSoon(data));
                    } else if matches!(
                        data.quiz_status,
                        QuizStatus::NotStarted | QuizStatus::InProgress
                    ) {
                        // Quiz required — fetch questions, then route to Quiz state
                        let claim_data = data.clone();
                        leptos::task::spawn_local(async move {
                            match api::get_quiz().await {
                                Ok(quiz_data) if quiz_data.configured => {
                                    set_state.set(ClaimState::Quiz(claim_data, quiz_data));
                                }
                                Ok(_) => {
                                    // Quiz not configured despite status — fallback to Ready
                                    log::warn!(
                                        "[claim] quiz status={:?} but quiz not configured, falling back to Ready",
                                        claim_data.quiz_status
                                    );
                                    if let Some(ref wallet) = claim_data.locked_wallet
                                        && !wallet.is_empty()
                                    {
                                        set_wallet_input.set(wallet.clone());
                                    }
                                    set_state.set(ClaimState::Ready(claim_data));
                                }
                                Err(e) => {
                                    log::error!("[claim] failed to fetch quiz: {e}");
                                    // Fallback to Ready so attendee isn't stuck
                                    if let Some(ref wallet) = claim_data.locked_wallet
                                        && !wallet.is_empty()
                                    {
                                        set_wallet_input.set(wallet.clone());
                                    }
                                    set_state.set(ClaimState::Ready(claim_data));
                                }
                            }
                        });
                    } else {
                        // Pre-fill wallet if locked to a pre-registered address
                        if let Some(ref wallet) = data.locked_wallet
                            && !wallet.is_empty()
                        {
                            set_wallet_input.set(wallet.clone());
                        }
                        // Check adventure status — if required and not passed, show adventure gate
                        let claim_data_for_adventure = data.clone();
                        let token_for_adventure = token.clone();
                        leptos::task::spawn_local(async move {
                            match api::get_adventure_status(&token_for_adventure).await {
                                Ok(status_data) => {
                                    match status_data.status {
                                        AdventureStatusType::NotRequired => {
                                            set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                        }
                                        AdventureStatusType::Passed => {
                                            set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                        }
                                        AdventureStatusType::NotStarted | AdventureStatusType::InProgress => {
                                            log::info!("[claim] adventure required but not passed, showing adventure gate");
                                            set_state.set(ClaimState::Adventure(
                                                claim_data_for_adventure,
                                                status_data.status,
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[claim] failed to check adventure status: {e}, proceeding to Ready");
                                    // Fallback to Ready so attendee isn't stuck
                                    set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                }
                            }
                        });
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
            let start = js_sys::Date::now();
            let result = api::post_claim(&token, &wallet).await;
            // Ensure spinner displays for at least 1.5s for smooth UX
            let elapsed = js_sys::Date::now() - start;
            if elapsed < 1500.0 {
                let wait = (1500.0 - elapsed) as u32;
                gloo::timers::future::TimeoutFuture::new(wait).await;
            }
            match result {
                Ok(mint_data) => {
                    log::info!(
                        "[claim] minted nft: asset_id={} sig={}",
                        mint_data.asset_id,
                        mint_data.signature
                    );
                    set_state.set(ClaimState::Success(mint_data));
                    // Increment claim counter for display
                    set_total_claimed.update(|n| *n += 1);
                    // Launch confetti celebration!
                    launch_confetti();
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
        let set_w = set_wallet_input;
        leptos::task::spawn_local(async move {
            let promise = read_clipboard_text_js();
            if let Ok(val) = js_sys::futures::JsFuture::from(promise).await
                && let Some(text) = val.as_string()
            {
                let trimmed: String = text.trim().to_string();
                if !trimmed.is_empty() {
                    set_w.set(trimmed);
                }
            }
        });
    };

    // Clone signal setters for use in nested reactive closures
    let set_w_for_connect = set_wallet_input;
    let set_cw_for_connect = set_connected_wallet;

    view! {
        <div class="center-page">
            <Title text="Claim Your NFT — BeThere" />
            <div class="container claim-container">
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

                // Progress stepper — shows claim flow progress
                {move || {
                    let (current, total) = claim_step(&state.get());
                    // Only show when flow has started (not loading/not found)
                    if current > 0 {
                        // Determine if quiz step is needed for this flow
                        let show_quiz = matches!(
                            state.get(),
                            ClaimState::Quiz(_, _)
                                | ClaimState::QuizSubmitted(_, _, _)
                        );
                        view! {
                            <ClaimStepper current=current total=total show_quiz=show_quiz />
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}

                // State-dependent rendering
                {move || {
                    match state.get() {
                        // ---- Loading ----
                        ClaimState::Loading => {
                            view! {
                                <div class="claim-state-full">
                                    // Shimmer: welcome card (avatar + 2 text lines)
                                    <div class="shimmer-card claim-shimmer-row">
                                        <div class="shimmer shimmer-avatar"></div>
                                        <div class="claim-shimmer-col">
                                            <div class="shimmer shimmer-line" style="width:60%;"></div>
                                            <div class="shimmer shimmer-line-sm" style="width:40%;"></div>
                                        </div>
                                    </div>

                                    // Shimmer: NFT preview card (square + 2 text lines)
                                    <div class="shimmer-card claim-shimmer-row">
                                        <div class="shimmer claim-shimmer-nft"></div>
                                        <div class="claim-shimmer-col">
                                            <div class="shimmer shimmer-line" style="width:75%;"></div>
                                            <div class="shimmer shimmer-line-sm" style="width:50%;"></div>
                                        </div>
                                    </div>

                                    // Shimmer: wallet input card (label + input bar + hint)
                                    <div class="shimmer-card u-mb-sm">
                                        <div class="shimmer shimmer-line-sm u-mb-sm" style="width:40%;"></div>
                                        <div class="shimmer claim-shimmer-input"></div>
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
                                    <a href="/" class="btn btn-outline claim-retry-btn">
                                        "Go to Home"
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- NFT Coming Soon ----
                        ClaimState::NftComingSoon(data) => {
                            let checked_in_display = checked_in_label(&data.checked_in_at, &data.participation_type);
                            view! {
                                <div class="claim-state-full">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">{checked_in_display}</p>
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

                        // ---- Quiz required ----
                        ClaimState::Quiz(claim_data, quiz_data) => {
                            view! {
                                <QuizView
                                    claim_data=claim_data
                                    quiz_data=quiz_data
                                    quiz_answers=quiz_answers
                                    set_quiz_answers=set_quiz_answers
                                    set_state=set_state
                                />
                            }
                                .into_any()
                        }

                        // ---- Quiz submitted — results ----
                        ClaimState::QuizSubmitted(claim_data, quiz_data, submit_result) => {
                            view! {
                                <QuizSubmittedView
                                    claim_data=claim_data
                                    quiz_data=quiz_data
                                    submit_result=submit_result
                                    set_quiz_answers=set_quiz_answers
                                    wallet_input=wallet_input
                                    set_wallet_input=set_wallet_input
                                    set_state=set_state
                                />
                            }
                                .into_any()
                        }

                        // ---- Ready: show wallet input ----
                        ClaimState::Ready(data) => {
                            let checked_in_display = checked_in_label(&data.checked_in_at, &data.participation_type);
                            let locked_wallet = data.locked_wallet.clone();
                            view! {
                                <div class="claim-state-full">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">{checked_in_display}</p>
                                    </div>

                                    // NFT badge preview
                                    <NftBadgePreview />

                                    // Wallet input — wallet adapter + manual fallback
                                    <div class="card">
                                        <label class="claim-wallet-label">
                                            "Solana Wallet Address"
                                        </label>

                                        // Locked wallet pill + wallet adapter section (single reactive closure)
                                        {move || {
                                            // Locked wallet pill badge — shown when pre-registered wallet exists
                                            let is_locked = matches!(&locked_wallet, Some(w) if !w.is_empty());
                                            if is_locked {
                                                let w = locked_wallet.as_ref().unwrap();
                                                let truncated = if w.len() > 12 {
                                                    format!("{}...{}", &w[..4], &w[w.len()-4..])
                                                } else {
                                                    w.clone()
                                                };
                                                view! {
                                                    <div class="claim-wallet-locked">
                                                        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                                                            <rect x="3" y="7" width="10" height="7" rx="1.5"></rect>
                                                            <path d="M5 7V5a3 3 0 0 1 6 0v2"></path>
                                                        </svg>
                                                        <span class="locked-wallet-addr">{truncated}</span>
                                                    </div>
                                                }.into_any()
                                            } else {
                                                let cw = connected_wallet.get();
                                                match cw {
                                                    // Connected state: wallet icon + name + truncated address + connected badge
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
                                                                <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Connected"</span>
                                                            </div>
                                                            <button
                                                                class="btn btn-outline btn-sm"
                                                                style="width:100%;margin-bottom:0.75rem;"
                                                                on:click=move |_| { set_cw_for_connect.set(None); }
                                                                type="button"
                                                            >
                                                                "Disconnect"
                                                            </button>
                                                        }.into_any()
                                                    }
                                                    // Not connected: show connect buttons + manual fallback
                                                    None => {
                                                        let mut wallets = detected_wallets.get();
                                                        // Always show Phantom as an option — if not installed,
                                                        // the connect will fail gracefully and show install prompt.
                                                        if !wallets.iter().any(|w| w.eq_ignore_ascii_case("Phantom")) {
                                                            wallets.push("Phantom".to_string());
                                                        }
                                                        let has_wallets = !wallets.is_empty();
                                                        view! {
                                                            // Wallet adapter connect buttons
                                                            {if has_wallets {
                                                                let wallets_for_click = wallets.clone();
                                                                view! {
                                                                    <div class="wallet-list">
                                                                        <p class="wallet-prompt">
                                                                            <Icon icon=IconName::Link class="icon-sm"/>
                                                                            " Connect your Solana wallet:"
                                                                        </p>
                                                                        {wallets_for_click.into_iter().map(|w| {
                                                                            let w_clone = w.clone();
                                                                            let wallet_icon = wallet_icon_name(&w);
                                                                            view! {
                                                                                <button
                                                                                    class="btn btn-primary btn-block wallet-btn-inner"
                                                                                    on:click={
                                                                                        let w = w.clone();
                                                                                        let set_w = set_w_for_connect;
                                                                                        let set_cw = set_cw_for_connect;
                                                                                        move |_| {
                                                                                            let w = w.clone();
                                                                                            let set_w = set_w;
                                                                                            let set_cw = set_cw;
                                                                                            leptos::task::spawn_local(async move {
                                                                                                match connect_wallet_js(&w).await {
                                                                                                    crate::wallet_error::WalletResult::Success(pubkey) => {
                                                                                                        log::info!("[claim] wallet connected: {} ({})", w, pubkey);
                                                                                                        set_w.set(pubkey.clone());
                                                                                                        set_cw.set(Some((w, pubkey)));
                                                                                                    }
                                                                                                    crate::wallet_error::WalletResult::Error(e) => {
                                                                                                        if e.raw_message.contains("Wallet not found") {
                                                                                                            log::info!("[claim] {} not installed, opening download page", w);
                                                                                                            let url = match w.to_lowercase().as_str() {
                                                                                                                "phantom" => "https://phantom.app/download",
                                                                                                                "backpack" => "https://backpack.app/download",
                                                                                                                "solflare" => "https://solflare.com/download",
                                                                                                                _ => "https://phantom.app/download",
                                                                                                            };
                                                                                                            let _ = web_sys::window().and_then(|w| w.open_with_url_and_target(url, "_blank").ok());
                                                                                                        } else {
                                                                                                            log::warn!("[claim] wallet connect error for {}: code={:?} msg={}", w, e.code, e.raw_message);
                                                                                                        }
                                                                                                    }
                                                                                                    crate::wallet_error::WalletResult::UnknownFailure => {
                                                                                                        log::warn!("[claim] wallet connect failed for {}", w);
                                                                                                    }
                                                                                                }
                                                                                            });
                                                                                        }
                                                                                    }
                                                                                >
                                                                                    <span><Icon icon=wallet_icon class="icon-sm" /></span>
                                                                                    <span>{format!("Connect {}", &w_clone)}</span>
                                                                                </button>
                                                                            }
                                                                        }).collect::<Vec<_>>()}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}

                                                            // Divider — "or enter manually"
                                                            {if has_wallets {
                                                                view! {
                                                                    <div class="claim-wallet-divider">
                                                                        <span>"or enter manually"</span>
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}

                                                            // Manual text input (always visible as fallback)
                                                            <div class="claim-wallet-row">
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
                                                                <button
                                                                    class="claim-paste-btn"
                                                                    on:click=handle_paste
                                                                    type="button"
                                                                >
                                                                    "Paste"
                                                                </button>
                                                            </div>
                                                            <p class="claim-wallet-hint">
                                                                {
                                                                    match &locked_wallet {
                                                                        Some(w) if !w.is_empty() => "Use the pre-filled wallet address to claim.",
                                                                        _ => "Tap Paste or type your Phantom, Solflare, or Backpack address.",
                                                                    }
                                                                }
                                                            </p>
                                                        }.into_any()
                                                    }
                                                }
                                            }
                                        }}
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

                        // ---- Adventure gate — must complete before claiming ----
                        ClaimState::Adventure(data, adv_status) => {
                            let token_val = match params.get() {
                                Ok(p) => p.token.unwrap_or_default(),
                                Err(_) => String::new(),
                            };
                            let adventure_url = format!("/adventure?token={token_val}");
                            let status_msg = match adv_status {
                                AdventureStatusType::NotStarted => "You haven't started the Rust Adventure yet. Complete it to unlock your NFT!",
                                AdventureStatusType::InProgress => "You're making progress! Keep going to complete the adventure.",
                                _ => "Complete the Rust Adventure to unlock your NFT!",
                            };
                            view! {
                                <div class="claim-adventure-gate">
                                    <ParticipantAvatar name=data.name.clone() />
                                    <h2><Icon icon=IconName::Crab class="icon-md" />" Rust Adventure Required"</h2>
                                    <p class="claim-adventure-status">{status_msg}</p>
                                    <div class="claim-adventure-info">
                                        <p>
                                            <strong>{escape_html(&data.name)}</strong>", complete the Rust Adventures game to earn your NFT badge."
                                        </p>
                                        <p class="claim-adventure-hint">
                                            "Learn Rust basics by solving coding puzzles in a fun tile-based game!"
                                        </p>
                                    </div>
                                    <a
                                        class="btn btn-primary claim-adventure-btn"
                                        href={adventure_url}
                                    >
                                        "🎮 Start Adventure"
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Minting in progress ----
                        ClaimState::Minting(data) => {
                            view! {
                                <div class="claim-minting">
                                    // Pulsing minting indicator
                                    <div class="claim-minting-spinner">
                                        <div class="shimmer claim-minting-shimmer"></div>
                                        <span class="spinner spinner-lg"></span>
                                    </div>
                                    <h3 class="claim-minting-title">"Minting your NFT..."</h3>
                                    <p class="claim-minting-detail">
                                        "Minting for "{escape_html(&data.name)}
                                    </p>
                                    <p class="claim-minting-hint">
                                        "This usually takes 3-5 seconds."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Success! ----
                        ClaimState::Success(data) => {
                            let explorer_url = solscan_tx_url(&data.signature, &data.cluster);
                            let metaplex_url = metaplex_explorer_url(&data.asset_id, &data.cluster);
                            let solanafm_url = solanafm_asset_url(&data.asset_id, &data.cluster);
                            let asset_id_display = {
                                let id = &data.asset_id;
                                if id.len() > 12 {
                                    format!("{}...{}", &id[..6], &id[id.len()-4..])
                                } else {
                                    id.clone()
                                }
                            };
                            let asset_id_full = data.asset_id.clone();

                            // Build share text & URL
                            let tweet_text = {
                                let event = evt_name.get();
                                if event.is_empty() {
                                    "I just claimed my attendance NFT on BeThere! 🎫✨\n\nProof I showed up. On-chain.\n\n#BeThere #Solana".to_string()
                                } else {
                                    format!("I just earned my POAP at {event}! 🎫✨\n\nShowed up, proved I was there, got my NFT badge.\n\n#BeThere #Solana")
                                }
                            };
                            let share_to_x_url = format!(
                                "https://twitter.com/intent/tweet?text={}",
                                js_sys::encode_uri_component(&tweet_text)
                            );
                            let claim_page_url = format!(
                                "https://bethere.solana-thailand.workers.dev/claim/{}",
                                match params.get() {
                                    Ok(p) => p.token.unwrap_or_default(),
                                    Err(_) => String::new(),
                                }
                            );
                            let tweet_preview_text = tweet_text.clone();

                            view! {
                                <div class="claim-success">
                                    <div class="claim-success-rings">
                                        <div class="claim-success-ring claim-success-ring-3"></div>
                                        <div class="claim-success-ring claim-success-ring-2"></div>
                                        <div class="claim-success-ring claim-success-ring-1"></div>
                                        <div class="success-check">
                                            <svg viewBox="0 0 24 24">
                                                <polyline points="20 6 9 17 4 12"></polyline>
                                            </svg>
                                        </div>
                                    </div>
                                    <h2>"NFT Claimed!"</h2>

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
                                        <p><strong>"Name:"</strong>" " {escape_html(&data.name)}</p>
                                        <p><strong>"Wallet:"</strong>
                                            <code>{escape_html(&data.wallet_address)}</code>
                                        </p>
                                        <p><strong>"Claimed:"</strong>" " {format_timestamp(&data.claimed_at)}</p>
                                    </div>

                                    // Explorer links
                                    <div class="success-actions">
                                        <a
                                            href=solanafm_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-primary btn-block"
                                        >
                                            "View NFT on SolanaFM ↗"
                                        </a>
                                        <a
                                            href=explorer_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-block"
                                        >
                                            "View TX on Solscan ↗"
                                        </a>
                                        <a
                                            href=metaplex_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-block"
                                        >
                                            "Verify on Metaplex ↗"
                                        </a>
                                    </div>

                                    // cNFT explanation — help attendees understand how to view their NFT
                                    <div class="claim-cnft-hint">
                                        <p class="hint-title">"🎫 Your Compressed NFT"</p>
                                        <p class="hint-desc">
                                            "This NFT is stored on Solana using state compression. View it on "
                                            <strong>"SolanaFM"</strong>
                                            " above — it shows the full NFT with artwork and metadata. "
                                            "It may not appear in some wallet apps (Phantom, Solflare)."
                                        </p>
                                    </div>

                                    // Claim counter — social proof
                                    {move || {
                                        let checked_in = total_checked_in.get();
                                        let claimed = total_claimed.get();
                                        if checked_in > 0 {
                                            view! {
                                                <div class="claim-counter">
                                                    <span class="claim-counter-badge"><Icon icon=IconName::Trophy class="icon-md icon-warning" /></span>
                                                    <span class="claim-counter-text">
                                                        <strong>{claimed}</strong>
                                                        " of "
                                                        <strong>{checked_in}</strong>
                                                        " attendees claimed their NFT"
                                                    </span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }
                                    }}

                                    // Share section with tweet preview
                                    <div class="claim-share-section">
                                        <div class="claim-share-heading">"Share your achievement"</div>
                                        <div class="claim-share-preview">
                                            <div class="claim-share-preview-name">"BeThere ✦ @BeThere"</div>
                                            {tweet_preview_text}
                                        </div>
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
                                                "Post to X"
                                            </a>
                                            <button
                                                class="claim-share-copy-btn"
                                                type="button"
                                                title="Copy claim link"
                                                on:click=move |_| {
                                                    let _ = copy_to_clipboard_js(&claim_page_url);
                                                    set_share_copied.set(true);
                                                    leptos::task::spawn_local(async move {
                                                        gloo::timers::future::TimeoutFuture::new(2000).await;
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
                                            "Link copied!"
                                        </div>
                                    </div>
                                </div>
                                {move || {
                                    if deposit_enabled.get() && !deposit_api_id.get().is_empty() {
                                        view! {
                                            <DepositRefundSection
                                                api_id=deposit_api_id.get()
                                                event_id=deposit_event_id.get()
                                                deposit_amount_usdc=deposit_amount_usdc.get()
                                                deposit_amount_thb=deposit_amount_thb.get()
                                                has_usdc_deposit=has_usdc_deposit.get()
                                                has_claimed=true
                                            />
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }
                                }}
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

                            // Explorer links (if claim lock data is available)
                            let has_explorer_links = data.claimed_signature.is_some()
                                || data.claimed_asset_id.is_some();
                            let claimed_cluster = data.cluster.as_deref().unwrap_or("devnet");
                            let explorer_url = data.claimed_signature.as_ref().map(|sig| {
                                solscan_tx_url(sig, claimed_cluster)
                            });
                            let metaplex_url = data.claimed_asset_id.as_ref().map(|asset_id| {
                                metaplex_explorer_url(asset_id, claimed_cluster)
                            });

                            view! {
                                <div class="claim-warning">
                                    <ParticipantAvatar name=data.name.clone() />
                                    <h2>"Already Claimed"</h2>
                                    <div class="result-details">
                                        <p>
                                            <strong>{escape_html(&data.name)}</strong>
                                            " — your NFT was claimed "{claimed_display}"."
                                        </p>
                                        {
                                            if let Some(ref wallet) = data.claimed_wallet {
                                                view! {
                                                    <p><strong>"Wallet:"</strong>
                                                        <code>{escape_html(wallet)}</code>
                                                    </p>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }
                                        }
                                        <p class="claim-already-detail">
                                            "Your compressed NFT may not appear in wallet apps like Phantom or Solflare. Use the links below to verify it on-chain."
                                        </p>
                                    </div>
                                </div>

                                // Explorer links — only if claim lock data is available
                                {move || {
                                    if has_explorer_links {
                                        view! {
                                            <div class="success-actions">
                                                {
                                                    if let Some(ref url) = explorer_url {
                                                        view! {
                                                            <a
                                                                href=url
                                                                target="_blank"
                                                                rel="noopener noreferrer"
                                                                class="btn btn-primary btn-block"
                                                            >
                                                                "View TX on Solscan ↗"
                                                            </a>
                                                        }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }
                                                }
                                                {
                                                    if let Some(ref url) = metaplex_url {
                                                        view! {
                                                            <a
                                                                href=url
                                                                target="_blank"
                                                                rel="noopener noreferrer"
                                                                class="btn btn-outline btn-block"
                                                            >
                                                                "Verify NFT on Metaplex ↗"
                                                            </a>
                                                        }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }
                                                }
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }
                                }}

                                // cNFT explanation hint
                                {move || {
                                    if has_explorer_links {
                                        view! {
                                            <div class="claim-cnft-hint">
                                                <p class="hint-title">"💡 Compressed NFT Info"</p>
                                                <p class="hint-desc">
                                                    "This is a compressed NFT stored on Solana. It may "
                                                    <strong>"not appear"</strong>
                                                    " in some wallet apps (Phantom, Solflare). Use the links above to verify your NFT on-chain."
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }
                                }}

                                {move || {
                                    if deposit_enabled.get() && !deposit_api_id.get().is_empty() {
                                        view! {
                                            <DepositRefundSection
                                                api_id=deposit_api_id.get()
                                                event_id=deposit_event_id.get()
                                                deposit_amount_usdc=deposit_amount_usdc.get()
                                                deposit_amount_thb=deposit_amount_thb.get()
                                                has_usdc_deposit=has_usdc_deposit.get()
                                                has_claimed=true
                                            />
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }
                                }}
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
                                        class="btn btn-primary claim-retry-btn"
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
