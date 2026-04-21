use std::env;
use std::sync::Arc;

use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleServiceAccountConfig {
    pub client_email: String,
    pub private_key: String,
    pub token_uri: String,
}

#[derive(Debug, Clone)]
pub struct SheetsConfig {
    pub sheet_id: String,
    pub sheet_name: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub google_oauth: GoogleOAuthConfig,
    pub service_account: GoogleServiceAccountConfig,
    pub sheets: SheetsConfig,
    pub jwt_secret: String,
    pub staff_emails: Vec<String>,
    pub server_url: String,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let google_oauth = GoogleOAuthConfig {
            client_id: env_required("GOOGLE_CLIENT_ID")?,
            client_secret: env_required("GOOGLE_CLIENT_SECRET")?,
            redirect_uri: env_required("GOOGLE_REDIRECT_URI")?,
        };

        let service_account = GoogleServiceAccountConfig {
            client_email: env_required("GOOGLE_SERVICE_ACCOUNT_EMAIL")?,
            private_key: env_required("GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY")?,
            token_uri: env_required("GOOGLE_SERVICE_ACCOUNT_TOKEN_URI")?,
        };

        let sheets = SheetsConfig {
            sheet_id: env_required("GOOGLE_SHEET_ID")?,
            sheet_name: env::var("GOOGLE_SHEET_NAME").unwrap_or_else(|_| "Sheet1".to_string()),
        };

        let staff_emails_str = env_required("STAFF_EMAILS")?;
        let staff_emails: Vec<String> = staff_emails_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let config = Self {
            google_oauth,
            service_account,
            sheets,
            jwt_secret: env_required("JWT_SECRET")?,
            staff_emails,
            server_url: env_required("SERVER_URL")?,
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
        };

        Ok(config)
    }

    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub http_client: Client,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        Self {
            config: Arc::new(config),
            http_client,
        }
    }
}

fn env_required(key: &str) -> Result<String, Box<dyn std::error::Error>> {
    match env::var(key) {
        Ok(val) if !val.is_empty() => Ok(val),
        Ok(_) => Err(format!("env var {key} is empty").into()),
        Err(_) => Err(format!("env var {key} is required but not set").into()),
    }
}
