//! Staff list from the "staff" tab, cached per isolate and in KV.

use worker::KvStore;

use crate::http::{ValueRange, fetch_sheet_range};
use crate::state::AppState;

use super::a1;
use super::token::get_cached_access_token;

/// KV key for caching the staff members list. `v2` holds a
/// [`StaffCacheEntry`]; the old plain-array key expires on its own TTL.
const STAFF_CACHE_KEY: &str = "cache:staff_members:v2";

/// TTL for the cached staff members list (60 seconds). This is also the
/// revocation latency: the owner kept it at 60 s (plan 028 W1, 2026-09-24).
const STAFF_CACHE_TTL_SECS: u64 = 60;
const STAFF_CACHE_TTL_MS: f64 = (STAFF_CACHE_TTL_SECS * 1000) as f64;

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

/// The staff list as cached in KV, stamped with when it was read from Sheets.
#[derive(serde::Serialize, serde::Deserialize)]
struct StaffCacheEntry {
    fetched_at_ms: f64,
    members: Vec<StaffMember>,
}

type StaffIsolateCache =
    crate::isolate_cache::BoundedCache<String, crate::isolate_cache::Expiring<Vec<StaffMember>>>;

thread_local! {
    /// The staff list per isolate (plan 028 W1). It expires at the Sheets
    /// fetch time + TTL, the same instant the KV copy does, so it saves the KV
    /// read on every authed request without lengthening revocation.
    static STAFF_IN_ISOLATE: std::cell::RefCell<StaffIsolateCache> =
        const { std::cell::RefCell::new(crate::isolate_cache::BoundedCache::new(1)) };
}

fn remember_staff_in_isolate(key: String, entry: &StaffCacheEntry) {
    let expires_at_ms = entry.fetched_at_ms + STAFF_CACHE_TTL_MS;
    let value = crate::isolate_cache::Expiring::new(entry.members.clone(), expires_at_ms);
    STAFF_IN_ISOLATE.with_borrow_mut(|cache| cache.insert(key, value));
}

/// Fetch staff members from the dedicated "staff" sheet tab.
///
/// Checks this isolate's copy first, then the KV cache: returns cached staff on cache hit,
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
    let now_ms = js_sys::Date::now();
    let isolate_key = format!("{sheet_id}\n{staff_sheet_name}");
    let in_isolate = STAFF_IN_ISOLATE.with_borrow(|cache| {
        cache
            .get(isolate_key.as_str())
            .and_then(|entry| entry.get(now_ms))
    });
    if let Some(members) = in_isolate {
        tracing::debug!(count = members.len(), "cache hit: staff members in isolate");
        return Ok(members);
    }

    // Then the KV cache
    if let Some(kv) = kv {
        match kv.get(STAFF_CACHE_KEY).text().await {
            Ok(Some(cached)) => match serde_json::from_str::<StaffCacheEntry>(&cached) {
                Ok(entry) => {
                    tracing::info!(
                        count = entry.members.len(),
                        "cache hit: staff members from KV"
                    );
                    remember_staff_in_isolate(isolate_key, &entry);
                    return Ok(entry.members);
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
    let sheet_ref = a1::sheet_ref(staff_sheet_name);
    let range = format!("{sheet_ref}!A2:B");
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

    // Stamp with the pre-fetch time: the earlier instant expires sooner.
    let entry = StaffCacheEntry {
        fetched_at_ms: now_ms,
        members,
    };
    remember_staff_in_isolate(isolate_key, &entry);

    // Write to KV cache
    if let (Some(kv), Ok(json)) = (kv, serde_json::to_string(&entry)) {
        match kv
            .put(STAFF_CACHE_KEY, &json)
            .map_err(|e| format!("failed to build staff cache KV put: {e:?}"))
        {
            Ok(builder) => {
                if let Err(e) = builder.expiration_ttl(STAFF_CACHE_TTL_SECS).execute().await {
                    tracing::info!(error = ?e, "failed to cache staff members in KV");
                } else {
                    tracing::info!(
                        count = entry.members.len(),
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

    Ok(entry.members)
}
