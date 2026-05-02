use std::fmt;

use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl fmt::Debug for GoogleOAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleOAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"***REDACTED***")
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

#[derive(Clone, Deserialize)]
pub struct GoogleServiceAccountConfig {
    pub client_email: String,
    pub private_key: String,
    pub token_uri: String,
}

impl fmt::Debug for GoogleServiceAccountConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleServiceAccountConfig")
            .field("client_email", &self.client_email)
            .field("private_key", &"***REDACTED***")
            .field("token_uri", &self.token_uri)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct SheetsConfig {
    pub sheet_id: String,
    pub sheet_name: String,
    pub staff_sheet_name: String,
}

#[derive(Clone)]
pub struct AppConfig {
    pub google_oauth: GoogleOAuthConfig,
    pub service_account: GoogleServiceAccountConfig,
    pub sheets: SheetsConfig,
    pub jwt_secret: String,
    pub staff_emails: Vec<String>,
    pub server_url: String,
    /// Base URL for claim links (e.g. https://bethere.solana-thailand.workers.dev/claim)
    pub claim_base_url: String,
    /// Helius RPC URL for Solana JSON-RPC calls (e.g. https://devnet.helius-rpc.com)
    pub helius_rpc_url: String,
    /// Helius API key for RPC authentication
    pub helius_api_key: String,
    /// NFT collection mint address for compressed NFTs
    pub nft_collection_mint: String,
    /// URI to metadata JSON on Arweave/IPFS for the NFT
    pub nft_metadata_uri: String,
    /// NFT badge image URL
    pub nft_image_url: String,
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
    /// Global admin emails that can create/manage all events.
    /// Set via SUPER_ADMIN_EMAILS env var (comma-separated).
    pub super_admin_emails: Vec<String>,
    pub host: String,
    pub port: u16,
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("google_oauth", &self.google_oauth)
            .field("service_account", &self.service_account)
            .field("sheets", &self.sheets)
            .field("jwt_secret", &"***REDACTED***")
            .field("staff_emails", &self.staff_emails)
            .field("server_url", &self.server_url)
            .field("claim_base_url", &self.claim_base_url)
            .field("helius_rpc_url", &self.helius_rpc_url)
            .field("helius_api_key", &"***REDACTED***")
            .field("nft_collection_mint", &self.nft_collection_mint)
            .field("nft_metadata_uri", &self.nft_metadata_uri)
            .field("nft_image_url", &self.nft_image_url)
            .field("event_name", &self.event_name)
            .field("event_tagline", &self.event_tagline)
            .field("event_link", &self.event_link)
            .field("event_start_ms", &self.event_start_ms)
            .field("event_end_ms", &self.event_end_ms)
            .field("super_admin_emails", &self.super_admin_emails)
            .field("host", &self.host)
            .field("port", &self.port)
            .finish()
    }
}

impl AppConfig {
    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
