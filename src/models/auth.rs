use chrono::Utc;
use serde::{Deserialize, Serialize};

/// JWT claims used for staff session tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Google email of the staff member
    pub email: String,
    /// Subject (Google user ID)
    pub sub: String,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Expiration (Unix timestamp)
    pub exp: u64,
}

impl Claims {
    /// Create new claims with a 24-hour expiry.
    pub fn new(email: String, sub: String) -> Self {
        let now = Utc::now();
        let iat = now.timestamp() as u64;
        let exp = (now + chrono::Duration::hours(24)).timestamp() as u64;

        Self {
            email,
            sub,
            iat,
            exp,
        }
    }

    /// Check if the token has expired.
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp() as u64;
        self.exp < now
    }
}

/// Google OAuth 2.0 token exchange response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub token_type: String,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// Google user info from the userinfo endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub verified_email: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

/// Google OAuth 2.0 token request body.
#[derive(Debug, Serialize)]
pub struct TokenRequest {
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub grant_type: String,
}

impl TokenRequest {
    pub fn new(
        code: String,
        client_id: String,
        client_secret: String,
        redirect_uri: String,
    ) -> Self {
        Self {
            code,
            client_id,
            client_secret,
            redirect_uri,
            grant_type: "authorization_code".to_string(),
        }
    }
}

/// Service account JWT assertion payload for Google API access.
#[derive(Debug, Serialize)]
pub struct ServiceAccountClaim {
    pub iss: String,
    pub scope: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
}

impl ServiceAccountClaim {
    pub fn new(client_email: String, token_uri: String) -> Self {
        let now = Utc::now().timestamp() as u64;

        Self {
            iss: client_email,
            scope: "https://www.googleapis.com/auth/spreadsheets".to_string(),
            aud: token_uri,
            iat: now,
            exp: now + 3600, // 1 hour
        }
    }
}
