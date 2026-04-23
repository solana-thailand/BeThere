use crate::config::AppState;
use crate::models::auth::{GoogleUserInfo, TokenRequest, TokenResponse};

/// Build the Google OAuth 2.0 authorization URL.
/// This URL redirects the user to Google's consent screen.
pub fn get_auth_url(state: &AppState) -> String {
    let config = &state.config.google_oauth;
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .finish();

    format!("https://accounts.google.com/o/oauth2/v2/auth?{params}")
}

/// Handle the OAuth callback: exchange the authorization code for tokens.
/// Returns the Google user info on success.
pub async fn handle_callback(code: &str, state: &AppState) -> Result<GoogleUserInfo, String> {
    let config = &state.config.google_oauth;

    // Exchange code for tokens
    let token_request = TokenRequest::new(
        code.to_string(),
        config.client_id.clone(),
        config.client_secret.clone(),
        config.redirect_uri.clone(),
    );

    let response = state
        .http_client
        .post("https://oauth2.googleapis.com/token")
        .form(&token_request)
        .send()
        .await
        .map_err(|e| format!("failed to exchange code for token: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("token exchange failed ({status}): {body}"));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("failed to parse token response: {e}"))?;

    // Fetch user info using the access token
    let user_info = fetch_user_info(&token_response.access_token, state)
        .await
        .map_err(|e| format!("failed to fetch user info: {e}"))?;

    Ok(user_info)
}

/// Fetch the authenticated user's profile from Google's userinfo endpoint.
pub async fn fetch_user_info(
    access_token: &str,
    state: &AppState,
) -> Result<GoogleUserInfo, String> {
    let response = state
        .http_client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| format!("failed to call userinfo endpoint: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("userinfo request failed ({status}): {body}"));
    }

    response
        .json()
        .await
        .map_err(|e| format!("failed to parse user info: {e}"))
}

/// Check if a given email is in the staff emails allowlist.
pub fn is_staff(email: &str, state: &AppState) -> bool {
    state
        .config
        .staff_emails
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(email))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, GoogleOAuthConfig, GoogleServiceAccountConfig, SheetsConfig};
    use reqwest::Client;

    fn test_state() -> AppState {
        let config = AppConfig {
            google_oauth: GoogleOAuthConfig {
                client_id: "test-client-id".to_string(),
                client_secret: "test-secret".to_string(),
                redirect_uri: "http://localhost:3000/api/auth/callback".to_string(),
            },
            service_account: GoogleServiceAccountConfig {
                client_email: "test@test.iam.gserviceaccount.com".to_string(),
                private_key: "test-key".to_string(),
                token_uri: "https://oauth2.googleapis.com/token".to_string(),
            },
            sheets: SheetsConfig {
                sheet_id: "test-sheet-id".to_string(),
                sheet_name: "Sheet1".to_string(),
                staff_sheet_name: "staff".to_string(),
            },
            jwt_secret: "test-jwt-secret".to_string(),
            staff_emails: vec![
                "admin@example.com".to_string(),
                "staff@example.com".to_string(),
            ],
            server_url: "http://localhost:3000".to_string(),
            host: "0.0.0.0".to_string(),
            port: 3000,
        };

        AppState {
            config: std::sync::Arc::new(config),
            http_client: Client::new(),
        }
    }

    #[test]
    fn test_get_auth_url_contains_required_params() {
        let state = test_state();
        let url = get_auth_url(&state);

        assert!(url.contains("accounts.google.com/o/oauth2/v2/auth"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A3000"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid+email+profile"));
    }

    #[test]
    fn test_is_staff_allowed() {
        let state = test_state();
        assert!(is_staff("admin@example.com", &state));
        assert!(is_staff("staff@example.com", &state));
        assert!(is_staff("Admin@Example.COM", &state)); // case insensitive
    }

    #[test]
    fn test_is_staff_not_allowed() {
        let state = test_state();
        assert!(!is_staff("random@example.com", &state));
        assert!(!is_staff("unknown@gmail.com", &state));
    }
}
