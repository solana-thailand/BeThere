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
    pub staff_sheet_name: String,
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
    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
