//! Online-attendee quest gate (quiz or adventure).


use event_checkin_domain::models::adventure::AdventureStatus;
use event_checkin_domain::models::api::QuizStatus;

use crate::state::AppState;


// ---------------------------------------------------------------------------

/// Check if an online attendee has completed the required quest (quiz or adventure).
/// Returns true if at least one is passed, or if neither is required.
///
/// When `quiz_enabled = true` but no quiz config exists in D1/KV yet, returns `false`
/// to prevent claiming before the organizer finishes setting up the quiz.
pub(super) async fn verify_online_quest_completion(
    state: &AppState,
    event_id: &str,
    claim_token: &str,
    quiz_enabled: bool,
) -> bool {
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    // Check quiz status first
    match crate::quiz::get_quiz_status(d1, kv, event_id, claim_token).await {
        Ok(QuizStatus::Passed) => true,
        Ok(QuizStatus::NotRequired) => {
            // Quiz not configured — check adventure (D1 only)
            let Some(db) = d1 else {
                // No D1 available — treat adventure as not required
                return !quiz_enabled;
            };
            match crate::adventure::get_adventure_status(db, event_id, claim_token).await {
                Ok(AdventureStatus::Passed) => true,
                Ok(AdventureStatus::NotRequired) => {
                    // Neither quiz nor adventure configured.
                    // If quiz_enabled is true, the organizer intends a quest but
                    // hasn't set it up yet — block claiming.
                    !quiz_enabled
                }
                _ => false,
            }
        }
        _ => false,
    }
}
