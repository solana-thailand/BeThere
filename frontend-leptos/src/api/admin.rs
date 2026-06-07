//! Admin quiz, adventure config, audit, on-chain events, escrow cancel status.

use serde::{Deserialize, Serialize};

use super::types::{ApiError, ApiResponse, default_true};
use super::{api_base, api_get, api_post_json, api_put_json, fetch::response_json};

// ===== Admin Quiz Types =====

/// A quiz question as stored in the admin config (includes correct answer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestionAdmin {
    pub id: String,
    pub text: String,
    pub options: Vec<String>,
    pub correct_index: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub session_title: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Full quiz config for admin management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizConfigAdmin {
    pub questions: Vec<QuizQuestionAdmin>,
    pub passing_score_percent: u8,
    pub max_attempts: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit_seconds: Option<u16>,
}

/// Response from GET /api/admin/quiz.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminQuizData {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub questions: Vec<QuizQuestionAdmin>,
    #[serde(default)]
    pub passing_score_percent: u8,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub time_limit_seconds: Option<u16>,
}

/// Response from POST /api/admin/quiz.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminQuizSaveData {
    pub questions_count: usize,
    pub passing_score_percent: u8,
    pub max_attempts: u8,
}

// ===== Adventure Admin Types =====

/// Adventure config from GET /api/admin/adventure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdventureConfigData {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub required_level: Option<usize>,
}

// ===== Audit Trail Types =====

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub description: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Response from GET /api/events/{id}/audit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditResponse {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub entries: Vec<AuditEntry>,
}

// ===== On-Chain Escrow Events Types =====

/// On-chain escrow instruction type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EscrowInstruction {
    CreateEvent,
    Deposit,
    MarkCheckedIn,
    Refund,
    ClaimForfeited,
    CloseEvent,
    DeactivateEvent,
    CloseDeposit,
    Unknown,
}

impl EscrowInstruction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CreateEvent => "Create Event",
            Self::Deposit => "Deposit",
            Self::MarkCheckedIn => "Check In",
            Self::Refund => "Refund",
            Self::ClaimForfeited => "Claim Forfeited",
            Self::CloseEvent => "Close Event",
            Self::DeactivateEvent => "Deactivate",
            Self::CloseDeposit => "Close Deposit",
            Self::Unknown => "Unknown",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::CreateEvent => "#6366f1",    // indigo
            Self::Deposit => "#3b82f6",       // blue
            Self::MarkCheckedIn => "#22c55e",  // green
            Self::Refund => "#eab308",        // yellow
            Self::ClaimForfeited => "#f97316", // orange
            Self::CloseEvent => "#ef4444",     // red
            Self::DeactivateEvent => "#a855f7", // purple
            Self::CloseDeposit => "#64748b",   // slate
            Self::Unknown => "#94a3b8",        // gray
        }
    }
}

/// A single on-chain escrow event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainEvent {
    pub signature: String,
    pub slot: u64,
    pub block_time: i64,
    pub instruction: EscrowInstruction,
    pub escrow_address: String,
    #[serde(default)]
    pub organizer: Option<String>,
    #[serde(default)]
    pub attendee: Option<String>,
    #[serde(default)]
    pub amount: Option<u64>,
    pub indexed_at: String,
}

/// Response for GET /api/escrow/events/{event_id}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnchainEventsResponse {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub escrow_address: String,
    #[serde(default)]
    pub events: Vec<OnChainEvent>,
}

// ===== Cancellation Workflow Types =====

/// Response from POST /api/refund/batch-thb — batch THB refund result.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct BatchThbRefundResponse {
    pub refunded: u32,
    pub skipped: u32,
    pub total_thb_deposits: u32,
}

/// A USDC deposit in the refund queue (requires attendee signature).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct UsdcQueueItem {
    pub attendee_id: String,
    pub wallet_address: Option<String>,
    pub amount: u64,
    pub deposited_at: String,
}

/// Response from GET /api/escrow/refund-queue — USDC refund queue.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct UsdcRefundQueueResponse {
    pub event_id: String,
    pub usdc_pending: usize,
    pub queue: Vec<UsdcQueueItem>,
}

/// Response from GET /api/escrow/cancel-status — cancellation status overview.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CancelStatusResponse {
    pub event_id: String,
    pub event_name: String,
    pub escrow_status: String,
    pub usdc_deposits: usize,
    pub usdc_verified: usize,
    pub usdc_refundable: usize,
    pub thb_deposits: usize,
    pub thb_refunded: usize,
    pub thb_pending_refund: usize,
}

// ===== Admin Quiz Management API =====

/// Get the full quiz configuration (admin only, includes correct answers).
pub async fn get_admin_quiz(event_id: Option<&str>) -> Result<AdminQuizData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/admin/quiz?event_id={eid}"),
        _ => "/admin/quiz".to_string(),
    };
    let response = api_get(&path).await?;
    let result: ApiResponse<AdminQuizData> = response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse admin quiz response: {e}"),
        status: 0,
    })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

/// Save quiz configuration (admin only).
pub async fn put_admin_quiz(config: &QuizConfigAdmin, event_id: Option<&str>) -> Result<AdminQuizSaveData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/admin/quiz?event_id={eid}"),
        _ => "/admin/quiz".to_string(),
    };
    api_post_json(&path, config).await
}

/// DELETE /api/admin/quiz/questions/{id} — delete a single question.
pub async fn delete_quiz_question(
    question_id: &str,
    event_id: Option<&str>,
) -> Result<(), ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/admin/quiz/questions/{question_id}?event_id={eid}"),
        _ => format!("/admin/quiz/questions/{question_id}"),
    };
    let response = super::api_delete(&path).await?;
    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Delete failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Delete failed".to_string()),
            status: response.status(),
        });
    }
    Ok(())
}

/// PATCH /api/admin/quiz/questions/{id}/toggle — toggle question enabled state.
pub async fn toggle_quiz_question(
    question_id: &str,
    event_id: Option<&str>,
) -> Result<QuizQuestionAdmin, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => {
            format!("/admin/quiz/questions/{question_id}/toggle?event_id={eid}")
        }
        _ => format!("/admin/quiz/questions/{question_id}/toggle"),
    };
    let url = format!("{}{path}", super::api_base());
    let token = crate::auth::get_token();
    let mut hdrs: Vec<(&str, &str)> = vec![("Content-Type", "application/json")];
    let auth_val;
    if let Some(ref t) = token {
        auth_val = format!("Bearer {t}");
        hdrs.push(("Authorization", &auth_val));
    }
    let response = super::fetch::patch(&url, &hdrs, None).await?;

    if response.status() == 401 {
        crate::auth::clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    if !response.ok() {
        let body: ApiResponse<()> = super::fetch::response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Toggle failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Toggle failed".to_string()),
            status: response.status(),
        });
    }

    // Backend returns { success: true, data: { question: {...} } }
    let raw: serde_json::Value = super::fetch::response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse toggle response: {e}"),
        status: 0,
    })?;

    if !raw["success"].as_bool().unwrap_or(false) {
        let err_msg = raw["error"].as_str().unwrap_or("Toggle failed");
        return Err(ApiError {
            message: err_msg.to_string(),
            status: 0,
        });
    }

    let question_val = raw
        .get("data")
        .and_then(|d| d.get("question"))
        .ok_or_else(|| ApiError {
            message: "No question data in response".to_string(),
            status: 0,
        })?;

    serde_json::from_value(question_val.clone()).map_err(|e| ApiError {
        message: format!("Failed to deserialize question: {e}"),
        status: 0,
    })
}

// ===== Adventure Admin API =====

/// GET /api/admin/adventure
/// Get adventure config for the active event.
pub async fn get_admin_adventure_config(
    event_id: Option<&str>,
) -> Result<AdventureConfigData, ApiError> {
    let mut url = format!("{}/admin/adventure", api_base());
    if let Some(eid) = event_id {
        url = format!("{url}?event_id={eid}");
    }
    let response = super::fetch::get(&url, &[]).await?;

    if !response.ok() {
        let body: ApiResponse<()> = super::fetch::response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to fetch adventure config".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Failed to fetch adventure config".to_string()),
            status: response.status(),
        });
    }

    #[derive(Default, Deserialize)]
    struct ConfigResponse {
        #[serde(default)]
        config: Option<AdventureConfigData>,
    }

    let wrapper: ApiResponse<ConfigResponse> = super::fetch::response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse adventure config: {e}"),
        status: 0,
    })?;

    wrapper
        .data
        .and_then(|d| d.config)
        .ok_or_else(|| ApiError {
            message: "No config data".to_string(),
            status: 0,
        })
}

/// PUT /api/admin/adventure
/// Update adventure config for the active event.
pub async fn put_admin_adventure_config(
    config: &AdventureConfigData,
    event_id: Option<&str>,
) -> Result<AdventureConfigData, ApiError> {
    let mut url = format!("{}/admin/adventure", api_base());
    if let Some(eid) = event_id {
        url = format!("{url}?event_id={eid}");
    }
    let body_str = serde_json::to_string(config).unwrap_or_default();
    let hdrs = [("Content-Type", "application/json")];
    let response = super::fetch::put(&url, &hdrs, Some(body_str)).await?;

    if !response.ok() {
        let body: ApiResponse<()> = super::fetch::response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to save adventure config".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Failed to save adventure config".to_string()),
            status: response.status(),
        });
    }

    // Return the config we sent (backend echoes it back)
    Ok(config.clone())
}

// ===== Form Config API (Issue #049 Phase 2) =====

/// Form field configuration (mirrors backend `FormFieldConfig`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FormFieldConfigAdmin {
    /// Stable unique ID for Leptos `For` key (not serialized to backend).
    #[serde(skip)]
    pub uid: usize,
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: FormFieldTypeAdmin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub profile_field: bool,
}

/// Supported form field types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FormFieldTypeAdmin {
    Text,
    Textarea,
    Select,
    Multiselect,
}

impl FormFieldTypeAdmin {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Textarea => "Textarea",
            Self::Select => "Select",
            Self::Multiselect => "Multi-select",
        }
    }

    /// All variants in order.
    pub fn all() -> &'static [Self] {
        &[Self::Text, Self::Textarea, Self::Select, Self::Multiselect]
    }
}

/// Registration form config (mirrors backend `RegistrationFormConfig`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationFormConfigAdmin {
    #[serde(default = "default_section_label")]
    pub section_label: String,
    pub fields: Vec<FormFieldConfigAdmin>,
}

fn default_section_label() -> String {
    "About You (optional \u{2014} helps us plan better events)".to_string()
}

impl Default for RegistrationFormConfigAdmin {
    fn default() -> Self {
        Self {
            section_label: default_section_label(),
            fields: vec![
                FormFieldConfigAdmin {
                    uid: 1,
                    key: "experience_level".to_string(),
                    label: "Experience level".to_string(),
                    field_type: FormFieldTypeAdmin::Select,
                    options: Some(vec![
                        "Beginner".to_string(),
                        "Intermediate".to_string(),
                        "Senior".to_string(),
                        "Tech Lead".to_string(),
                    ]),
                    required: false,
                    profile_field: true,
                },
                FormFieldConfigAdmin {
                    uid: 2,
                    key: "tech_stack".to_string(),
                    label: "Technologies you use".to_string(),
                    field_type: FormFieldTypeAdmin::Multiselect,
                    options: Some(vec![
                        "Rust".to_string(),
                        "TypeScript".to_string(),
                        "Python".to_string(),
                        "Solidity".to_string(),
                        "Move".to_string(),
                        "Go".to_string(),
                        "C++".to_string(),
                    ]),
                    required: false,
                    profile_field: true,
                },
                FormFieldConfigAdmin {
                    uid: 3,
                    key: "interests".to_string(),
                    label: "Topics that interest you".to_string(),
                    field_type: FormFieldTypeAdmin::Multiselect,
                    options: Some(vec![
                        "DeFi".to_string(),
                        "NFT".to_string(),
                        "ZK Proofs".to_string(),
                        "Infrastructure".to_string(),
                        "Gaming".to_string(),
                        "AI/ML".to_string(),
                        "Mobile".to_string(),
                    ]),
                    required: false,
                    profile_field: true,
                },
            ],
        }
    }
}

/// Valid profile field keys (must match developer_profiles columns).
pub const VALID_PROFILE_KEYS: &[&str] = &[
    "experience_level",
    "tech_stack",
    "interests",
    "github_handle",
    "discord_handle",
    "twitter_handle",
    "primary_role",
    "learning_goals",
    "company_org",
    "location_city",
];

/// GET /api/events/{id}/form-config
pub async fn get_form_config(event_id: &str) -> Result<RegistrationFormConfigAdmin, ApiError> {
    let path = format!("/events/{event_id}/form-config");
    let response = api_get(&path).await?;

    let wrapper: ApiResponse<RegistrationFormConfigAdmin> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse form config: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// PUT /api/events/{id}/form-config
pub async fn put_form_config(
    event_id: &str,
    config: &RegistrationFormConfigAdmin,
) -> Result<RegistrationFormConfigAdmin, ApiError> {
    let path = format!("/events/{event_id}/form-config");
    api_put_json(&path, config).await
}

// ===== Audit Trail API =====

/// GET /api/events/{id}/audit — fetch audit trail for an event.
pub async fn get_event_audit(event_id: &str) -> Result<AuditResponse, ApiError> {
    let path = format!("/events/{event_id}/audit");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to fetch audit trail".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let result: ApiResponse<AuditResponse> = response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse audit response: {e}"),
        status: 0,
    })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

// ===== On-Chain Escrow Events API =====

/// GET /api/escrow/events/{event_id} — fetch indexed on-chain events
pub async fn get_onchain_events(event_id: &str) -> Result<OnchainEventsResponse, ApiError> {
    let path = format!("/escrow/events/{event_id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to fetch on-chain events".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let result: ApiResponse<OnchainEventsResponse> = response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse on-chain events: {e}"),
        status: 0,
    })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

// ===== Cancellation Workflow API =====

/// POST /api/refund/batch-thb — batch refund all THB deposits for an event
pub async fn batch_thb_refund(event_id: &str) -> Result<BatchThbRefundResponse, ApiError> {
    let body = serde_json::json!({ "event_id": event_id });
    api_post_json("/refund/batch-thb", &body).await
}

/// GET /api/escrow/refund-queue?event_id=xxx — list USDC deposits for cancellation
pub async fn get_usdc_refund_queue(event_id: &str) -> Result<UsdcRefundQueueResponse, ApiError> {
    let path = format!("/escrow/refund-queue?event_id={event_id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get refund queue".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<UsdcRefundQueueResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse refund queue: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/escrow/cancel-status?event_id=xxx — get cancellation status
pub async fn get_cancel_status(event_id: &str) -> Result<CancelStatusResponse, ApiError> {
    let path = format!("/escrow/cancel-status?event_id={event_id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get cancel status".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<CancelStatusResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse cancel status: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}
