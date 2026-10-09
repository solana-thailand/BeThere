//! `/api/sandbox/*` (.plans/042 0.4): public, no session. The page shows the
//! server's error text as is, so every call returns it as the `Err`.

use serde::{Deserialize, Serialize};

use super::api_base;
use super::fetch::{get_no_cache, post, response_json, response_text};
use super::types::ApiResponse;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SandboxConfig {
    pub enabled: bool,
    #[serde(default)]
    pub organizer: String,
    #[serde(default)]
    pub faucet: String,
    pub deposit_amount: u64,
    pub event_seconds: i64,
    pub browser_rpc: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SandboxEvent {
    pub event_id: String,
    pub event_end: i64,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SandboxSignature {
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SandboxTx {
    pub transaction_b64: String,
}

#[derive(Serialize)]
struct Wallet<'a> {
    wallet: &'a str,
}

#[derive(Serialize)]
struct EventWallet<'a> {
    event_id: &'a str,
    wallet: &'a str,
}

/// POST `path` with `body`, unwrap `{success, data, error}`.
async fn post_json<T: serde::de::DeserializeOwned + Default>(
    path: &str,
    body: Option<String>,
) -> Result<T, String> {
    let url = format!("{}/sandbox{path}", api_base());
    let response = post(&url, &[("Content-Type", "application/json")], body)
        .await
        .map_err(|e| e.message)?;
    let text = response_text(&response).await.map_err(|e| e.message)?;
    let parsed: ApiResponse<T> = serde_json::from_str(&text)
        .map_err(|_| format!("HTTP {}: unexpected reply", response.status()))?;
    match (parsed.success, parsed.data) {
        (true, Some(data)) => Ok(data),
        _ => Err(parsed
            .error
            .unwrap_or_else(|| format!("HTTP {}", response.status()))),
    }
}

fn body(value: &impl Serialize) -> Option<String> {
    serde_json::to_string(value).ok()
}

/// GET /api/sandbox/config: not wrapped, always 200.
pub async fn sandbox_config() -> Result<SandboxConfig, String> {
    let url = format!("{}/sandbox/config", api_base());
    let response = get_no_cache(&url, &[]).await.map_err(|e| e.message)?;
    response_json(&response).await.map_err(|e| e.message)
}

pub async fn sandbox_create_event() -> Result<SandboxEvent, String> {
    post_json("/events", None).await
}

pub async fn sandbox_faucet(wallet: &str) -> Result<SandboxSignature, String> {
    post_json("/faucet", body(&Wallet { wallet })).await
}

pub async fn sandbox_deposit_tx(event_id: &str, wallet: &str) -> Result<SandboxTx, String> {
    post_json("/deposit-tx", body(&EventWallet { event_id, wallet })).await
}

pub async fn sandbox_check_in(event_id: &str, wallet: &str) -> Result<SandboxSignature, String> {
    post_json("/check-in", body(&EventWallet { event_id, wallet })).await
}

pub async fn sandbox_refund_tx(event_id: &str, wallet: &str) -> Result<SandboxTx, String> {
    post_json("/refund-tx", body(&EventWallet { event_id, wallet })).await
}

pub async fn sandbox_return_tx(wallet: &str) -> Result<SandboxTx, String> {
    post_json("/return-tx", body(&Wallet { wallet })).await
}
