//! Column mapping: row-1 headers → [`ColumnMapping`], cached in KV.

use event_checkin_domain::models::attendee::ColumnMapping;

use worker::KvStore;

use crate::http::fetch_sheet_range;
use crate::state::AppState;

use super::a1;
use super::token::get_cached_access_token;

/// KV key prefix for caching column mappings.
const COLUMN_MAP_CACHE_KEY_PREFIX: &str = "cache:column_map";

/// TTL for the cached column mapping (1 hour — headers rarely change).
const COLUMN_MAP_CACHE_TTL_SECS: u64 = 3600;

// ---------------------------------------------------------------------------
// Cache helpers (column map only — attendee cache removed in Phase 2d)
// ---------------------------------------------------------------------------

/// KV cache key for a sheet's column mapping.
fn column_map_cache_key(sheet_id: &str, sheet_name: &str) -> String {
    format!("{COLUMN_MAP_CACHE_KEY_PREFIX}:{sheet_id}:{sheet_name}")
}

/// Invalidate the column mapping cache for the given sheet.
/// Errors are non-fatal — logged and ignored.
pub(crate) async fn invalidate_column_map_cache(
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

// ---------------------------------------------------------------------------
// Column mapping
// ---------------------------------------------------------------------------

/// [`get_column_mapping`], falling back to the hardcoded layout (logged) when
/// it cannot be resolved. For Sheets-mirror writes, where a wrong-but-standard
/// layout beats not writing at all.
pub async fn column_mapping_or_hardcoded(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> ColumnMapping {
    get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to get column mapping, using hardcoded fallback");
            ColumnMapping::hardcoded()
        })
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
    let sheet_ref = a1::sheet_ref(sheet_name);
    let range = format!("{sheet_ref}!1:1");
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
