//! Progress stepper component.

use leptos::prelude::*;

use super::state::*;
use crate::i18n::{t_string, use_i18n};

/// One stepper label. Display only; the step itself is `ClaimState`.
#[derive(Clone, Copy)]
enum StepLabel {
    Verified,
    Quiz,
    Claim,
}

// ---------------------------------------------------------------------------
// Progress Stepper
// ---------------------------------------------------------------------------

/// Determines the current step number for the claim flow progress indicator.
/// Returns (current_step, total_steps) where steps are 1-indexed.
pub(super) fn claim_step(state: &ClaimState) -> (usize, usize) {
    match state {
        // Loading = step 0 (before flow starts)
        ClaimState::Loading => (0, 3),
        // Step 1: Verified (attendee found + checked in)
        ClaimState::NotFound(_) => (0, 3),
        // Not checked in yet: "Verified ✓" would contradict the card below.
        ClaimState::NftComingSoon(d)
            if super::helpers::awaiting_check_in(&d.checked_in_at, &d.participation_type) =>
        {
            (0, 3)
        }
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
pub(super) fn ClaimStepper(current: usize, total: usize, show_quiz: bool) -> impl IntoView {
    // Build step labels based on whether quiz is shown
    let _ = total; // used for context, steps are hardcoded
    let i18n = use_i18n();
    let steps: Vec<(&'static str, StepLabel, usize)> = if show_quiz {
        vec![
            ("✓", StepLabel::Verified, 1),
            ("?", StepLabel::Quiz, 2),
            ("", StepLabel::Claim, 3),
        ]
    } else {
        vec![("✓", StepLabel::Verified, 1), ("", StepLabel::Claim, 2)]
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
                                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" class="claim-step-check-icon">
                                            <polyline points="20 6 9 17 4 12"></polyline>
                                        </svg>
                                    }.into_any()
                                } else {
                                    view! { <span>{icon}</span> }.into_any()
                                }}
                            </div>
                            <span class=if is_current || is_completed { "claim-step-label active" } else { "claim-step-label" }>
                                {move || match label {
                                    StepLabel::Verified => t_string!(i18n, claim.step.verified),
                                    StepLabel::Quiz => t_string!(i18n, claim.step.quiz),
                                    StepLabel::Claim => t_string!(i18n, claim.step.claim),
                                }}
                            </span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
