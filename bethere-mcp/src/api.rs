//! Thin HTTP client over the BeThere worker's existing API. Nothing here is a
//! new endpoint: every call is one the web app already makes.

use domain::models::auth::{
    WalletNonceRequest, WalletNonceResponse, WalletVerifyRequest, WalletVerifyResponse,
};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use solana_sdk::signer::{keypair::Keypair, Signer};
use tokio::sync::Mutex;
use url::Url;

use crate::error::ToolError;

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

pub struct BeThereApi {
    http: reqwest::Client,
    base: Url,
    /// SIWS session JWT, minted on first use and renewed once on a 401.
    token: Mutex<Option<String>>,
}

impl BeThereApi {
    pub fn new(base: Url) -> Result<Self, ToolError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent(concat!("bethere-mcp/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            base,
            token: Mutex::new(None),
        })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    /// Public GET; returns the envelope's `data`.
    pub async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, ToolError> {
        self.send(Method::GET, path, query, None, None).await
    }

    /// Public POST with a JSON body; returns the envelope's `data`.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, ToolError> {
        self.send(Method::POST, path, &[], Some(body), None).await
    }

    /// Call an attendee-authed route as `wallet`. Signs in with SIWS when no
    /// session exists and retries once if the session has expired.
    pub async fn authed(
        &self,
        wallet: &Keypair,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<Value, ToolError> {
        let token = self.session(wallet).await?;
        match self
            .send(method.clone(), path, query, body, Some(&token))
            .await
        {
            Err(ToolError::Api { status: 401, .. }) => {
                *self.token.lock().await = None;
                let token = self.session(wallet).await?;
                self.send(method, path, query, body, Some(&token)).await
            }
            other => other,
        }
    }

    pub fn decode<T: DeserializeOwned>(value: Value) -> Result<T, ToolError> {
        serde_json::from_value(value)
            .map_err(|e| ToolError::Upstream(format!("unexpected response shape: {e}")))
    }

    async fn session(&self, wallet: &Keypair) -> Result<String, ToolError> {
        let mut guard = self.token.lock().await;
        if let Some(token) = guard.as_ref() {
            return Ok(token.clone());
        }
        let token = self.sign_in(wallet).await?;
        *guard = Some(token.clone());
        Ok(token)
    }

    /// Sign-In With Solana: the server issues the challenge, the agent's key
    /// signs exactly that text, the server verifies it against its own copy.
    async fn sign_in(&self, wallet: &Keypair) -> Result<String, ToolError> {
        let wallet_address = wallet.pubkey().to_string();
        let nonce: WalletNonceResponse = Self::decode(
            self.post(
                "/api/auth/wallet/nonce",
                &serde_json::to_value(WalletNonceRequest {
                    wallet_address: wallet_address.clone(),
                })
                .map_err(|e| ToolError::Upstream(e.to_string()))?,
            )
            .await?,
        )?;
        let signature = wallet.sign_message(nonce.message.as_bytes()).to_string();
        let verified: WalletVerifyResponse = Self::decode(
            self.post(
                "/api/auth/wallet/verify",
                &serde_json::to_value(WalletVerifyRequest {
                    wallet_address,
                    signature,
                    message: nonce.message,
                    nonce: nonce.nonce,
                })
                .map_err(|e| ToolError::Upstream(e.to_string()))?,
            )
            .await?,
        )?;
        match (verified.authenticated, verified.token.is_empty()) {
            (true, false) => Ok(verified.token),
            _ => Err(ToolError::Upstream(
                "wallet sign-in returned no session".to_string(),
            )),
        }
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
        bearer: Option<&str>,
    ) -> Result<Value, ToolError> {
        let mut url = self
            .base
            .join(path)
            .map_err(|e| ToolError::InvalidArgs(format!("path '{path}': {e}")))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let mut req = self
            .http
            .request(method, url)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            req = req.json(body);
        }
        if let Some(token) = bearer {
            req = req.bearer_auth(token);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        match status {
            s if s.is_success() => unwrap_envelope(&text),
            StatusCode::TOO_MANY_REQUESTS => Err(ToolError::Api {
                status: status.as_u16(),
                body: "rate limited; wait and retry".to_string(),
            }),
            _ => Err(ToolError::Api {
                status: status.as_u16(),
                body: truncate(&text, 600),
            }),
        }
    }
}

/// The worker wraps every body as `{"success":true,"data":…}`. Return `data`,
/// or the whole body for the few routes that are not enveloped.
pub fn unwrap_envelope(text: &str) -> Result<Value, ToolError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| ToolError::Upstream(format!("non-JSON response: {e}")))?;
    match value {
        Value::Object(mut map) if map.contains_key("data") => {
            Ok(map.remove("data").unwrap_or(Value::Null))
        }
        other => Ok(other),
    }
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((idx, _)) => format!("{}…", &text[..idx]),
        None => text.to_string(),
    }
}
