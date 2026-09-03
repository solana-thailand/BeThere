//! Quiz rendering helpers (extracted to avoid nested view! macro issues).

use leptos::prelude::*;

use crate::api::{
    self, AdventureStatusType, ClaimLookupData,
};
use crate::icons::{Icon, IconName};

use super::interop::*;
use super::state::*;
use super::widgets::*;

// ---------------------------------------------------------------------------
// Quiz rendering helpers (extracted to avoid nested view! macro issues)
// ---------------------------------------------------------------------------

/// Build quiz question cards as pre-rendered views.
pub(super) fn build_quiz_questions(
    questions: &[crate::api::QuizQuestionPublic],
    total_q: usize,
    quiz_answers: ReadSignal<QuizAnswers>,
    set_quiz_answers: WriteSignal<QuizAnswers>,
) -> Vec<AnyView> {
    let mut views = Vec::new();
    let mut last_session_id: Option<String> = None;
    let mut first_session = true;

    for (idx, q) in questions.iter().enumerate() {
        // Insert session header when session changes
        if first_session || q.session_id != last_session_id {
            if let Some(ref title) = q.session_title {
                let title_clone = title.clone();
                views.push(view! {
                    <div class="claim-quiz-session">
                        <h4 class="claim-quiz-session-title">{title_clone}</h4>
                    </div>
                }.into_any());
            }
            last_session_id = q.session_id.clone();
            first_session = false;
        }

        let q_id = q.id.clone();
        let q_text = q.text.clone();
        let q_num = idx + 1;
        let options = q.options.clone();

        let option_views: Vec<AnyView> = options.iter().map(|opt| {
            let opt_display = opt.clone();
            let qid_c = q_id.clone();
            let opt_c = opt.clone();
            let qa_c = quiz_answers;
            let qid_r = q_id.clone();
            let opt_r = opt.clone();
            let qa_r = quiz_answers;
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

        views.push(view! {
            <div class="card claim-quiz-question">
                <div class="claim-quiz-q-header">
                    <span class="claim-quiz-q-num">{format!("{q_num}")}</span>
                    <span class="claim-quiz-q-of">"of "{total_q}</span>
                </div>
                <p class="claim-quiz-q-text">{q_text}</p>
                <div class="claim-quiz-options">{option_views}</div>
            </div>
        }.into_any());
    }

    views
}

/// Build quiz explanation cards as a pre-rendered view.
pub(super) fn build_quiz_explanations(
    explanations: &[crate::api::QuestionExplanation],
    questions: &[crate::api::QuizQuestionPublic],
) -> AnyView {
    if explanations.is_empty() {
        return view! { <div></div> }.into_any();
    }

    // Build a lookup from question_id to the question (for session info)
    let q_lookup: std::collections::HashMap<String, &crate::api::QuizQuestionPublic> =
        questions.iter().map(|q| (q.id.clone(), q)).collect();

    let mut items: Vec<AnyView> = Vec::new();
    let mut last_session_id: Option<String> = None;
    let mut first_session = true;

    for (idx, exp) in explanations.iter().enumerate() {
        // Resolve session info from the matching question
        let session_info = q_lookup.get(&exp.question_id).and_then(|q| {
            q.session_title.as_ref().map(|t| (q.session_id.clone(), t.clone()))
        });

        // Insert session header when session changes
        if let Some((ref sid, ref title)) = session_info
            && (first_session || *sid != last_session_id) {
                    let title_clone = title.clone();
                    items.push(view! {
                        <div class="claim-quiz-session">
                            <h4 class="claim-quiz-session-title">{title_clone}</h4>
                        </div>
                    }.into_any());
                last_session_id = sid.clone();
                first_session = false;
            }

        let q_text = q_lookup.get(&exp.question_id)
            .map(|q| q.text.clone())
            .unwrap_or_default();
        let icon = match exp.correct { true => "✓", _ => "✗" };
        let exp_class = match exp.correct {
            true => "claim-quiz-exp-correct",
            _ => "claim-quiz-exp-wrong",
        };
        let exp_text = exp.explanation.clone();
        let num = idx + 1;

        items.push(view! {
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
        }.into_any());
    }

    view! {
        <div class="card claim-quiz-explanations">
            <h4>"Answer Review"</h4>
            {items}
        </div>
    }.into_any()
}

/// Allowed quiz actions after submission.
pub(super) enum QuizAction {
    Passed,
    Retry,
    Exhausted,
}

/// Build the action section for quiz results (wallet+claim, retry, or exhausted).
/// Extracted to avoid nested view! macros inside conditional blocks.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_quiz_action(
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
                let set_wi_c = set_wi;
                let lw_c = lw.clone();
                let ss_c = ss;
                leptos::task::spawn_local(async move {
                    match api::get_adventure_status(&token_adv, Some(&claim_data_adv.event_id)).await {
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
