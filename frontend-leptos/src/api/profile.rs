//! Developer profile API types and client functions.

use serde::{Deserialize, Serialize};

use super::{ApiError, api_get_no_cache, api_put_json};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Developer profile data returned by GET /api/my-profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DeveloperProfile {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub wallet_address: Option<String>,
    #[serde(default)]
    pub github_handle: Option<String>,
    #[serde(default)]
    pub discord_handle: Option<String>,
    #[serde(default)]
    pub twitter_handle: Option<String>,
    #[serde(default)]
    pub experience_level: Option<String>,
    #[serde(default)]
    pub primary_role: Option<String>,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    #[serde(default)]
    pub interests: Vec<String>,
    #[serde(default)]
    pub learning_goals: String,
    #[serde(default)]
    pub company_org: String,
    #[serde(default)]
    pub location_city: String,
    #[serde(default)]
    pub consent_outreach: bool,
    #[serde(default)]
    pub total_events: i64,
}

/// Profile update request body for PUT /api/my-profile.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateProfileBody {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub github_handle: String,
    #[serde(default)]
    pub discord_handle: String,
    #[serde(default)]
    pub twitter_handle: String,
    #[serde(default)]
    pub primary_role: String,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    #[serde(default)]
    pub interests: Vec<String>,
    #[serde(default)]
    pub learning_goals: String,
    #[serde(default)]
    pub company_org: String,
    #[serde(default)]
    pub location_city: String,
    #[serde(default)]
    pub consent_outreach: bool,
}

impl From<&DeveloperProfile> for UpdateProfileBody {
    fn from(p: &DeveloperProfile) -> Self {
        Self {
            display_name: p.display_name.clone(),
            github_handle: p.github_handle.clone().unwrap_or_default(),
            discord_handle: p.discord_handle.clone().unwrap_or_default(),
            twitter_handle: p.twitter_handle.clone().unwrap_or_default(),
            primary_role: p.primary_role.clone().unwrap_or_default(),
            tech_stack: p.tech_stack.clone(),
            interests: p.interests.clone(),
            learning_goals: p.learning_goals.clone(),
            company_org: p.company_org.clone(),
            location_city: p.location_city.clone(),
            consent_outreach: p.consent_outreach,
        }
    }
}

// ---------------------------------------------------------------------------
// API functions
// ---------------------------------------------------------------------------

/// GET /api/my-profile — fetch the current user's developer profile.
pub async fn get_my_profile() -> Result<DeveloperProfile, ApiError> {
    let response = api_get_no_cache("/my-profile").await?;

    if !response.ok() {
        let status = response.status();
        let body = super::fetch::response_text(&response).await.unwrap_or_default();
        return Err(ApiError {
            message: format!("Failed to get profile ({status}): {body}"),
            status,
        });
    }

    let result: super::types::ApiResponse<DeveloperProfile> =
        super::fetch::response_json(&response)
            .await
            .map_err(|e| ApiError {
                message: format!("Failed to parse profile: {e}"),
                status: 0,
            })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in profile response".to_string(),
        status: 0,
    })
}

/// PUT /api/my-profile — update the current user's developer profile.
pub async fn update_my_profile(body: &UpdateProfileBody) -> Result<DeveloperProfile, ApiError> {
    api_put_json("/my-profile", body).await
}

// ---------------------------------------------------------------------------
// Constants for tag options
// ---------------------------------------------------------------------------

/// Predefined interest tag options for the profile form.
pub const INTEREST_OPTIONS: &[&str] = &[
    "DeFi",
    "NFT",
    "Gaming",
    "ZK / Privacy",
    "AI + Blockchain",
    "DAO / Governance",
    "Infrastructure",
    "Mobile",
    "Payments",
    "Tokenization",
    "Security",
    "Solana",
    "Ethereum",
    "Cross-chain",
];

/// Predefined role options for the profile form.
pub const ROLE_OPTIONS: &[&str] = &[
    "Developer",
    "Designer",
    "Product Manager",
    "Founder / CEO",
    "Community Manager",
    "DevRel",
    "Investor",
    "Researcher",
    "Student",
    "Other",
];
