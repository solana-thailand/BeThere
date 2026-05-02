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

use crate::api::{self, AdventureStatusType, ClaimLookupData, ClaimMintData, QuizQuestionsData, QuizStatus, QuizSubmitData};
use crate::utils::{escape_html, format_timestamp};
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
                    <p class="claim-quiz-passed-msg">"✅ Quiz passed! Verifying adventure progress..."</p>
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
    let checked_in_display = format_timestamp(&claim_data.checked_in_at);
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
                <p class="checked-in-label">"Checked in "{checked_in_display}</p>
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
    let checked_in_display = format_timestamp(&claim_data.checked_in_at);
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
            if let Ok(promise_val) = js_sys::eval(
                "navigator.clipboard ? navigator.clipboard.readText() : Promise.resolve('')"
            ) {
                let promise = js_sys::Promise::from(promise_val);
                if let Ok(val) = js_sys::futures::JsFuture::from(promise).await
                    && let Some(text) = val.as_string()
                {
                    let trimmed: String = text.trim().to_string();
                    if !trimmed.is_empty() {
                        set_w.set(trimmed);
                    }
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
                <p class="checked-in-label">"Checked in "{checked_in_display}</p>
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
            if let Ok(promise_val) = js_sys::eval(
                "navigator.clipboard ? navigator.clipboard.readText() : Promise.resolve('')"
            ) {
                let promise = js_sys::Promise::from(promise_val);
                if let Ok(val) = js_sys::futures::JsFuture::from(promise).await
                    && let Some(text) = val.as_string()
                {
                    let trimmed: String = text.trim().to_string();
                    if !trimmed.is_empty() {
                        set_w.set(trimmed);
                    }
                }
            }
        });
    };

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
                                    <div class="shimmer-card" style="margin-bottom:1rem;">
                                        <div class="shimmer shimmer-line-sm" style="width:40%;margin-bottom:0.75rem;"></div>
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
                            let checked_in_display = format_timestamp(&data.checked_in_at);
                            view! {
                                <div class="claim-state-full">
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
                            let checked_in_display = format_timestamp(&data.checked_in_at);
                            let locked_wallet = data.locked_wallet.clone();
                            let locked_wallet_hint = data.locked_wallet.clone();
                            view! {
                                <div class="claim-state-full">
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
                                        <label class="claim-wallet-label">
                                            "Solana Wallet Address"
                                        </label>
                                        // Locked wallet pill badge — shown when pre-registered wallet exists
                                        {move || {
                                            match &locked_wallet {
                                                Some(w) if !w.is_empty() => {
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
                                                }
                                                _ => view! { <div></div> }.into_any(),
                                            }
                                        }}
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
                                    <h2>"🦀 Rust Adventure Required"</h2>
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
                            // Build Share to X URL with pre-filled tweet
                            let tweet_text = {
                                let event = evt_name.get();
                                if event.is_empty() {
                                    "I just claimed my POAP NFT! 🎫✨\n\n#BeThere #Solana".to_string()
                                } else {
                                    format!("I just claimed my POAP at {event}! 🎫✨\n\n#BeThere #Solana")
                                }
                            };
                            let share_to_x_url = format!(
                                "https://twitter.com/intent/tweet?text={}",
                                js_sys::encode_uri_component(&tweet_text)
                            );
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
                                            href=share_to_x_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-block claim-share-x"
                                        >
                                            <svg viewBox="0 0 24 24" fill="currentColor">
                                                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
                                            </svg>
                                            "Share to X"
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
                                        <p class="claim-already-detail">
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
