//! Google Sheets API operations for the Cloudflare Worker.
//!
//! Mirrors `src/sheets/client.rs` from the Axum build but uses
//! `worker::Fetch` (via `crate::http`) and SubtleCrypto (via `crate::crypto`)
//! instead of `reqwest` and the `rsa` crate.

pub mod write;

use base64::Engine;
use std::collections::HashMap;

use event_checkin_domain::models::attendee::{Attendee, AttendeeRow, ColumnMapping};
use event_checkin_domain::models::auth::ServiceAccountClaim;

use worker::KvStore;

use crate::crypto;
use crate::http::{AccessTokenResponse, ValueRange, exchange_jwt_assertion, fetch_sheet_range};
use crate::state::AppState;

// Re-export all public write functions for backward compatibility.
pub use write::*;

/// KV key for caching the Google API access token.
const GOOGLE_TOKEN_KV_KEY: &str = "google_access_token";

/// TTL for the cached Google access token (3500s = ~58 min, 100s buffer before 3600s expiry).
const GOOGLE_TOKEN_TTL_SECS: u64 = 3500;

/// KV key prefix for caching attendee lists.
const ATTENDEE_CACHE_KEY_PREFIX: &str = "cache:attendees";

/// TTL for the cached attendee list (5 minutes).
const ATTENDEE_CACHE_TTL_SECS: u64 = 300;

/// KV key for caching the staff members list.
const STAFF_CACHE_KEY: &str = "cache:staff_members";

/// TTL for the cached staff members list (60 seconds).
const STAFF_CACHE_TTL_SECS: u64 = 60;

/// KV key prefix for caching column mappings.
const COLUMN_MAP_CACHE_KEY_PREFIX: &str = "cache:column_map";

/// TTL for the cached column mapping (1 hour — headers rarely change).
const COLUMN_MAP_CACHE_TTL_SECS: u64 = 3600;

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

/// Build the KV cache key for a given (sheet_id, sheet_name) combination.
fn attendee_cache_key(sheet_id: &str, sheet_name: &str) -> String {
    format!("{ATTENDEE_CACHE_KEY_PREFIX}:{sheet_id}:{sheet_name}")
}

/// Invalidate the attendee cache for the given sheet after a mutation.
/// Errors are non-fatal — logged and ignored.
pub(super) async fn invalidate_attendee_cache(
    kv: Option<&KvStore>,
    sheet_id: &str,
    sheet_name: &str,
) {
    if let Some(kv) = kv {
        let key = attendee_cache_key(sheet_id, sheet_name);
        if let Err(e) = kv.delete(&key).await {
            tracing::debug!(error = ?e, "failed to invalidate attendee cache");
        }
    }
}

/// Invalidate the column mapping cache for the given sheet.
/// Errors are non-fatal — logged and ignored.
pub(super) async fn invalidate_column_map_cache(
    kv: Option<&KvStore>,
    sheet_id: &str,
    sheet_name: &str,
) {
    if let Some(kv) = kv {
        let key = column_map_cache_key(sheet_id, sheet_name);
        if let Err(e) = kv.delete(&key).await {
            tracing::debug!(error = ?e, "failed to invalidate column map cache");
        }
    }
}

/// Flush all caches (attendee list + column mapping) for a sheet.
pub async fn flush_caches(
    _state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) {
    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    tracing::info!(sheet_id = %sheet_id, sheet_name = %sheet_name, "flushed all caches");
}

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
// Attendee queries
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Column mapping
// ---------------------------------------------------------------------------

/// KV cache key for a sheet's column mapping.
fn column_map_cache_key(sheet_id: &str, sheet_name: &str) -> String {
    format!("{COLUMN_MAP_CACHE_KEY_PREFIX}:{sheet_id}:{sheet_name}")
}

/// Get the column mapping for a sheet.
///
/// Resolution order:
/// 1. KV cache (if available)
/// 2. Read row 1 headers from Google Sheets, build mapping, cache in KV
/// 3. Fall back to hardcoded mapping on any error
pub async fn get_column_mapping(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<ColumnMapping, String> {
    let cache_key = column_map_cache_key(sheet_id, sheet_name);

    // 1. Try KV cache
    if let Some(kv) = kv {
        match kv.get(&cache_key).text().await {
            Ok(Some(cached)) => {
                if let Ok(mapping) = serde_json::from_str::<ColumnMapping>(&cached) {
                    tracing::debug!(
                        mapped = mapping.mapped_count(),
                        total = mapping.total_columns,
                        "column mapping cache hit"
                    );
                    return Ok(mapping);
                }
            }
            Ok(None) => {
                tracing::debug!(cache_key = %cache_key, "column mapping cache miss");
            }
            Err(e) => {
                tracing::debug!(error = ?e, "column mapping cache read error");
            }
        }
    }

    // 2. Read row 1 headers from Google Sheets
    let access_token = get_cached_access_token(state, kv).await?;
    let range = format!("{sheet_name}!1:1");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    match fetch_sheet_range(&url, &access_token).await {
        Ok(header_range) => {
            if let Some(headers) = header_range.values.first() {
                let mapping = ColumnMapping::from_headers(headers);
                tracing::info!(
                    mapped = mapping.mapped_count(),
                    total = mapping.total_columns,
                    "built column mapping from sheet headers"
                );

                // Cache the mapping in KV
                if let Some(kv) = kv
                    && let Ok(json) = serde_json::to_string(&mapping)
                    && let Ok(builder) = kv
                        .put(&cache_key, &json)
                        .map_err(|e| format!("failed to build column map KV put: {e:?}"))
                    && let Err(e) = builder
                        .expiration_ttl(COLUMN_MAP_CACHE_TTL_SECS)
                        .execute()
                        .await
                {
                    tracing::debug!(error = ?e, "failed to cache column mapping");
                }

                return Ok(mapping);
            }

            // Empty header row — fall through to hardcoded
            tracing::warn!("sheet header row is empty, using hardcoded mapping");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to read sheet headers, using hardcoded mapping"
            );
        }
    }

    // 3. Fallback to hardcoded
    Ok(ColumnMapping::hardcoded())
}

// ---------------------------------------------------------------------------
// Attendee queries
// ---------------------------------------------------------------------------

/// Fetch all attendees from the Google Sheet.
/// Returns a list of typed Attendee structs parsed from sheet rows.
///
/// Uses KV cache when available: returns cached attendees on cache hit,
/// fetches from Google Sheets on cache miss and stores the result with
/// a 30-second TTL.
pub async fn get_attendees(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Vec<Attendee>, String> {
    let cache_key = attendee_cache_key(sheet_id, sheet_name);

    // Try KV cache first
    if let Some(kv) = kv {
        match kv.get(&cache_key).text().await {
            Ok(Some(cached)) => match serde_json::from_str::<Vec<Attendee>>(&cached) {
                Ok(attendees) => {
                    tracing::info!(
                        count = attendees.len(),
                        cache_key = %cache_key,
                        "cache hit: attendees from KV"
                    );
                    return Ok(attendees);
                }
                Err(e) => {
                    tracing::info!(error = ?e, "cache deserialize error, fetching fresh");
                }
            },
            Ok(None) => {
                tracing::info!(cache_key = %cache_key, "cache miss: KV key");
            }
            Err(e) => {
                tracing::info!(error = ?e, "cache read error, fetching fresh");
            }
        }
    }

    // Cache miss or no KV — fetch from Google Sheets
    let access_token = get_cached_access_token(state, kv).await?;

    // Resolve column mapping from headers
    let mapping = get_column_mapping(state, sheet_id, sheet_name, kv).await?;

    let last_col = mapping.last_column_letter();
    let range = format!("{sheet_name}!A2:{last_col}");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let value_range: ValueRange = fetch_sheet_range(&url, &access_token).await?;

    let attendees: Vec<Attendee> = value_range
        .values
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.is_empty())
        .filter(|(_, row)| row.first().is_some_and(|v| !v.trim().is_empty()))
        .filter_map(|(idx, _)| {
            // row_index is 1-based in the sheet, +2 because row 1 is header and idx is 0-based
            let row_index = idx + 2;
            AttendeeRow::from_sheet_values(&value_range.values, row_index, &mapping)
        })
        .map(|row| row.to_attendee())
        .collect();

    tracing::info!(
        count = attendees.len(),
        "fetched attendees from google sheets"
    );

    // Write to KV cache
    if let Some(kv) = kv {
        match serde_json::to_string(&attendees) {
            Ok(json) => match kv
                .put(&cache_key, &json)
                .map_err(|e| format!("failed to build attendee cache KV put: {e:?}"))
            {
                Ok(builder) => match builder
                    .expiration_ttl(ATTENDEE_CACHE_TTL_SECS)
                    .execute()
                    .await
                {
                    Ok(()) => {
                        tracing::info!(
                            count = attendees.len(),
                            cache_key = %cache_key,
                            ttl = ATTENDEE_CACHE_TTL_SECS,
                            "cached attendees in KV"
                        );
                    }
                    Err(e) => {
                        tracing::info!(error = ?e, "failed to cache attendees in KV");
                    }
                },
                Err(e) => {
                    tracing::info!(error = %e, "failed to build attendee cache KV put");
                }
            },
            Err(e) => {
                tracing::info!(error = ?e, "failed to serialize attendees for cache");
            }
        }
    }

    Ok(attendees)
}

// ---------------------------------------------------------------------------
// HashMap helpers for O(1) lookups
// ---------------------------------------------------------------------------

/// Build a HashMap of attendees keyed by `api_id`.
/// Internally calls `get_attendees()` so KV caching is preserved.
pub async fn get_attendees_map(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<HashMap<String, Attendee>, String> {
    let attendees: Vec<Attendee> = get_attendees(state, sheet_id, sheet_name, kv).await?;
    Ok(attendees
        .into_iter()
        .map(|a| (a.api_id.clone(), a))
        .collect())
}

/// Build a HashMap of attendees keyed by `claim_token`.
/// Only attendees with `Some(token)` are included.
/// Internally calls `get_attendees()` so KV caching is preserved.
async fn get_attendees_by_claim_map(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<HashMap<String, Attendee>, String> {
    let attendees: Vec<Attendee> = get_attendees(state, sheet_id, sheet_name, kv).await?;
    Ok(attendees
        .into_iter()
        .filter_map(|a| a.claim_token.clone().map(|token| (token, a)))
        .collect())
}

/// Get a single attendee by their api_id — O(1) via HashMap lookup.
pub async fn get_attendee_by_id(
    api_id: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Option<Attendee>, String> {
    let map = get_attendees_map(state, sheet_id, sheet_name, kv).await?;
    Ok(map.get(api_id).cloned())
}

/// Find an attendee by their claim token (column L) — O(1) via HashMap lookup.
/// Returns `None` if no attendee has the given token.
pub async fn get_attendee_by_claim_token(
    claim_token: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Option<Attendee>, String> {
    let map = get_attendees_by_claim_map(state, sheet_id, sheet_name, kv).await?;
    Ok(map.get(claim_token).cloned())
}

/// Look up an attendee by claim token and return claim counts in a single Sheets API call.
/// Returns `(attendee, total_checked_in, total_claimed)`.
/// Attendee lookup is O(1) via HashMap; counts iterate the full Vec.
pub async fn get_attendee_with_claim_counts(
    claim_token: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(Option<Attendee>, usize, usize), String> {
    let attendees: Vec<Attendee> = get_attendees(state, sheet_id, sheet_name, kv).await?;
    let total_checked_in = attendees
        .iter()
        .filter(|a| a.checked_in_at.is_some())
        .count();
    let total_claimed = attendees.iter().filter(|a| a.claimed_at.is_some()).count();

    // O(1) lookup by claim token
    let claim_map: HashMap<String, Attendee> = attendees
        .into_iter()
        .filter_map(|a| {
            let token = a.claim_token.clone()?;
            Some((token, a))
        })
        .collect();
    let attendee = claim_map.get(claim_token).cloned();

    Ok((attendee, total_checked_in, total_claimed))
}

// ---------------------------------------------------------------------------
// Staff queries
// ---------------------------------------------------------------------------

/// A staff member entry from the Google Sheets "staff" tab.
///
/// Column mapping:
///   A[0] = email
///   B[1] = role ("admin" or "staff")
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StaffMember {
    /// Staff email address (lowercased).
    pub email: String,
    /// Role: "admin" (full access) or "staff" (scanner only).
    /// Defaults to "staff" if column B is empty.
    pub role: String,
}

/// Fetch staff members from the dedicated "staff" sheet tab.
///
/// Uses KV cache when available: returns cached staff on cache hit,
/// fetches from Google Sheets on cache miss and stores with 60-second TTL.
///
/// Reads columns A (email) and B (role) starting from row 2 (row 1 is header).
/// Returns a list of `StaffMember` with lowercased emails and role.
///
/// If column B (role) is empty, defaults to "staff".
/// Valid roles: "admin" (scanner + admin dashboard), "staff" (scanner only).
pub async fn get_staff_members(
    state: &AppState,
    sheet_id: &str,
    staff_sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Vec<StaffMember>, String> {
    // Try KV cache first
    if let Some(kv) = kv {
        match kv.get(STAFF_CACHE_KEY).text().await {
            Ok(Some(cached)) => match serde_json::from_str::<Vec<StaffMember>>(&cached) {
                Ok(members) => {
                    tracing::info!(count = members.len(), "cache hit: staff members from KV");
                    return Ok(members);
                }
                Err(e) => {
                    tracing::info!(error = ?e, "staff cache deserialize error, fetching fresh");
                }
            },
            Ok(None) => {
                tracing::info!("cache miss: staff members not in KV");
            }
            Err(e) => {
                tracing::info!(error = ?e, "staff cache read error, fetching fresh");
            }
        }
    }

    // Cache miss or no KV — fetch from Google Sheets
    let access_token = get_cached_access_token(state, kv).await?;
    let range = format!("{staff_sheet_name}!A2:B");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let value_range: ValueRange = fetch_sheet_range(&url, &access_token).await?;

    let members: Vec<StaffMember> = value_range
        .values
        .iter()
        .filter_map(|row| {
            let email = row.first().cloned().unwrap_or_default().trim().to_string();
            if email.is_empty() {
                return None;
            }
            let role = row
                .get(1)
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "staff".to_string());
            Some(StaffMember {
                email: email.to_lowercase(),
                role,
            })
        })
        .collect();

    tracing::info!(
        count = members.len(),
        "fetched staff members from google sheets"
    );

    // Write to KV cache
    if let (Some(kv), Ok(json)) = (kv, serde_json::to_string(&members)) {
        match kv
            .put(STAFF_CACHE_KEY, &json)
            .map_err(|e| format!("failed to build staff cache KV put: {e:?}"))
        {
            Ok(builder) => {
                if let Err(e) = builder.expiration_ttl(STAFF_CACHE_TTL_SECS).execute().await {
                    tracing::info!(error = ?e, "failed to cache staff members in KV");
                } else {
                    tracing::info!(
                        count = members.len(),
                        ttl = STAFF_CACHE_TTL_SECS,
                        "cached staff members in KV"
                    );
                }
            }
            Err(e) => {
                tracing::info!(error = %e, "failed to build staff cache KV put");
            }
        }
    }

    Ok(members)
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
