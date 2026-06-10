//! Attendee-facing developer profile API.
//!
//! GET /api/my-profile — read own profile (attendee-authed)
//! PUT /api/my-profile — update own profile (attendee-authed)

use axum::{Extension, extract::State};
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Profile data returned to the attendee.
#[derive(Serialize, Clone)]
pub struct MyProfileResponse {
    pub email: String,
    pub display_name: String,
    pub wallet_address: Option<String>,
    pub github_handle: Option<String>,
    pub discord_handle: Option<String>,
    pub twitter_handle: Option<String>,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub tech_stack: Vec<String>,
    pub interests: Vec<String>,
    pub learning_goals: String,
    pub company_org: String,
    pub location_city: String,
    pub consent_outreach: bool,
    pub total_events: i64,
}

/// Profile update request from the attendee.
#[derive(Deserialize, Clone)]
pub struct UpdateProfileRequest {
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a JSON array string into a Vec<String>. Returns empty vec on failure.
fn parse_json_array(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/my-profile
///
/// Returns the developer profile for the currently authenticated attendee.
/// Creates an empty profile row if one doesn't exist yet.
#[worker::send]
pub async fn get_my_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<MyProfileResponse>, WorkerError> {
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))?;

    let profile = crate::db::developers::get_developer_profile(d1, &claims.email)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch profile: {e}")))?
        .unwrap_or_else(|| {
            // Return a default empty profile for new users
            crate::db::developers::DeveloperProfileRow {
                email: claims.email.clone(),
                display_name: String::new(),
                wallet_address: None,
                github_handle: None,
                discord_handle: None,
                twitter_handle: None,
                experience_level: None,
                primary_role: None,
                tech_stack: "[]".to_string(),
                interests: "[]".to_string(),
                learning_goals: String::new(),
                expectations: String::new(),
                company_org: String::new(),
                location_city: String::new(),
                consent_outreach: 0,
                first_seen_at: String::new(),
                last_active_at: String::new(),
                total_events: 0,
                badges_earned: "[]".to_string(),
            }
        });

    Ok(ApiOk::new(MyProfileResponse {
        email: profile.email,
        display_name: profile.display_name,
        wallet_address: profile.wallet_address,
        github_handle: profile.github_handle,
        discord_handle: profile.discord_handle,
        twitter_handle: profile.twitter_handle,
        experience_level: profile.experience_level,
        primary_role: profile.primary_role,
        tech_stack: parse_json_array(&profile.tech_stack),
        interests: parse_json_array(&profile.interests),
        learning_goals: profile.learning_goals,
        company_org: profile.company_org,
        location_city: profile.location_city,
        consent_outreach: profile.consent_outreach != 0,
        total_events: profile.total_events,
    }))
}

/// PUT /api/my-profile
///
/// Updates the developer profile for the currently authenticated attendee.
/// Creates a new profile if one doesn't exist.
#[worker::send]
pub async fn update_my_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::Json(body): axum::Json<UpdateProfileRequest>,
) -> Result<ApiOk<MyProfileResponse>, WorkerError> {
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))?;

    // Serialize JSON array fields
    let tech_stack_json =
        serde_json::to_string(&body.tech_stack).unwrap_or_else(|_| "[]".to_string());
    let interests_json =
        serde_json::to_string(&body.interests).unwrap_or_else(|_| "[]".to_string());
    let consent_val = if body.consent_outreach { 1 } else { 0 };

    // Build the upsert SQL — insert or update all editable fields
    let sql = format!(
        "INSERT INTO developer_profiles \
         (email, display_name, github_handle, discord_handle, twitter_handle, \
          primary_role, tech_stack, interests, learning_goals, company_org, \
          location_city, consent_outreach, first_seen_at, last_active_at, \
          total_events, updated_at) \
         VALUES ('{email}', '{display_name}', '{github}', '{discord}', '{twitter}', \
          '{primary_role}', '{tech_stack}', '{interests}', '{learning_goals}', \
          '{company_org}', '{location_city}', {consent_val}, \
          datetime('now'), datetime('now'), 0, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
          display_name = excluded.display_name, \
          github_handle = excluded.github_handle, \
          discord_handle = excluded.discord_handle, \
          twitter_handle = excluded.twitter_handle, \
          primary_role = excluded.primary_role, \
          tech_stack = excluded.tech_stack, \
          interests = excluded.interests, \
          learning_goals = excluded.learning_goals, \
          company_org = excluded.company_org, \
          location_city = excluded.location_city, \
          consent_outreach = excluded.consent_outreach, \
          updated_at = datetime('now')",
        email = claims.email.replace('\'', "''"),
        display_name = body.display_name.replace('\'', "''"),
        github = body.github_handle.replace('\'', "''"),
        discord = body.discord_handle.replace('\'', "''"),
        twitter = body.twitter_handle.replace('\'', "''"),
        primary_role = body.primary_role.replace('\'', "''"),
        tech_stack = tech_stack_json.replace('\'', "''"),
        interests = interests_json.replace('\'', "''"),
        learning_goals = body.learning_goals.replace('\'', "''"),
        company_org = body.company_org.replace('\'', "''"),
        location_city = body.location_city.replace('\'', "''"),
        consent_val = consent_val,
    );

    worker::D1Database::prepare(d1, &sql)
        .run()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to update profile: {e:?}")))?;

    // Return the updated profile
    Ok(ApiOk::new(MyProfileResponse {
        email: claims.email,
        display_name: body.display_name,
        wallet_address: None, // wallet is set separately via claim flow
        github_handle: Some(body.github_handle).filter(|s| !s.is_empty()),
        discord_handle: Some(body.discord_handle).filter(|s| !s.is_empty()),
        twitter_handle: Some(body.twitter_handle).filter(|s| !s.is_empty()),
        experience_level: None, // not editable in MVP
        primary_role: Some(body.primary_role).filter(|s| !s.is_empty()),
        tech_stack: body.tech_stack,
        interests: body.interests,
        learning_goals: body.learning_goals,
        company_org: body.company_org,
        location_city: body.location_city,
        consent_outreach: body.consent_outreach,
        total_events: 0, // not meaningful here
    }))
}
