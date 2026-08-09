use std::collections::HashSet;
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
    /// Google Sheet ID for the master contacts list (cross-event deduplicated emails).
    /// If empty, contacts upsert is skipped silently.
    pub contacts_sheet_id: String,
    /// Tab name for the contacts sheet. Defaults to "Contacts".
    pub contacts_sheet_name: String,
    /// Tab name for the events registry in the contacts sheet. Defaults to "Events".
    pub events_sheet_name: String,
    /// Google Sheet ID for platform-level data (users, waitlist, audit log).
    /// If empty, user logging and waitlist fall back to `sheet_id`.
    pub platform_sheet_id: String,
}

/// Solana/Helius RPC configuration.
#[derive(Clone)]
pub struct SolanaConfig {
    /// Helius RPC URL for Solana JSON-RPC calls.
    pub rpc_url: String,
    /// Helius API key for RPC authentication.
    pub api_key: String,
}

impl SolanaConfig {
    /// Pre-computed RPC URL with API key appended.
    /// Handles both `?key=val` and `?api-key=KEY` URL patterns.
    pub fn full_rpc_url(&self) -> String {
        if self.api_key.is_empty() {
            return self.rpc_url.clone();
        }
        format!(
            "{}{}{}",
            self.rpc_url,
            if self.rpc_url.contains('?') {
                "&"
            } else {
                "?api-key="
            },
            self.api_key
        )
    }
}

impl fmt::Debug for SolanaConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SolanaConfig")
            .field("rpc_url", &self.rpc_url)
            .field("api_key", &"***REDACTED***")
            .finish()
    }
}

/// NFT configuration (global defaults, per-event overrides via EventConfig).
#[derive(Clone)]
pub struct NftConfig {
    /// Collection mint address for compressed NFTs.
    pub collection_mint: String,
    /// URI to metadata JSON on Arweave/IPFS.
    pub metadata_uri: String,
    /// NFT badge image URL.
    pub image_url: String,
}

impl fmt::Debug for NftConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NftConfig")
            .field("collection_mint", &self.collection_mint)
            .field("metadata_uri", &self.metadata_uri)
            .field("image_url", &self.image_url)
            .finish()
    }
}

/// Server configuration.
#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub url: String,
    /// Base URL for claim links.
    pub claim_base_url: String,
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("url", &self.url)
            .field("claim_base_url", &self.claim_base_url)
            .finish()
    }
}

impl ServerConfig {
    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Legacy event defaults read from env vars.
///
/// Used by `seed_from_config` and `from_global_config` to create the initial
/// default event when the EVENTS KV namespace is not yet populated.
#[derive(Clone)]
pub struct EventDefaults {
    pub name: String,
    pub tagline: String,
    pub link: String,
    pub start_ms: i64,
    pub end_ms: i64,
    /// Whether deposits are enabled for the seeded event.
    pub deposit_enabled: bool,
    /// Deposit amount in USDC smallest unit (6 decimals).
    pub deposit_amount_usdc: u64,
    /// Deposit amount in Thai Baht (for PromptPay track).
    pub deposit_amount_thb: u64,
    /// PromptPay ID for THB payments.
    pub promptpay_id: String,
}

impl fmt::Debug for EventDefaults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventDefaults")
            .field("name", &self.name)
            .field("tagline", &self.tagline)
            .field("link", &self.link)
            .field("start_ms", &self.start_ms)
            .field("end_ms", &self.end_ms)
            .field("deposit_enabled", &self.deposit_enabled)
            .field("deposit_amount_usdc", &self.deposit_amount_usdc)
            .field("deposit_amount_thb", &self.deposit_amount_thb)
            .field("promptpay_id", &self.promptpay_id)
            .finish()
    }
}

#[derive(Clone)]
pub struct AppConfig {
    pub google_oauth: GoogleOAuthConfig,
    pub service_account: GoogleServiceAccountConfig,
    pub sheets: SheetsConfig,
    pub jwt_secret: String,
    pub staff_emails: HashSet<String>,
    pub super_admin_emails: HashSet<String>,
    pub server: ServerConfig,
    pub solana: SolanaConfig,
    pub nft: NftConfig,
    pub event_defaults: EventDefaults,
    /// Dev mode bypasses JWT verification — accepts "dev-token" as valid.
    /// Enable via `DEV_MODE=1` in `.dev.vars` or `wrangler.toml`.
    /// **Never enable in production.**
    pub dev_mode: bool,
    /// Email to use as the authenticated user in dev mode.
    /// Defaults to the first super_admin_email or "dev@localhost".
    pub dev_email: String,
    // ---- Social OAuth linking ----
    /// GitHub OAuth app client ID for social account linking.
    pub github_client_id: String,
    /// GitHub OAuth app client secret for social account linking.
    pub github_client_secret: String,
    /// GitHub OAuth callback URL (should point to /api/auth/github/callback).
    pub github_redirect_uri: String,
    /// Telegram bot token for verifying Login Widget HMAC signatures.
    pub telegram_bot_token: String,
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("google_oauth", &self.google_oauth)
            .field("service_account", &self.service_account)
            .field("sheets", &self.sheets)
            .field("jwt_secret", &"***REDACTED***")
            .field("staff_emails", &self.staff_emails)
            .field("super_admin_emails", &self.super_admin_emails)
            .field("server", &self.server)
            .field("solana", &self.solana)
            .field("nft", &self.nft)
            .field("event_defaults", &self.event_defaults)
            .field("dev_mode", &self.dev_mode)
            .field("dev_email", &self.dev_email)
            .field("github_client_id", &self.github_client_id)
            .field("github_client_secret", &"***REDACTED***")
            .field("telegram_bot_token", &"***REDACTED***")
            .finish()
    }
}
