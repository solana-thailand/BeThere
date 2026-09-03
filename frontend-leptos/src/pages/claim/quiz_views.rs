//! Extracted quiz components (reduce nesting in main view! macro).

use leptos::prelude::*;

use crate::api::{
    self, ClaimLookupData, QuizQuestionsData,
    QuizSubmitData,
};
use crate::utils::escape_html;

use super::helpers::*;
use super::interop::*;
use super::quiz_helpers::*;
use super::state::*;
use super::widgets::*;

// ---------------------------------------------------------------------------
// Extracted quiz components (reduce nesting in main view! macro)
// ---------------------------------------------------------------------------

/// Quiz view — handles the ClaimState::Quiz state.
/// Extracted from the main Claim component to avoid the unclosed delimiter
/// caused by deeply nested view! macro content.
#[component]
pub(super) fn QuizView(
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
                    {
                        let mut seen = std::collections::HashSet::new();
                        for q in &quiz_data.questions {
                            if let Some(ref sid) = q.session_id {
                                seen.insert(sid.clone());
                            }
                        }
                        let session_count = seen.len();
                        if session_count > 1 {
                            view! {
                                <>
                                    <span class="claim-quiz-sep">"·"</span>
                                    <span>{format!("{session_count} sessions")}</span>
                                </>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }
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
pub(super) fn QuizSubmittedView(
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
