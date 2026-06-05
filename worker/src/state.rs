use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use worker::{Bucket, Context, D1Database, Env, KvStore, ObjectNamespace};

// ---------------------------------------------------------------------------
// Cached bindings — stable per Workers isolate, avoid JS interop on every request
// ---------------------------------------------------------------------------

/// Cached stable bindings (KV + D1 + DO) — read once per isolate, not per request.
///
/// `env.kv()`, `env.d1()`, and `env.durable_object()` cross the WASM↔JS boundary.
/// Since bindings don't change within an isolate's lifetime, cache them alongside config.
struct CachedBindings {
    quiz_kv: Option<KvStore>,
    events_kv: Option<KvStore>,
    d1: Option<Arc<D1Database>>,
    r2: Option<Bucket>,
    event_do: Option<ObjectNamespace>,
}

static CACHED_BINDINGS: OnceLock<CachedBindings> = OnceLock::new();

use event_checkin_domain::config::{
    AppConfig, EventDefaults, GoogleOAuthConfig, GoogleServiceAccountConfig, NftConfig,
    ServerConfig, SheetsConfig, SolanaConfig,
};

/// Global cache for the parsed `AppConfig`.
///
/// Built once per Workers isolate, reused across all subsequent fetch requests.
/// Only the KV bindings and `webhook_secret` are re-read per request (cheap
/// single calls to `env.kv()` / `env.var()`).
///
/// Safe because `AppConfig` is pure Rust data — no JS interop objects — so it
/// is `Send + Sync`.
static CACHED_CONFIG: OnceLock<Arc<AppConfig>> = OnceLock::new();

/// Whether the `WEBHOOK_SECRET` empty-warning has already been emitted.
/// Prevents log spam on every request when the var is not set.
static WARNED_WEBHOOK_SECRET: OnceLock<()> = OnceLock::new();

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
    /// D1 database for claim locks and audit trail (Issue 037).
    /// `None` if the `DB` binding is not configured in `wrangler.toml`.
    /// Wrapped in `Arc` because `D1Database` is not `Clone`.
    pub d1: Option<Arc<D1Database>>,
    /// Shared secret for validating webhook `Authorization: Bearer <token>` header.
    /// If empty, webhook auth validation is skipped (backward compatible).
    pub webhook_secret: String,
    /// Workers fetch-event context — used for `wait_until()` to detach background
    /// tasks (e.g. Google Sheets sync) so the HTTP response returns immediately.
    /// `None` outside the fetch lifecycle (tests, scheduled events).
    pub worker_ctx: Option<Arc<Context>>,
    /// R2 bucket for NFT metadata, badge SVGs, slip images, refund proofs (Issue #039).
    /// `None` if the `ASSETS_BUCKET` binding is not configured.
    #[allow(dead_code)]
    pub r2: Option<Bucket>,
    /// Durable Object namespace for ACID event writes (Issue #050).
    /// `None` if the `EVENT_DO` binding is not configured.
    pub event_do: Option<ObjectNamespace>,
}

impl AppState {
    /// Build `AppConfig` from Workers environment (called once, cached globally).
    ///
    /// Reads 22+ env vars, creates 2 `HashSet`s, and performs string
    /// allocation. Expensive enough to justify caching.
    fn build_config(env: &Env) -> Result<AppConfig, String> {
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
            sheet_name: get_var(env, "GOOGLE_SHEET_NAME")
                .unwrap_or_else(|_| "Attendees".to_string()),
            staff_sheet_name: get_var(env, "GOOGLE_STAFF_SHEET_NAME")
                .unwrap_or_else(|_| "staff".to_string()),
            contacts_sheet_id: get_var(env, "CONTACTS_SHEET_ID").unwrap_or_default(),
            contacts_sheet_name: get_var(env, "CONTACTS_SHEET_NAME")
                .unwrap_or_else(|_| "Contacts".to_string()),
            events_sheet_name: get_var(env, "EVENTS_SHEET_NAME")
                .unwrap_or_else(|_| "Events".to_string()),
            platform_sheet_id: get_var(env, "PLATFORM_SHEET_ID").unwrap_or_default(),
        };

        let staff_emails_str = get_secret(env, "STAFF_EMAILS")?;
        let staff_emails: HashSet<String> = staff_emails_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let super_admin_emails_str = get_secret(env, "SUPER_ADMIN_EMAILS")
            .or_else(|_| get_var(env, "SUPER_ADMIN_EMAILS"))
            .unwrap_or_else(|_| String::new());
        let super_admin_emails: HashSet<String> = super_admin_emails_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let server_url = get_var(env, "SERVER_URL")
            .unwrap_or_else(|_| "https://event-checkin.workers.dev".to_string());

        let claim_base_url =
            get_var(env, "CLAIM_BASE_URL").unwrap_or_else(|_| format!("{server_url}/claim"));

        let server = ServerConfig {
            // host/port unused on Workers — placeholder values
            host: "0.0.0.0".to_string(),
            port: 0,
            url: server_url,
            claim_base_url,
        };

        let solana = SolanaConfig {
            rpc_url: get_secret(env, "HELIUS_RPC_URL")
                .unwrap_or_else(|_| "https://devnet.helius-rpc.com".to_string()),
            api_key: get_secret(env, "HELIUS_API_KEY").unwrap_or_else(|_| String::new()),
        };

        let nft = NftConfig {
            collection_mint: get_secret(env, "NFT_COLLECTION_MINT")
                .unwrap_or_else(|_| String::new()),
            metadata_uri: get_secret(env, "NFT_METADATA_URI").unwrap_or_else(|_| String::new()),
            image_url: get_secret(env, "NFT_IMAGE_URL").unwrap_or_else(|_| String::new()),
        };

        let event_defaults = EventDefaults {
            name: get_var(env, "EVENT_NAME").unwrap_or_else(|_| {
                "Solana x AI Builders: The Road to Mainnet #1 (Bangkok)".to_string()
            }),
            tagline: get_var(env, "EVENT_TAGLINE").unwrap_or_else(|_| {
                "Deep Dive into Rust, AI Agents, and the Solana Ecosystem".to_string()
            }),
            link: get_var(env, "EVENT_LINK").unwrap_or_else(|_| {
                "https://solana-thailand.github.io/genesis/events/road-to-mainnet-1-bangkok/"
                    .to_string()
            }),
            start_ms: get_var(env, "EVENT_START_MS")
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1_777_170_600_000),
            end_ms: get_var(env, "EVENT_END_MS")
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1_777_183_200_000),
            deposit_enabled: get_var(env, "EVENT_DEPOSIT_ENABLED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            deposit_amount_usdc: get_var(env, "EVENT_DEPOSIT_AMOUNT_USDC")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
            deposit_amount_thb: get_var(env, "EVENT_DEPOSIT_AMOUNT_THB")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
            promptpay_id: get_var(env, "EVENT_PROMPTPAY_ID").unwrap_or_default(),
        };

        let dev_mode = get_var(env, "DEV_MODE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let dev_email = get_var(env, "DEV_EMAIL").unwrap_or_else(|_| {
            super_admin_emails
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| "dev@localhost".to_string())
        });

        if dev_mode {
            // VULN-006: Refuse DEV_MODE on production domains
            let is_production = google_oauth.redirect_uri.contains("bethere")
                || google_oauth.redirect_uri.contains("workers.dev");
            if is_production {
                return Err(
                    "SECURITY: DEV_MODE=1 is set but redirect_uri looks like a production domain. \
                     Remove DEV_MODE before deploying to production."
                        .to_string(),
                );
            }
            tracing::warn!(
                email = %dev_email,
                "⚠️  DEV_MODE enabled — JWT verification bypassed, accepting \"dev-token\" as valid"
            );
        }

        Ok(AppConfig {
            google_oauth,
            service_account,
            sheets,
            jwt_secret: get_secret(env, "JWT_SECRET")?,
            staff_emails,
            super_admin_emails,
            server,
            solana,
            nft,
            event_defaults,
            dev_mode,
            dev_email,
        })
    }

    /// Build `AppState` from Workers environment bindings.
    ///
    /// Caches the expensive `AppConfig` parsing (22+ env var reads, 2 HashSets)
    /// behind a global `OnceLock` so it runs once per Workers isolate.
    /// Only the cheap KV bindings and `webhook_secret` are re-read per request.
    pub fn from_env(env: &Env) -> Result<Self, String> {
        // Use cached config if available, otherwise build + cache it
        let config = match CACHED_CONFIG.get() {
            Some(cached) => Arc::clone(cached),
            None => {
                let cfg = Self::build_config(env)?;
                let arc = Arc::new(cfg);
                // Best-effort: if another invocation beat us, that's fine —
                // both produce identical configs from the same env.
                let _ = CACHED_CONFIG.set(arc.clone());
                arc
            }
        };

        // Bindings are stable per isolate — cache to avoid JS interop on every request
        let bindings = match CACHED_BINDINGS.get() {
            Some(cached) => cached,
            None => {
                let b = CachedBindings {
                    quiz_kv: env.kv("QUIZ").ok(),
                    events_kv: env.kv("EVENTS").ok(),
                    d1: env.d1("DB").ok().map(Arc::new),
                    r2: env.bucket("ASSETS_BUCKET").ok(),
                    event_do: env.durable_object("EVENT_DO").ok(),
                };
                let _ = CACHED_BINDINGS.set(b);
                CACHED_BINDINGS.get().unwrap()
            }
        };

        let quiz_kv = bindings.quiz_kv.clone();
        let events_kv = bindings.events_kv.clone();
        let d1 = bindings.d1.clone();
        let r2 = bindings.r2.clone();
        let event_do = bindings.event_do.clone();

        let webhook_secret = get_var(env, "WEBHOOK_SECRET").unwrap_or_default();
        if webhook_secret.is_empty() {
            // VULN-005: Only warn in dev mode, but still allow startup.
            // Production deployments MUST set WEBHOOK_SECRET — the deposit
            // webhook handler now rejects requests when it's empty.
            let _ = WARNED_WEBHOOK_SECRET.get_or_init(|| {
                tracing::error!(
                    "WEBHOOK_SECRET not set — deposit webhook handler will reject all requests. \
                     Set WEBHOOK_SECRET in your environment."
                );
            });
        }

        Ok(Self {
            config,
            quiz_kv,
            events_kv,
            d1,
            r2,
            event_do,
            webhook_secret,
            worker_ctx: None,
        })
    }

    /// Attach the Workers fetch-event context so handlers can call `wait_until()`.
    pub fn with_ctx(mut self, ctx: Context) -> Self {
        self.worker_ctx = Some(Arc::new(ctx));
        self
    }

    /// Check if a given email is in the staff emails allowlist.
    pub fn is_staff(&self, email: &str) -> bool {
        self.config.staff_emails.contains(&email.to_lowercase())
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
