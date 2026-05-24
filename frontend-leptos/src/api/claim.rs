//! Claim, quiz, and adventure types and API functions (public — no auth required).

use serde::{Deserialize, Serialize};

use super::types::{ApiError, ApiResponse};
use super::{api_base, cached_get};

// ===== Claim API types =====

/// Dynamic event metadata served from backend config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventConfig {
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub event_tagline: String,
    #[serde(default)]
    pub event_link: String,
    /// Event start time as Unix epoch milliseconds.
    #[serde(default)]
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds.
    #[serde(default)]
    pub event_end_ms: i64,
}

/// Quiz requirement status for a claim.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuizStatus {
    #[default]
    NotRequired,
    NotStarted,
    InProgress,
    Passed,
}

/// Response data for GET /api/claim/{token} — attendee claim lookup.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaimLookupData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub checked_in_at: String,
    #[serde(default)]
    pub claim_token: String,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub claimed_at: Option<String>,
    /// Whether NFT minting is configured on the backend.
    #[serde(default = "super::types::default_true")]
    pub nft_available: bool,
    /// Pre-registered wallet address from column P.
    /// When present, the claim is locked to this wallet — any other address is rejected.
    #[serde(default)]
    pub locked_wallet: Option<String>,
    /// Dynamic event metadata (name, tagline, link, timestamps).
    #[serde(default)]
    pub event: EventConfig,
    /// Quiz requirement status for this attendee's claim.
    #[serde(default)]
    pub quiz_status: QuizStatus,
    /// Total number of attendees checked in for this event.
    #[serde(default)]
    pub total_checked_in: usize,
    /// Total number of attendees who have claimed their NFT.
    #[serde(default)]
    pub total_claimed: usize,
    /// Attendee's API ID (for deposit page link: /deposit/{api_id}).
    #[serde(default)]
    pub api_id: String,
    /// Event ID (for deposit page link query param).
    #[serde(default)]
    pub event_id: String,
    /// Whether deposit is enabled for this event.
    #[serde(default)]
    pub deposit_enabled: bool,
    /// Deposit amount in USDC (smallest unit, e.g. 15000000 = 15 USDC).
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    /// Deposit amount in THB (e.g. 500).
    #[serde(default)]
    pub deposit_amount_thb: u64,
    /// Attendee's participation type ("In-Person", "Online", etc.).
    #[serde(default)]
    pub participation_type: String,
    /// Transaction signature from the finalized claim lock KV (if available).
    #[serde(default)]
    pub claimed_signature: Option<String>,
    /// Asset ID from the finalized claim lock KV (if available).
    #[serde(default)]
    pub claimed_asset_id: Option<String>,
    /// Wallet address from the finalized claim lock KV (if available).
    #[serde(default)]
    pub claimed_wallet: Option<String>,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    #[serde(default)]
    pub cluster: Option<String>,
}

/// Response data for POST /api/claim/{token} — NFT mint result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaimMintData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub asset_id: String,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub wallet_address: String,
    #[serde(default)]
    pub claimed_at: String,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    #[serde(default)]
    pub cluster: String,
}

// ===== Quiz API types =====

/// A single quiz question as served to the frontend (no correct answer).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizQuestionPublic {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub session_title: Option<String>,
}

/// Response data for GET /api/quiz — quiz questions and config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizQuestionsData {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub questions: Vec<QuizQuestionPublic>,
    #[serde(default)]
    pub passing_score_percent: u8,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub time_limit_seconds: Option<u16>,
}

/// A single answer in a quiz submission (text-based, not index).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAnswer {
    pub question_id: String,
    pub selected_text: String,
}

/// Per-question feedback after submission.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionExplanation {
    #[serde(default)]
    pub question_id: String,
    #[serde(default)]
    pub correct: bool,
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Response data for POST /api/quiz/{token}/submit — scored result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizSubmitData {
    #[serde(default)]
    pub attempt_number: u8,
    #[serde(default)]
    pub score_percent: u8,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub correct_count: usize,
    #[serde(default)]
    pub total_questions: usize,
    #[serde(default)]
    pub remaining_attempts: u8,
    #[serde(default)]
    pub explanations: Vec<QuestionExplanation>,
}

/// Response data for GET /api/quiz/{token}/status — quiz progress.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizStatusData {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub quiz_status: String,
    #[serde(default)]
    pub attempts: u8,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub best_score_percent: u8,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub passing_threshold_percent: u8,
}

// ===== Adventure types =====

/// Adventure status from GET /api/adventure/{token}/status
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdventureStatusType {
    #[default]
    NotRequired,
    NotStarted,
    InProgress,
    Passed,
}

/// Level score from the adventure API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdventureLevelScore {
    #[serde(default)]
    pub moves: u32,
    #[serde(default)]
    pub puzzles_solved: u32,
    #[serde(default)]
    pub time_seconds: u32,
    #[serde(default)]
    pub stars: u8,
}

/// Adventure progress data from the API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdventureProgressData {
    #[serde(default)]
    pub claim_token: String,
    #[serde(default)]
    pub levels_completed: Vec<String>,
    #[serde(default)]
    pub scores: std::collections::HashMap<String, AdventureLevelScore>,
    #[serde(default)]
    pub total_moves: u32,
    #[serde(default)]
    pub total_time_seconds: u32,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub passed_at: Option<String>,
    #[serde(default)]
    pub last_played_at: Option<String>,
}

/// Response from GET /api/adventure/{token}/status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdventureStatusData {
    #[serde(default)]
    pub status: AdventureStatusType,
    #[serde(default)]
    pub progress: Option<AdventureProgressData>,
}

/// Request body for POST /api/adventure/{token}/save
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdventureSaveBody {
    pub claim_token: String,
    pub level_id: String,
    pub score: AdventureLevelScore,
}

// ===== Claim API functions =====

/// GET /api/claim/{token}
/// Look up an attendee's claim status by their claim token.
///
/// Public endpoint — no authentication required.
/// Results are cached client-side for 30 seconds (B5).
pub async fn get_claim(token: &str) -> Result<ClaimLookupData, ApiError> {
    let path = format!("/claim/{token}");
    let json = cached_get(&path).await?;

    let wrapper: ApiResponse<ClaimLookupData> =
        serde_json::from_str(&json).map_err(|e| ApiError {
            message: format!("Failed to parse claim response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Quiz API functions =====

/// GET /api/quiz
/// Fetch quiz questions for the frontend (no correct answers).
///
/// Public endpoint — no authentication required.
pub async fn get_quiz() -> Result<QuizQuestionsData, ApiError> {
    let url = format!("{}/quiz", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Quiz fetch failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<QuizQuestionsData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse quiz response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/quiz/{token}/submit
/// Submit quiz answers for scoring.
///
/// Public endpoint — no authentication required.
/// The attendee must be checked in (valid claim token).
pub async fn submit_quiz(
    token: &str,
    answers: &[QuizAnswer],
) -> Result<QuizSubmitData, ApiError> {
    let url = format!("{}/quiz/{token}/submit", api_base());
    let body = serde_json::json!({ "answers": answers });

    let response = gloo::net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap_or_default())
        .map_err(|e| ApiError {
            message: format!("Failed to build request: {e}"),
            status: 0,
        })?
        .send()
        .await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Quiz submit failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<QuizSubmitData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse quiz submit response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/quiz/{token}/status
/// Get quiz progress for an attendee.
///
/// Public endpoint — no authentication required.
pub async fn get_quiz_status(token: &str) -> Result<QuizStatusData, ApiError> {
    let url = format!("{}/quiz/{token}/status", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Quiz status fetch failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<QuizStatusData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse quiz status response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/claim/{token}
/// Mint a compressed NFT to the given wallet address.
///
/// Public endpoint — no authentication required.
/// The attendee must be checked in and not already claimed.
pub async fn post_claim(token: &str, wallet_address: &str) -> Result<ClaimMintData, ApiError> {
    let url = format!("{}/claim/{token}", api_base());
    let body = serde_json::json!({ "wallet_address": wallet_address });

    let response = gloo::net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap_or_default())
        .map_err(|e| ApiError {
            message: format!("Failed to build request: {e}"),
            status: 0,
        })?
        .send()
        .await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Claim mint failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<ClaimMintData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse mint response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Adventure API functions =====

/// GET /api/adventure/{token}/status
/// Get adventure status and progress for a claim token.
///
/// Public endpoint — no authentication required.
pub async fn get_adventure_status(token: &str) -> Result<AdventureStatusData, ApiError> {
    let url = format!("{}/adventure/{token}/status", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Adventure status fetch failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<AdventureStatusData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse adventure status response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/adventure/{token}/save
/// Save level completion progress.
///
/// Public endpoint — no authentication required.
pub async fn save_adventure_progress(
    token: &str,
    level_id: &str,
    score: &AdventureLevelScore,
) -> Result<AdventureProgressData, ApiError> {
    let url = format!("{}/adventure/{token}/save", api_base());
    let body = AdventureSaveBody {
        claim_token: token.to_string(),
        level_id: level_id.to_string(),
        score: score.clone(),
    };

    let response = gloo::net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap_or_default())
        .map_err(|e| ApiError {
            message: format!("Failed to build request: {e}"),
            status: 0,
        })?
        .send()
        .await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Adventure save failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    // Response is { success: true, data: { progress: ... } }
    #[derive(Debug, Default, Deserialize)]
    struct SaveResponse {
        #[serde(default)]
        progress: AdventureProgressData,
    }
    let wrapper: ApiResponse<SaveResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse adventure save response: {e}"),
            status: 0,
        })?;

    wrapper
        .data
        .map(|d| d.progress)
        .ok_or_else(|| ApiError {
            message: wrapper.error.unwrap_or("No data".to_string()),
            status: 0,
        })
}
