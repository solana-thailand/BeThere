use std::sync::Arc;

use worker::Env;

use event_checkin_domain::config::{
    AppConfig, GoogleOAuthConfig, GoogleServiceAccountConfig, SheetsConfig,
};

/// Application state available to all Axum handlers on Workers.
///
/// Holds the parsed config. The `Env` is not stored here — the SPA fallback
/// uses `include_str!` for `index.html` instead of the ASSETS binding.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
}

impl AppState {
    /// Build `AppState` from Workers environment bindings.
    ///
    /// Secrets are set via `npx wrangler secret put <NAME>`.
    /// Vars are defined in `wrangler.toml` `[vars]`.
    pub fn from_env(env: &Env) -> Result<Self, String> {
        let google_oauth = GoogleOAuthConfig {
            client_id: get_secret(env, "GOOGLE_CLIENT_ID")?,
            client_secret: get_secret(env, "GOOGLE_CLIENT_SECRET")?,
            redirect_uri: get_secret(env, "GOOGLE_REDIRECT_URI")?,
        };

        let service_account = GoogleServiceAccountConfig {
            client_email: get_secret(env, "GOOGLE_SERVICE_ACCOUNT_EMAIL")?,
            private_key: get_secret(env, "GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY")?,
            token_uri: get_secret(env, "GOOGLE_SERVICE_ACCOUNT_TOKEN_URI")?,
        };

        let sheets = SheetsConfig {
            sheet_id: get_secret(env, "GOOGLE_SHEET_ID")?,
            sheet_name: get_var(env, "GOOGLE_SHEET_NAME").unwrap_or_else(|_| "Sheet1".to_string()),
            staff_sheet_name: get_var(env, "GOOGLE_STAFF_SHEET_NAME")
                .unwrap_or_else(|_| "staff".to_string()),
        };

        let staff_emails_str = get_secret(env, "STAFF_EMAILS")?;
        let staff_emails: Vec<String> = staff_emails_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let server_url = get_var(env, "SERVER_URL")
            .unwrap_or_else(|_| "https://event-checkin.workers.dev".to_string());

        let config = AppConfig {
            google_oauth,
            service_account,
            sheets,
            jwt_secret: get_secret(env, "JWT_SECRET")?,
            staff_emails,
            server_url,
            // host/port unused on Workers — placeholder values
            host: "0.0.0.0".to_string(),
            port: 0,
        };

        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// Check if a given email is in the staff emails allowlist.
    pub fn is_staff(&self, email: &str) -> bool {
        self.config
            .staff_emails
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(email))
    }
}

/// Read a secret from Workers environment (set via `wrangler secret put`).
fn get_secret(env: &Env, key: &str) -> Result<String, String> {
    env.secret(key)
        .map(|s| s.to_string())
        .map_err(|e| format!("secret '{key}' not configured: {e}"))
}

/// Read a variable from Workers environment (set in `wrangler.toml` [vars]).
fn get_var(env: &Env, key: &str) -> Result<String, String> {
    env.var(key)
        .map(|v| v.to_string())
        .map_err(|e| format!("var '{key}' not configured: {e}"))
}
