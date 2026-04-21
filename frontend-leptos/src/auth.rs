use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use wasm_bindgen::prelude::*;
use web_sys::window;

/// localStorage key for the JWT session token.
const TOKEN_KEY: &str = "event_checkin_token";

/// Read the JWT token from localStorage.
pub fn get_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|storage| storage.get_item(TOKEN_KEY).ok())
        .flatten()
}

/// Write the JWT token to localStorage.
pub fn set_token(token: &str) {
    if let Some(storage) = window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item(TOKEN_KEY, token);
    }
}

/// Remove the JWT token from localStorage.
pub fn clear_token() {
    if let Some(storage) = window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item(TOKEN_KEY);
    }
}

/// Parse the JWT payload without validation (client-side only).
/// Returns the decoded claims as a serde_json::Value.
fn parse_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload = parts.get(1)?;
    // Base64url decode
    let decoded = js_sys::atob(
        &base64url_to_base64(payload),
    )
    .ok()?
    .as_string()?;

    serde_json::from_str(&decoded).ok()
}

/// Convert base64url encoding to standard base64.
fn base64url_to_base64(input: &str) -> String {
    let mut s = input.replace('-', "+").replace('_', "/");
    let padding = (4 - s.len() % 4) % 4;
    for _ in 0..padding {
        s.push('=');
    }
    s
}

/// Check if the stored token is expired.
/// Returns `true` if expired or no token present.
pub fn is_token_expired() -> bool {
    let token = match get_token() {
        Some(t) => t,
        None => return true,
    };

    let claims = match parse_jwt_payload(&token) {
        Some(c) => c,
        None => return true,
    };

    let exp = match claims.get("exp").and_then(|v| v.as_u64()) {
        Some(e) => e,
        None => return true,
    };

    let now = (js_sys::Date::now() / 1000.0) as u64;
    exp < now
}

/// Check if the user is authenticated (token exists and not expired).
pub fn is_authenticated() -> bool {
    get_token().is_some() && !is_token_expired()
}

/// Extract the email from the stored JWT token, if present and valid.
pub fn get_token_email() -> Option<String> {
    let token = get_token()?;
    let claims = parse_jwt_payload(&token)?;
    claims.get("email").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Handle token from URL query params (after OAuth callback redirect).
/// If a `token` param exists in the current URL:
/// 1. Saves it to localStorage
/// 2. Cleans the URL by removing the query param
/// Returns `true` if a token was found and saved.
pub fn handle_token_from_url() -> bool {
    let window = match window() {
        Some(w) => w,
        None => return false,
    };

    let url = match web_sys::Url::new(
        &window.location().href().unwrap_or_default(),
    )
    .ok()
    {
        Some(u) => u,
        None => return false,
    };

    let token = match url.search_params().get("token") {
        Some(t) => t,
        None => return false,
    };

    if token.is_empty() {
        return false;
    }

    // Save the token
    set_token(&token);

    // Clean up the URL
    url.search_params().delete("token");
    let clean_path = url.pathname();
    let _ = window.history().and_then(|h| {
        h.replace_state_with_url(
            &JsValue::NULL,
            "",
            Some(&clean_path),
        )
    });

    log::info!("[auth] token saved from URL, cleaned up query params");
    true
}

/// Handle error from URL query params (after OAuth callback redirect failure).
/// Returns a human-readable error message if an error param is present.
pub fn get_url_error() -> Option<String> {
    let window = window()?;
    let url = web_sys::Url::new(&window.location().href().ok()?).ok()?;
    let error = url.search_params().get("error")?;

    let message = match error.as_str() {
        "not_authorized" => {
            "You are not authorized to access this system. Only approved staff members can sign in.".to_string()
        }
        "auth_failed" => {
            let msg = url.search_params().get("message").unwrap_or_default();
            format!("Authentication failed. Please try again. ({msg})")
        }
        "oauth_failed" => {
            let msg = url.search_params().get("message").unwrap_or_default();
            format!("Google authentication was cancelled or failed. ({msg})")
        }
        "missing_code" => {
            "Invalid authentication response. Please try again.".to_string()
        }
        "token_failed" => {
            "Failed to create session. Please try again.".to_string()
        }
        "session_expired" => {
            "Your session has expired. Please sign in again.".to_string()
        }
        _ => "An unexpected error occurred. Please try again.".to_string(),
    };

    // Clean up URL
    url.search_params().delete("error");
    url.search_params().delete("message");
    let clean_path = url.pathname();
    let _ = window.history().and_then(|h| {
        h.replace_state_with_url(&JsValue::NULL, "", Some(&clean_path))
    });

    Some(message)
}

/// Redirect to login page if not authenticated.
/// Call this at the top of protected page components.
pub fn require_auth(navigate: &dyn Fn(&str)) {
    if !is_authenticated() {
        clear_token();
        navigate("/");
    }
}

/// Logout: clear token and redirect to login page.
pub fn logout(navigate: &dyn Fn(&str)) {
    clear_token();
    navigate("/");
}
