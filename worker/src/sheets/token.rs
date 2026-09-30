//! Google API access token: service-account JWT assertion, cached in KV.

use base64::Engine;

use event_checkin_domain::models::auth::ServiceAccountClaim;

use worker::KvStore;

use crate::crypto;
use crate::http::{AccessTokenResponse, exchange_jwt_assertion};
use crate::state::AppState;

/// KV key for caching the Google API access token.
const GOOGLE_TOKEN_KV_KEY: &str = "google_access_token";

/// TTL for the cached Google access token (3500s = ~58 min, 100s buffer before 3600s expiry).
const GOOGLE_TOKEN_TTL_SECS: u64 = 3500;

// ---------------------------------------------------------------------------
// Access token
// ---------------------------------------------------------------------------

/// Get a Google API access token using service account JWT assertion.
///
/// Builds an RS256-signed JWT, exchanges it for an access token via
/// the Google OAuth2 token endpoint.
pub async fn get_access_token(state: &AppState) -> Result<String, String> {
    let sa = &state.config.service_account;
    let claim = ServiceAccountClaim::new(sa.client_email.clone(), sa.token_uri.clone());

    // Build JWT header + payload (base64url-encoded)
    let header_b64 = base64_url_encode(
        &serde_json::to_vec(&serde_json::json!({"alg": "RS256", "typ": "JWT"}))
            .map_err(|e| format!("failed to encode jwt header: {e}"))?,
    );
    let payload_b64 = base64_url_encode(
        &serde_json::to_vec(&claim).map_err(|e| format!("failed to encode jwt payload: {e}"))?,
    );

    // Sign with RSA-SHA256 via SubtleCrypto
    let jwt_assertion =
        crypto::sign_jwt_assertion(&header_b64, &payload_b64, &sa.private_key).await?;

    // Exchange the signed JWT for an access token
    let token_response: AccessTokenResponse =
        exchange_jwt_assertion(&sa.token_uri, &jwt_assertion).await?;

    tracing::debug!(
        expires_in = token_response.expires_in,
        "obtained google api access token"
    );

    Ok(token_response.access_token)
}

// ---------------------------------------------------------------------------
// Access token (cached)
// ---------------------------------------------------------------------------

/// Get a Google API access token, using KV cache when available.
///
/// If `kv` is provided, reads the cached token from KV. On cache miss,
/// calls `get_access_token` to obtain a fresh token and caches it with
/// a TTL of 3500 seconds (100s buffer before the 3600s Google expiry).
pub async fn get_cached_access_token(
    state: &AppState,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    // Try KV cache first
    if let Some(kv) = kv {
        match kv
            .get(GOOGLE_TOKEN_KV_KEY)
            .text()
            .await
            .map_err(|e| format!("failed to read google token from KV: {e:?}"))
        {
            Ok(Some(token)) => {
                tracing::info!("reused cached google access token from KV");
                return Ok(token);
            }
            Ok(None) => {
                tracing::info!("google access token not in KV, fetching new one");
            }
            Err(e) => {
                tracing::warn!(error = %e, "KV read for google token failed, falling back to fresh token");
            }
        }
    }

    // Cache miss or no KV — obtain a fresh token
    let token = get_access_token(state).await?;

    // Cache the new token in KV
    if let Some(kv) = kv {
        match kv
            .put(GOOGLE_TOKEN_KV_KEY, &token)
            .map_err(|e| format!("failed to build google token KV put: {e:?}"))
        {
            Ok(builder) => match builder
                .expiration_ttl(GOOGLE_TOKEN_TTL_SECS)
                .execute()
                .await
            {
                Ok(()) => {
                    tracing::info!(
                        ttl = GOOGLE_TOKEN_TTL_SECS,
                        "cached google access token in KV"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "failed to cache google token in KV");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "failed to build google token KV put");
            }
        }
    }

    Ok(token)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// URL-safe Base64 encoding (no padding).
fn base64_url_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_url_encode() {
        let input = b"hello world";
        let encoded = base64_url_encode(input);
        assert_eq!(encoded, "aGVsbG8gd29ybGQ");
    }
}
