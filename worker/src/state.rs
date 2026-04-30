use std::sync::Arc;

use worker::{Env, KvStore};

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
    /// KV namespace for quiz questions and progress (Issue 002).
    /// `None` if the `QUIZ` binding is not configured in `wrangler.toml`.
    pub quiz_kv: Option<KvStore>,
    /// KV namespace for event registry and per-event config (Issue 004).
    /// `None` if the `EVENTS` binding is not configured in `wrangler.toml`.
    pub events_kv: Option<KvStore>,
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

        let super_admin_emails_str = get_secret(env, "SUPER_ADMIN_EMAILS")
            .or_else(|_| get_var(env, "SUPER_ADMIN_EMAILS"))
            .unwrap_or_else(|_| String::new());
        let super_admin_emails: Vec<String> = super_admin_emails_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let server_url = get_var(env, "SERVER_URL")
            .unwrap_or_else(|_| "https://event-checkin.workers.dev".to_string());

        let claim_base_url =
            get_var(env, "CLAIM_BASE_URL").unwrap_or_else(|_| format!("{server_url}/claim"));

        // Phase 2: Helius / NFT config (secrets)
        let helius_rpc_url = get_secret(env, "HELIUS_RPC_URL")
            .unwrap_or_else(|_| "https://devnet.helius-rpc.com".to_string());
        let helius_api_key = get_secret(env, "HELIUS_API_KEY").unwrap_or_else(|_| String::new());
        // Collection is optional — Helius mints to its own tree without one
        let nft_collection_mint =
            get_secret(env, "NFT_COLLECTION_MINT").unwrap_or_else(|_| String::new());
        let nft_metadata_uri =
            get_secret(env, "NFT_METADATA_URI").unwrap_or_else(|_| String::new());
        let nft_image_url = get_secret(env, "NFT_IMAGE_URL").unwrap_or_else(|_| String::new());

        // Phase 3: Event config (vars — change per event without rebuild)
        let event_name = get_var(env, "EVENT_NAME").unwrap_or_else(|_| {
            "Solana x AI Builders: The Road to Mainnet #1 (Bangkok)".to_string()
        });
        let event_tagline = get_var(env, "EVENT_TAGLINE").unwrap_or_else(|_| {
            "Deep Dive into Rust, AI Agents, and the Solana Ecosystem".to_string()
        });
        let event_link = get_var(env, "EVENT_LINK").unwrap_or_else(|_| {
            "https://solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/"
                .to_string()
        });
        let event_start_ms = get_var(env, "EVENT_START_MS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1_777_170_600_000);
        let event_end_ms = get_var(env, "EVENT_END_MS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1_777_183_200_000);

        let config = AppConfig {
            google_oauth,
            service_account,
            sheets,
            jwt_secret: get_secret(env, "JWT_SECRET")?,
            staff_emails,
            server_url,
            claim_base_url,
            helius_rpc_url,
            helius_api_key,
            nft_collection_mint,
            nft_metadata_uri,
            nft_image_url,
            event_name,
            event_tagline,
            event_link,
            event_start_ms,
            event_end_ms,
            super_admin_emails,
            // host/port unused on Workers — placeholder values
            host: "0.0.0.0".to_string(),
            port: 0,
        };

        // Quiz KV namespace — optional, quiz feature disabled if not bound
        let quiz_kv = env.kv("QUIZ").ok();

        // Events KV namespace — optional, multi-event disabled if not bound
        let events_kv = env.kv("EVENTS").ok();

        Ok(Self {
            config: Arc::new(config),
            quiz_kv,
            events_kv,
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
