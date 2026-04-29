//! KV-based quiz storage for the activity-gated claim flow (Issue 002).
//!
//! Quiz questions and per-attendee progress are stored in a Cloudflare KV
//! namespace bound as `QUIZ`. Key schema:
//!
//!   "questions"                → QuizConfig (JSON) — set by organizer
//!   "progress:{claim_token}"   → QuizProgress (JSON) — per-attendee state

use chrono::Utc;
use worker::KvStore;

use event_checkin_domain::models::api::{
    QuestionExplanation, QuizAnswer, QuizAttempt, QuizConfig, QuizProgress, QuizQuestionPublic,
    QuizQuestionsResponse, QuizStatus, QuizSubmitResponse,
};

// ---------------------------------------------------------------------------
// Quiz config (questions)
// ---------------------------------------------------------------------------

/// Read quiz configuration from KV.
/// Returns `None` if no quiz is configured (key doesn't exist).
pub async fn get_quiz_config(kv: &KvStore) -> Result<Option<QuizConfig>, String> {
    kv.get("questions")
        .json::<QuizConfig>()
        .await
        .map_err(|e| format!("failed to read quiz config from KV: {e:?}"))
}

/// Write quiz configuration to KV (admin endpoint).
pub async fn save_quiz_config(kv: &KvStore, config: &QuizConfig) -> Result<(), String> {
    kv.put("questions", config)
        .map_err(|e| format!("failed to build quiz config put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write quiz config to KV: {e:?}"))
}

/// Convert full quiz config to public response (strips correct answers).
pub fn to_public_questions(config: &QuizConfig) -> QuizQuestionsResponse {
    QuizQuestionsResponse {
        questions: config
            .questions
            .iter()
            .map(|q| QuizQuestionPublic {
                id: q.id.clone(),
                text: q.text.clone(),
                options: q.options.clone(),
            })
            .collect(),
        passing_score_percent: config.passing_score_percent,
        max_attempts: config.max_attempts,
        time_limit_seconds: config.time_limit_seconds,
    }
}

// ---------------------------------------------------------------------------
// Quiz progress (per-attendee)
// ---------------------------------------------------------------------------

/// Build the KV key for an attendee's quiz progress.
fn progress_key(claim_token: &str) -> String {
    format!("progress:{claim_token}")
}

/// Read quiz progress for an attendee.
/// Returns `None` if no progress exists yet (hasn't attempted).
pub async fn get_quiz_progress(
    kv: &KvStore,
    claim_token: &str,
) -> Result<Option<QuizProgress>, String> {
    let key = progress_key(claim_token);
    kv.get(&key)
        .json::<QuizProgress>()
        .await
        .map_err(|e| format!("failed to read quiz progress from KV: {e:?}"))
}

/// Write quiz progress for an attendee.
async fn save_quiz_progress(kv: &KvStore, progress: &QuizProgress) -> Result<(), String> {
    let key = progress_key(&progress.claim_token);
    kv.put(&key, progress)
        .map_err(|e| format!("failed to build quiz progress put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write quiz progress to KV: {e:?}"))
}

/// Create a fresh quiz progress record for a first-time attempt.
fn new_progress(claim_token: &str) -> QuizProgress {
    QuizProgress {
        claim_token: claim_token.to_string(),
        attempts: 0,
        best_score_percent: 0,
        passed: false,
        passed_at: None,
        attempt_history: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Quiz submission logic
// ---------------------------------------------------------------------------

/// Score a quiz submission and persist updated progress.
///
/// Validates:
/// - Attendee hasn't exhausted max attempts
/// - Answer count matches question count
/// - Each submitted answer text matches a valid option
///
/// Compares selected **text** (not index) against the correct option text,
/// so frontend option shuffling doesn't break grading.
pub async fn submit_quiz(
    kv: &KvStore,
    config: &QuizConfig,
    claim_token: &str,
    answers: &[QuizAnswer],
) -> Result<QuizSubmitResponse, String> {
    // Load existing progress (or start fresh)
    let mut progress = get_quiz_progress(kv, claim_token)
        .await?
        .unwrap_or_else(|| new_progress(claim_token));

    // Attempt limit guard
    if progress.attempts >= config.max_attempts {
        return Err(format!(
            "no attempts remaining (used {}/{})",
            progress.attempts, config.max_attempts
        ));
    }

    // Answer count must match question count
    if answers.len() != config.questions.len() {
        return Err(format!(
            "expected {} answers, got {}",
            config.questions.len(),
            answers.len()
        ));
    }

    // Grade each question
    let mut explanations = Vec::with_capacity(config.questions.len());
    let mut correct_count = 0usize;

    for question in &config.questions {
        let selected = answers
            .iter()
            .find(|a| a.question_id == question.id)
            .map(|a| a.selected_text.trim().to_string())
            .unwrap_or_default();

        let correct_text = question
            .options
            .get(question.correct_index as usize)
            .map(|s| s.trim())
            .unwrap_or("");

        let is_correct = selected.eq_ignore_ascii_case(correct_text);
        if is_correct {
            correct_count += 1;
        }

        explanations.push(QuestionExplanation {
            question_id: question.id.clone(),
            correct: is_correct,
            explanation: question.explanation.clone(),
        });
    }

    // Calculate score percentage
    let score_percent = if config.questions.is_empty() {
        100u8
    } else {
        ((correct_count * 100) / config.questions.len()) as u8
    };

    let passed = score_percent >= config.passing_score_percent;

    // Update progress
    progress.attempts += 1;
    if score_percent > progress.best_score_percent {
        progress.best_score_percent = score_percent;
    }
    if passed && !progress.passed {
        progress.passed = true;
        progress.passed_at = Some(Utc::now().to_rfc3339());
    }

    // Record attempt
    progress.attempt_history.push(QuizAttempt {
        attempt_number: progress.attempts,
        answers: answers
            .iter()
            .map(|a| (a.question_id.clone(), a.selected_text.clone()))
            .collect(),
        score_percent,
        submitted_at: Utc::now().to_rfc3339(),
    });

    save_quiz_progress(kv, &progress).await?;

    let remaining = config.max_attempts.saturating_sub(progress.attempts);

    Ok(QuizSubmitResponse {
        attempt_number: progress.attempts,
        score_percent,
        passed,
        correct_count,
        total_questions: config.questions.len(),
        remaining_attempts: remaining,
        explanations,
    })
}

// ---------------------------------------------------------------------------
// Quiz status helper
// ---------------------------------------------------------------------------

/// Determine the quiz status for a claim token.
///
/// - `NotRequired` — no quiz config in KV
/// - `NotStarted`  — quiz exists, attendee hasn't attempted
/// - `InProgress`  — quiz exists, attempted but not yet passed
/// - `Passed`      — quiz passed, claim unlocked
pub async fn get_quiz_status(kv: &KvStore, claim_token: &str) -> Result<QuizStatus, String> {
    let config = get_quiz_config(kv).await?;
    match config {
        None => Ok(QuizStatus::NotRequired),
        Some(_) => {
            let progress = get_quiz_progress(kv, claim_token).await?;
            match progress {
                None => Ok(QuizStatus::NotStarted),
                Some(p) if p.passed => Ok(QuizStatus::Passed),
                Some(_) => Ok(QuizStatus::InProgress),
            }
        }
    }
}
