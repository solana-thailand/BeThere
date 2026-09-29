//! Numeric tab GID resolution (needed by dimension-based Sheets API calls).

use worker::KvStore;

use crate::state::AppState;

/// KV key prefix for caching a sheet tab's numeric GID.
const SHEET_GID_CACHE_KEY_PREFIX: &str = "cache:sheet_gid";

/// TTL for the cached sheet GID (24h — GIDs are stable unless a tab is
/// deleted and recreated, which is rare for an active event).
const SHEET_GID_CACHE_TTL_SECS: u64 = 86400;

fn sheet_gid_cache_key(sheet_id: &str, sheet_name: &str) -> String {
    format!("{SHEET_GID_CACHE_KEY_PREFIX}:{sheet_id}:{sheet_name}")
}

/// Resolve the numeric GID (`sheetId`) of a tab by its name.
///
/// `deleteDimension` and other dimension-based Sheets API calls require the
/// numeric GID, NOT the tab name. The GID is assigned by Google when a tab is
/// created and is NOT predictable — e.g. the first tab of a spreadsheet is
/// often a large number like `104609663`, not `0`. Hardcoding `0` causes the
/// delete to target a non-existent sheet and silently no-op.
///
/// Resolution order:
/// 1. KV cache (24h TTL — GIDs are stable).
/// 2. Spreadsheet metadata: `GET /v4/spreadsheets/{id}?fields=sheets.properties`
///    → match tab title case-insensitively; fall back to the first tab.
/// 3. Cache the resolved GID in KV.
pub async fn resolve_sheet_gid(
    _state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    access_token: &str,
) -> Result<i64, String> {
    let cache_key = sheet_gid_cache_key(sheet_id, sheet_name);

    // 1. KV cache
    if let Some(kv) = kv {
        match kv.get(&cache_key).text().await {
            Ok(Some(cached)) => {
                if let Ok(gid) = cached.trim().parse::<i64>() {
                    tracing::debug!(
                        sheet_id = %sheet_id,
                        sheet_name = %sheet_name,
                        gid, "sheet gid cache hit"
                    );
                    return Ok(gid);
                }
            }
            Ok(None) => {}
            Err(e) => tracing::debug!(error = ?e, "sheet gid cache read error"),
        }
    }

    // 2. Fetch spreadsheet metadata and match by title.
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}?fields=sheets(properties(title,sheetId,index))"
    );
    let meta: SheetMetaResponse = crate::http::get_json(&url, access_token).await?;

    let gid = meta
        .sheets
        .iter()
        // Prefer an exact (case-insensitive) title match.
        .find(|s| s.properties.title.eq_ignore_ascii_case(sheet_name))
        .map(|s| s.properties.sheet_id)
        // Fall back to the tab with the lowest index (usually the first tab).
        .or_else(|| {
            meta.sheets
                .iter()
                .min_by_key(|s| s.properties.index)
                .map(|s| s.properties.sheet_id)
        })
        .ok_or_else(|| {
            format!("spreadsheet {sheet_id} has no sheets to resolve gid for '{sheet_name}'")
        })?;

    tracing::info!(
        sheet_id = %sheet_id,
        sheet_name = %sheet_name,
        gid, "resolved sheet gid from metadata"
    );

    // 3. Cache in KV.
    if let Some(kv) = kv {
        match kv.put(&cache_key, gid.to_string()) {
            Ok(builder) => match builder
                .expiration_ttl(SHEET_GID_CACHE_TTL_SECS)
                .execute()
                .await
            {
                Ok(()) => {
                    tracing::debug!(ttl = SHEET_GID_CACHE_TTL_SECS, "cached sheet gid in KV");
                }
                Err(e) => tracing::warn!(error = %e, "failed to cache sheet gid in KV"),
            },
            Err(e) => tracing::warn!(error = %e, "failed to build sheet gid KV put"),
        }
    }

    Ok(gid)
}

/// Subset of the Sheets API spreadsheet metadata response.
/// Only the fields we need (`sheets.properties.title/sheetId/index`).
#[derive(serde::Deserialize)]
struct SheetMetaResponse {
    sheets: Vec<SheetMeta>,
}

#[derive(serde::Deserialize)]
struct SheetMeta {
    #[serde(rename = "properties")]
    properties: SheetProperties,
}

#[derive(serde::Deserialize)]
struct SheetProperties {
    title: String,
    #[serde(rename = "sheetId")]
    sheet_id: i64,
    index: i64,
}
