//! Quiz storage with D1-first reads and KV fallback (Issue 002, P4).
//!
//! Quiz questions and per-attendee progress are primarily stored in D1.
//! KV is used as a fallback when D1 is unavailable or empty.
//!
//! Key schema (KV):
//!   "event:{id}:quiz:questions"                → QuizConfig (JSON)
//!   "event:{id}:quiz:progress:{claim_token}"   → QuizProgress (JSON)

use chrono::Utc;
use worker::{D1Database, KvStore};

use event_checkin_domain::models::api::{
    QuestionExplanation, QuizAnswer, QuizAttempt, QuizConfig, QuizProgress, QuizQuestion,
    QuizQuestionPublic, QuizQuestionsResponse, QuizStatus, QuizSubmitResponse,
};

use crate::db::quiz as d1_quiz;
use crate::event_store::{quiz_progress_key, quiz_questions_key};

// No TTL on quiz progress.
// Attendees may pass the quiz and claim their NFT hours later.
// If progress expired before claiming, they'd have to redo the quiz.

/// Default empty quiz config.
fn default_config() -> QuizConfig {
    QuizConfig {
        questions: Vec::new(),
        passing_score_percent: 60,
        max_attempts: 3,
        time_limit_seconds: None,
    }
}

// ---------------------------------------------------------------------------
// Quiz config (questions)
// ---------------------------------------------------------------------------

/// Read quiz configuration — D1 first, KV fallback.
/// Returns `None` if no quiz is configured.
pub async fn get_quiz_config(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
) -> Result<Option<QuizConfig>, String> {
    // D1 first
    if let Some(db) = d1 {
        match d1_quiz::get_quiz_config_from_d1(db, event_id).await {
            Ok(Some(json_str)) => {
                if let Ok(config) = serde_json::from_str::<QuizConfig>(&json_str) {
                    return Ok(Some(config));
                }
                // Corrupt JSON — fall through to KV
                tracing::warn!(event_id, "D1 quiz config JSON corrupt, falling back to KV");
            }
            Ok(None) => return Ok(None),
            Err(e) => {
                tracing::warn!(event_id, error = %e, "D1 quiz config read failed, falling back to KV");
            }
        }
    }

    // KV fallback
    let kv_ref = match kv {
        Some(k) => k,
        None => return Ok(None),
    };
    let key = quiz_questions_key(event_id);
    let raw: Option<String> = kv_ref
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read quiz config from KV: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json_str) => serde_json::from_str(&json_str)
            .map(Some)
            .map_err(|e| format!("failed to parse quiz config: {e}")),
    }
}

/// Write quiz configuration — D1 + KV (dual-write).
pub async fn save_quiz_config(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    config: &QuizConfig,
) -> Result<(), String> {
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize quiz config: {e:?}"))?;

    // D1 write (primary)
    if let Some(db) = d1
        && let Err(e) = d1_quiz::upsert_quiz_config_to_d1(db, event_id, &json_str).await
    {
        tracing::warn!(event_id, error = %e, "D1 quiz config write failed");
    }

    // KV write (fallback / legacy)
    if let Some(kv_ref) = kv {
        let key = quiz_questions_key(event_id);
        kv_ref
            .put(&key, &json_str)
            .map_err(|e| format!("failed to build quiz config put: {e:?}"))?
            .execute()
            .await
            .map_err(|e| format!("failed to write quiz config to KV: {e:?}"))?;
    }

    Ok(())
}

/// Add a single question to the quiz config.
/// Generates a sequential ID if not provided.
pub async fn add_question(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    mut question: QuizQuestion,
) -> Result<QuizQuestion, String> {
    let mut config = get_quiz_config(d1, kv, event_id)
        .await?
        .unwrap_or_else(default_config);

    // Generate ID if empty
    if question.id.is_empty() {
        let max_num = config
            .questions
            .iter()
            .filter_map(|q| q.id.strip_prefix('q').and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
        question.id = format!("q{}", max_num + 1);
    }

    // Check for duplicate ID
    if config.questions.iter().any(|q| q.id == question.id) {
        return Err(format!("question id '{}' already exists", question.id));
    }

    config.questions.push(question.clone());
    save_quiz_config(d1, kv, event_id, &config).await?;
    Ok(question)
}

/// Update a single question by ID.
pub async fn update_question(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    question_id: &str,
    updated: QuizQuestion,
) -> Result<QuizQuestion, String> {
    let mut config = get_quiz_config(d1, kv, event_id)
        .await?
        .ok_or_else(|| "no quiz configured".to_string())?;

    let question = config
        .questions
        .iter_mut()
        .find(|q| q.id == question_id)
        .ok_or_else(|| format!("question '{}' not found", question_id))?;

    // Preserve the original ID (path takes precedence)
    let preserved_id = question_id.to_string();
    *question = updated;
    question.id = preserved_id;

    let result = question.clone();
    save_quiz_config(d1, kv, event_id, &config).await?;
    Ok(result)
}

/// Delete a single question by ID.
pub async fn delete_question(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    question_id: &str,
) -> Result<(), String> {
    let mut config = get_quiz_config(d1, kv, event_id)
        .await?
        .ok_or_else(|| "no quiz configured".to_string())?;

    let original_len = config.questions.len();
    config.questions.retain(|q| q.id != question_id);
    if config.questions.len() == original_len {
        return Err(format!("question '{}' not found", question_id));
    }

    save_quiz_config(d1, kv, event_id, &config).await?;
    Ok(())
}

/// Toggle the enabled state of a single question.
pub async fn toggle_question(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    question_id: &str,
) -> Result<QuizQuestion, String> {
    let mut config = get_quiz_config(d1, kv, event_id)
        .await?
        .ok_or_else(|| "no quiz configured".to_string())?;

    let question = config
        .questions
        .iter_mut()
        .find(|q| q.id == question_id)
        .ok_or_else(|| format!("question '{}' not found", question_id))?;

    question.enabled = !question.enabled;
    let result = question.clone();
    save_quiz_config(d1, kv, event_id, &config).await?;
    Ok(result)
}

/// Convert full quiz config to public response (strips correct answers and disabled questions).
pub fn to_public_questions(config: &QuizConfig) -> QuizQuestionsResponse {
    QuizQuestionsResponse {
        questions: config
            .questions
            .iter()
            .filter(|q| q.enabled)
            .map(|q| QuizQuestionPublic {
                id: q.id.clone(),
                text: q.text.clone(),
                options: q.options.clone(),
                session_id: q.session_id.clone(),
                session_title: q.session_title.clone(),
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

/// Read quiz progress for an attendee — D1 first, KV fallback.
/// Returns `None` if no progress exists yet (hasn't attempted).
pub async fn get_quiz_progress(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    claim_token: &str,
) -> Result<Option<QuizProgress>, String> {
    // D1 first
    if let Some(db) = d1 {
        match d1_quiz::get_quiz_progress_from_d1(db, event_id, claim_token).await {
            Ok(Some(json_str)) => {
                if let Ok(progress) = serde_json::from_str::<QuizProgress>(&json_str) {
                    return Ok(Some(progress));
                }
                tracing::warn!(
                    event_id,
                    claim_token,
                    "D1 quiz progress JSON corrupt, falling back to KV"
                );
            }
            Ok(None) => return Ok(None),
            Err(e) => {
                tracing::warn!(event_id, claim_token, error = %e, "D1 quiz progress read failed, falling back to KV");
            }
        }
    }

    // KV fallback
    let kv_ref = match kv {
        Some(k) => k,
        None => return Ok(None),
    };
    let key = quiz_progress_key(event_id, claim_token);
    let raw: Option<String> = kv_ref
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read quiz progress from KV: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json_str) => serde_json::from_str(&json_str)
            .map(Some)
            .map_err(|e| format!("failed to parse quiz progress: {e}")),
    }
}

/// Write quiz progress for an attendee — D1 + KV (dual-write).
async fn save_quiz_progress(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    progress: &QuizProgress,
) -> Result<(), String> {
    let json_str = serde_json::to_string(progress)
        .map_err(|e| format!("failed to serialize quiz progress: {e:?}"))?;

    // D1 write (primary)
    if let Some(db) = d1
        && let Err(e) = d1_quiz::upsert_quiz_progress_to_d1(
            db,
            event_id,
            &progress.claim_token,
            &json_str,
            progress.passed,
            progress.attempts,
        )
        .await
    {
        tracing::warn!(event_id, error = %e, "D1 quiz progress write failed");
    }

    // KV write (fallback / legacy)
    if let Some(kv_ref) = kv {
        let key = quiz_progress_key(event_id, &progress.claim_token);
        kv_ref
            .put(&key, &json_str)
            .map_err(|e| format!("failed to build quiz progress put: {e:?}"))?
            .execute()
            .await
            .map_err(|e| format!("failed to write quiz progress to KV: {e:?}"))?;
    }

    Ok(())
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
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    config: &QuizConfig,
    claim_token: &str,
    answers: &[QuizAnswer],
) -> Result<QuizSubmitResponse, String> {
    // Load existing progress (or start fresh)
    let mut progress = get_quiz_progress(d1, kv, event_id, claim_token)
        .await?
        .unwrap_or_else(|| new_progress(claim_token));

    // Attempt limit guard
    if progress.attempts >= config.max_attempts {
        return Err(format!(
            "no attempts remaining (used {}/{})",
            progress.attempts, config.max_attempts
        ));
    }

    // Only grade enabled questions
    let enabled_questions: Vec<_> = config.questions.iter().filter(|q| q.enabled).collect();

    // Answer count must match enabled question count
    if answers.len() != enabled_questions.len() {
        return Err(format!(
            "expected {} answers, got {}",
            enabled_questions.len(),
            answers.len()
        ));
    }

    // Grade each enabled question
    let mut explanations = Vec::with_capacity(enabled_questions.len());
    let mut correct_count = 0usize;

    for question in &enabled_questions {
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
    let score_percent = if enabled_questions.is_empty() {
        100u8
    } else {
        ((correct_count * 100) / enabled_questions.len()) as u8
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

    save_quiz_progress(d1, kv, event_id, &progress).await?;

    let remaining = config.max_attempts.saturating_sub(progress.attempts);

    Ok(QuizSubmitResponse {
        attempt_number: progress.attempts,
        score_percent,
        passed,
        correct_count,
        total_questions: enabled_questions.len(),
        remaining_attempts: remaining,
        explanations,
    })
}

// ---------------------------------------------------------------------------
// Quiz status helper
// ---------------------------------------------------------------------------

/// Determine the quiz status for a claim token.
///
/// - `NotRequired` — no quiz config
/// - `NotStarted`  — quiz exists, attendee hasn't attempted
/// - `InProgress`  — quiz exists, attempted but not yet passed
/// - `Passed`      — quiz passed, claim unlocked
pub async fn get_quiz_status(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    event_id: &str,
    claim_token: &str,
) -> Result<QuizStatus, String> {
    let config = get_quiz_config(d1, kv, event_id).await?;
    match config {
        None => Ok(QuizStatus::NotRequired),
        Some(_) => {
            let progress = get_quiz_progress(d1, kv, event_id, claim_token).await?;
            match progress {
                None => Ok(QuizStatus::NotStarted),
                Some(p) if p.passed => Ok(QuizStatus::Passed),
                Some(_) => Ok(QuizStatus::InProgress),
            }
        }
    }
}
