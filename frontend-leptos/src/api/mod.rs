//! API client for the event check-in backend.
//!
//! Provides typed response structs and authenticated request helpers
//! for all backend endpoints.

// ===== Domain modules =====

mod types;
mod event;
mod attendee;
mod deposit;
mod claim;
mod admin;

// Re-export everything so existing `use crate::api::*` still works.
pub use types::*;
pub use event::*;
pub use attendee::*;
pub use deposit::*;
pub use claim::*;
pub use admin::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use crate::auth::{clear_token, get_token};

// ---------------------------------------------------------------------------
// B5: Stale-while-revalidate in-memory API response cache
// ---------------------------------------------------------------------------

/// Cache entry holding serialized JSON and insertion timestamp.
struct CacheEntry {
    json: String,
    inserted_at: Duration,
}

// Global API response cache (keyed by URL path).
thread_local! {
    static API_CACHE: RefCell<HashMap<String, CacheEntry>> =
        RefCell::new(HashMap::new());
}

/// TTL for cached GET responses (30 seconds).
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Current time as Duration from some epoch (performance.now in WASM).
fn now_secs() -> Duration {
    // In WASM, use performance.now() for sub-ms precision.
    // Fall back to 0 if window is unavailable.
    let ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() as u64)
        .unwrap_or(0);
    Duration::from_millis(ms)
}

/// Try to get a cached response for the given path.
/// Returns `None` if not cached or expired.
fn cache_get(path: &str) -> Option<String> {
    let now = now_secs();
    API_CACHE.with(|c| {
        let cache = c.borrow();
        cache.get(path).and_then(|entry| {
            if now.saturating_sub(entry.inserted_at) < CACHE_TTL {
                Some(entry.json.clone())
            } else {
                None
            }
        })
    })
}

/// Store a response in the cache.
fn cache_put(path: &str, json: &str) {
    let now = now_secs();
    API_CACHE.with(|c| {
        c.borrow_mut().insert(
            path.to_string(),
            CacheEntry {
                json: json.to_string(),
                inserted_at: now,
            },
        );
    });
}

/// Invalidate a cached response (e.g. after a mutation).
pub(crate) fn cache_invalidate(path_prefix: &str) {
    API_CACHE.with(|c| {
        c.borrow_mut().retain(|k, _| !k.starts_with(path_prefix));
    });
}

/// Perform a cached GET request. Returns cached response if fresh,
/// otherwise fetches from the server.
///
/// Uses stale-while-revalidate: returns stale data immediately,
/// then fetches fresh data in the background.
pub(crate) async fn cached_get(path: &str) -> Result<String, ApiError> {
    // Check cache first
    if let Some(cached_json) = cache_get(path) {
        return Ok(cached_json);
    }

    // Cache miss — fetch from server
    let response = api_get(path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Request failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let json = response.text().await.map_err(|e| ApiError {
        message: format!("Failed to read response: {e}"),
        status: 0,
    })?;

    // Store in cache
    cache_put(path, &json);

    Ok(json)
}

// ===== Base URL helper =====

/// Get the API base URL from the current window location.
pub(crate) fn api_base() -> String {
    let window = web_sys::window().expect("no window");
    let location = window.location();
    let origin = location
        .origin()
        .unwrap_or_else(|_| "http://localhost:8787".to_string());
    format!("{origin}/api")
}

// ===== Authenticated request helpers =====

/// Make an authenticated GET request to the API.
pub(crate) async fn api_get(path: &str) -> Result<gloo::net::http::Response, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let mut req = gloo::net::http::Request::get(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }

    let response = req.send().await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    if response.status() == 403 {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Access denied".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Access denied".to_string()),
            status: 403,
        });
    }

    Ok(response)
}

/// Make an authenticated POST request to the API.
pub(crate) async fn api_post(path: &str) -> Result<gloo::net::http::Response, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let mut req = gloo::net::http::Request::post(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    req = req.header("Content-Type", "application/json");

    let response = req.send().await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    Ok(response)
}

/// Make an authenticated DELETE request to the API.
pub(crate) async fn api_delete(path: &str) -> Result<gloo::net::http::Response, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let mut req = gloo::net::http::Request::delete(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }

    let response = req.send().await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    Ok(response)
}

/// HTTP method for JSON body requests.
enum HttpMethod {
    Post,
    Put,
}

/// Shared implementation for POST/PUT requests with JSON body.
async fn api_json_with_body<T: serde::de::DeserializeOwned + Default>(
    method: HttpMethod,
    path: &str,
    body: &impl serde::Serialize,
) -> Result<T, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let json_body = serde_json::to_string(body).map_err(|e| ApiError {
        message: format!("Failed to serialize request: {e}"),
        status: 0,
    })?;

    let mut req = match method {
        HttpMethod::Post => gloo::net::http::Request::post(&url),
        HttpMethod::Put => gloo::net::http::Request::put(&url),
    };
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    req = req.header("Content-Type", "application/json");

    let response = req
        .body(&json_body)
        .map_err(|e| ApiError {
            message: format!("Failed to set request body: {e:?}"),
            status: 0,
        })?
        .send()
        .await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Request failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body
                .error
                .unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let result: ApiResponse<T> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse response: {e}"),
        status: 0,
    })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

/// Make an authenticated POST request with JSON body to the API.
pub(crate) async fn api_post_json<T: serde::de::DeserializeOwned + Default>(
    path: &str,
    body: &impl serde::Serialize,
) -> Result<T, ApiError> {
    api_json_with_body(HttpMethod::Post, path, body).await
}

/// Make an authenticated PUT request with JSON body to the API.
pub(crate) async fn api_put_json<T: serde::de::DeserializeOwned + Default>(
    path: &str,
    body: &impl serde::Serialize,
) -> Result<T, ApiError> {
    api_json_with_body(HttpMethod::Put, path, body).await
}

// ===== Public Auth API functions =====

/// GET /api/auth/url
/// Returns the Google OAuth 2.0 authorization URL.
pub async fn get_auth_url() -> Result<AuthUrlResponse, ApiError> {
    let url = format!("{}/auth/url", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to get auth URL".to_string(),
            status: response.status(),
        });
    }

    let result: ApiResponse<AuthUrlResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse auth URL response: {e}"),
        status: 0,
    })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in auth URL response".to_string(),
        status: 0,
    })
}

/// GET /api/auth/me
/// Returns the current authenticated user info.
pub async fn get_me() -> Result<MeResponse, ApiError> {
    let response = api_get("/auth/me").await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to get user info".to_string(),
            status: response.status(),
        });
    }

    let result: ApiResponse<MeResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse user info: {e}"),
        status: 0,
    })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in user info response".to_string(),
        status: 0,
    })
}
