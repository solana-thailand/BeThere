use serde::{Deserialize, Serialize};

/// Response for a single attendee or attendee in a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendeeResponse {
    pub api_id: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: String,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub qr_code_url: Option<String>,
    /// Claim token for NFT/refund claim link (set after check-in).
    pub claim_token: Option<String>,
    pub participation_type: String,
    pub row_index: usize,
}

impl AttendeeResponse {
    pub fn from_attendee(attendee: &crate::models::attendee::Attendee) -> Self {
        Self {
            api_id: attendee.api_id.clone(),
            name: attendee.display_name().to_string(),
            email: attendee.email.clone(),
            ticket_name: attendee.ticket_name.clone(),
            approval_status: attendee.approval_status.to_string(),
            checked_in_at: attendee.checked_in_at.clone(),
            checked_in_by: attendee.checked_in_by.clone(),
            qr_code_url: attendee.qr_code_url.clone(),
            claim_token: attendee.claim_token.clone(),
            participation_type: attendee.participation_type.clone(),
            row_index: attendee.row_index,
        }
    }
}

/// Response after checking in an attendee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInResponse {
    pub api_id: String,
    pub name: String,
    pub checked_in_at: String,
    pub checked_in_by: String,
    /// Claim token for NFT/refund claim link (UUID v7).
    /// Frontend constructs the full claim URL using `window.location.origin + /claim/{token}`.
    pub claim_token: Option<String>,
    pub message: String,
}

/// Response after bulk generating QR codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateQrResponse {
    pub total: usize,
    pub generated: usize,
    pub skipped: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<QrGenerationDetail>,
}

/// Detail about a single QR code generation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrGenerationDetail {
    pub api_id: String,
    pub name: String,
    pub qr_code_url: String,
    pub status: QrGenerationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QrGenerationStatus {
    Generated,
    Skipped,
}

/// Check-in statistics for the admin dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_approved: usize,
    pub total_checked_in: usize,
    pub total_remaining: usize,
    pub check_in_percentage: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_check_ins: Vec<RecentCheckIn>,
}

/// A recent check-in entry for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentCheckIn {
    pub api_id: String,
    pub name: String,
    pub checked_in_at: String,
    pub checked_in_by: Option<String>,
}

/// Dynamic event metadata served from backend config.
/// Eliminates hardcoded event name/timestamps in the frontend WASM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    /// Full event name (e.g. "Solana x AI Builders: The Road to Mainnet #1 (Bangkok)")
    pub event_name: String,
    /// Event tagline / subtitle
    pub event_tagline: String,
    /// External event page URL
    pub event_link: String,
    /// Event start time as Unix epoch milliseconds
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds
    pub event_end_ms: i64,
}

/// Quiz requirement status for a claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuizStatus {
    /// No quiz configured for this event — claim works normally.
    #[default]
    NotRequired,
    /// Quiz exists but attendee hasn't attempted yet.
    NotStarted,
    /// Quiz exists, attendee attempted but hasn't passed.
    InProgress,
    /// Quiz passed — claim unlocked.
    Passed,
}

/// Response for GET /api/claim/{token} — look up an attendee by claim token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimLookupResponse {
    pub name: String,
    pub checked_in_at: String,
    pub claim_token: String,
    pub claimed: bool,
    pub claimed_at: Option<String>,
    /// Whether NFT minting is configured (all required secrets present).
    pub nft_available: bool,
    /// Pre-registered wallet address from column P.
    /// When present, the claim is locked to this wallet — any other address is rejected.
    /// `None` means no pre-registered wallet; any valid address may claim.
    pub locked_wallet: Option<String>,
    /// Dynamic event metadata (name, tagline, link, timestamps).
    pub event: EventConfig,
    /// Quiz requirement status for this attendee's claim.
    #[serde(default)]
    pub quiz_status: QuizStatus,
}

/// Response for POST /api/claim/{token} — mint cNFT and mark as claimed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResponse {
    pub name: String,
    pub asset_id: String,
    pub signature: String,
    pub wallet_address: String,
    pub claimed_at: String,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    pub cluster: String,
}

// ---------------------------------------------------------------------------
// Quiz types — activity-gated claim flow (Issue 002)
// ---------------------------------------------------------------------------

/// A single quiz question as stored in KV (includes correct answer — server-side only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestion {
    /// Unique question identifier (e.g. "q1", "q2").
    pub id: String,
    /// Question text displayed to attendee.
    pub text: String,
    /// Multiple-choice options.
    pub options: Vec<String>,
    /// Index of the correct option (0-based).
    pub correct_index: u8,
    /// Optional explanation shown after submission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// A quiz question as sent to the frontend (correct answer stripped).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestionPublic {
    pub id: String,
    pub text: String,
    pub options: Vec<String>,
}

/// Full quiz configuration stored in KV under key "questions".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizConfig {
    pub questions: Vec<QuizQuestion>,
    /// Percentage of correct answers required to pass (e.g. 60 = 60%).
    pub passing_score_percent: u8,
    /// Maximum submission attempts allowed per attendee.
    pub max_attempts: u8,
    /// Optional per-attempt timer in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_limit_seconds: Option<u16>,
}

/// Response for GET /api/quiz — quiz questions for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestionsResponse {
    pub questions: Vec<QuizQuestionPublic>,
    pub passing_score_percent: u8,
    pub max_attempts: u8,
    pub time_limit_seconds: Option<u16>,
}

/// A single answer in a quiz submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAnswer {
    pub question_id: String,
    /// Selected option text (not index — survives option shuffling).
    pub selected_text: String,
}

/// Request body for POST /api/quiz/{token}/submit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizSubmitRequest {
    pub answers: Vec<QuizAnswer>,
}

/// Per-question feedback after submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionExplanation {
    pub question_id: String,
    pub correct: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// Response for POST /api/quiz/{token}/submit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizSubmitResponse {
    /// Which attempt number this was (1-based).
    pub attempt_number: u8,
    /// Score as percentage (0-100).
    pub score_percent: u8,
    /// Whether the attendee passed.
    pub passed: bool,
    /// Number of correct answers.
    pub correct_count: usize,
    /// Total number of questions.
    pub total_questions: usize,
    /// Remaining attempts (0 = exhausted).
    pub remaining_attempts: u8,
    /// Per-question feedback.
    pub explanations: Vec<QuestionExplanation>,
}

/// Record of a single quiz attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAttempt {
    /// Attempt number (1-based).
    pub attempt_number: u8,
    /// Selected option text per question (question_id → selected_text).
    pub answers: Vec<(String, String)>,
    /// Score as percentage (0-100).
    pub score_percent: u8,
    /// ISO 8601 timestamp of submission.
    pub submitted_at: String,
}

/// Per-attendee quiz progress stored in KV under key "progress:{claim_token}".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QuizProgress {
    pub claim_token: String,
    /// Total attempts so far.
    pub attempts: u8,
    /// Best score achieved across all attempts.
    pub best_score_percent: u8,
    /// Whether the attendee has passed.
    pub passed: bool,
    /// ISO 8601 timestamp of when they passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed_at: Option<String>,
    /// History of all attempts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attempt_history: Vec<QuizAttempt>,
}

/// Response for GET /api/quiz/{token}/status — current quiz progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizStatusResponse {
    /// Attempts used so far.
    pub attempts: u8,
    /// Maximum attempts allowed.
    pub max_attempts: u8,
    /// Best score achieved (0-100).
    pub best_score_percent: u8,
    /// Whether the attendee has passed.
    pub passed: bool,
    /// Percentage required to pass.
    pub passing_threshold_percent: u8,
}
