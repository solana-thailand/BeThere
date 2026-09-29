//! HTTP client module wrapping `worker::Fetch` for Google API calls.
//!
//! Replaces `reqwest::Client` from the Axum build. The Workers runtime
//! provides `worker::Fetch` for outbound HTTP — no tokio or reqwest needed.

use serde::Serialize;
use serde::de::DeserializeOwned;
use worker::{Fetch, Headers, Method, Request, RequestInit, Response};

use event_checkin_domain::models::auth::{GoogleUserInfo, TokenRequest, TokenResponse};

// ---------------------------------------------------------------------------
// Generic HTTP helpers
// ---------------------------------------------------------------------------

/// Send one request and fail on a non-2xx status. Every helper below goes
/// through here, so the headers, the body and the status check live in one
/// place.
async fn send(
    method: Method,
    url: &str,
    body: Option<(&str, String)>,
    access_token: Option<&str>,
) -> Result<Response, String> {
    let headers = Headers::new();
    if let Some(token) = access_token {
        headers
            .set("Authorization", &format!("Bearer {token}"))
            .map_err(|e| format!("failed to set auth header: {e:?}"))?;
    }

    let verb: &str = method.as_ref();
    let mut init = RequestInit::new();
    init.with_method(method.clone());
    if let Some((content_type, text)) = body {
        headers
            .set("Content-Type", content_type)
            .map_err(|e| format!("failed to set content-type: {e:?}"))?;
        init.with_body(Some(wasm_bindgen::JsValue::from_str(&text)));
    }
    init.with_headers(headers);

    let request = Request::new_with_init(url, &init)
        .map_err(|e| format!("failed to create {verb} request to {url}: {e:?}"))?;

    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("{verb} {url} failed: {e:?}"))?;

    check_status(&mut response, url).await?;
    Ok(response)
}

/// Parse a response body as JSON; `verb` and `url` only label the error.
async fn read_json<T: DeserializeOwned>(
    mut response: Response,
    verb: &str,
    url: &str,
) -> Result<T, String> {
    response
        .json()
        .await
        .map_err(|e| format!("failed to parse JSON from {verb} {url}: {e:?}"))
}

/// Perform a GET request with a Bearer token and parse the JSON response.
pub async fn get_json<T: DeserializeOwned>(url: &str, access_token: &str) -> Result<T, String> {
    let response = send(Method::Get, url, None, Some(access_token)).await?;
    read_json(response, "GET", url).await
}

/// Perform a POST request with form-encoded body and parse the JSON response.
pub async fn post_form<T: DeserializeOwned>(
    url: &str,
    form_data: &[(&str, &str)],
) -> Result<T, String> {
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form_data.iter().copied())
        .finish();
    let form = Some(("application/x-www-form-urlencoded", body));
    let response = send(Method::Post, url, form, None).await?;
    read_json(response, "POST", url).await
}

/// Send `body` as JSON, with a Bearer token when given.
async fn send_json(
    method: Method,
    url: &str,
    body: &impl Serialize,
    access_token: Option<&str>,
) -> Result<Response, String> {
    let json_body =
        serde_json::to_string(body).map_err(|e| format!("failed to serialize JSON body: {e}"))?;
    let json = Some(("application/json", json_body));
    send(method, url, json, access_token).await
}

/// Perform a POST request with a JSON body and parse the JSON response.
/// Optionally includes a Bearer token for authenticated requests.
pub async fn post_json<T: DeserializeOwned>(
    url: &str,
    body: &impl Serialize,
    access_token: Option<&str>,
) -> Result<T, String> {
    let response = send_json(Method::Post, url, body, access_token).await?;
    read_json(response, "POST", url).await
}

/// POST a JSON body with a Bearer token; only the status matters.
pub async fn post_json_status(
    url: &str,
    body: &impl Serialize,
    access_token: &str,
) -> Result<(), String> {
    send_json(Method::Post, url, body, Some(access_token))
        .await
        .map(drop)
}

/// PUT a JSON body with a Bearer token; only the status matters.
pub async fn put_json(url: &str, body: &impl Serialize, access_token: &str) -> Result<(), String> {
    send_json(Method::Put, url, body, Some(access_token))
        .await
        .map(drop)
}

// ---------------------------------------------------------------------------
// Google API specific helpers
// ---------------------------------------------------------------------------

/// Exchange an OAuth authorization code for tokens.
pub async fn exchange_oauth_code(token_request: &TokenRequest) -> Result<TokenResponse, String> {
    let form_data = [
        ("code", token_request.code.as_str()),
        ("client_id", token_request.client_id.as_str()),
        ("client_secret", token_request.client_secret.as_str()),
        ("redirect_uri", token_request.redirect_uri.as_str()),
        ("grant_type", token_request.grant_type.as_str()),
    ];

    post_form("https://oauth2.googleapis.com/token", &form_data).await
}

/// Fetch the authenticated user's profile from Google's userinfo endpoint.
pub async fn fetch_user_info(access_token: &str) -> Result<GoogleUserInfo, String> {
    get_json(
        "https://www.googleapis.com/oauth2/v2/userinfo",
        access_token,
    )
    .await
}

/// Exchange a signed JWT assertion for a Google API access token.
pub async fn exchange_jwt_assertion(
    token_uri: &str,
    jwt_assertion: &str,
) -> Result<AccessTokenResponse, String> {
    let form_data = [
        ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
        ("assertion", jwt_assertion),
    ];

    post_form(token_uri, &form_data).await
}

/// Google API access token response from service account assertion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

// ---------------------------------------------------------------------------
// Google Sheets API response types
// ---------------------------------------------------------------------------

/// Google Sheets API value range for read/write.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValueRange {
    #[serde(default)]
    pub range: String,
    #[serde(default)]
    pub values: Vec<Vec<String>>,
}

/// Google Sheets API batch update request body.
#[derive(Debug, serde::Serialize)]
pub struct BatchUpdateRequest {
    pub data: Vec<ValueRange>,
    pub value_input_option: String,
}

/// Fetch all rows from a Google Sheet range.
pub async fn fetch_sheet_range(sheet_url: &str, access_token: &str) -> Result<ValueRange, String> {
    get_json(sheet_url, access_token).await
}

/// Update a single cell range in a Google Sheet.
#[allow(dead_code)]
pub async fn update_sheet_range(
    url: &str,
    body: &ValueRange,
    access_token: &str,
) -> Result<(), String> {
    put_json(url, body, access_token).await
}

/// Batch update multiple ranges in a Google Sheet.
pub async fn batch_update_sheet(
    url: &str,
    body: &BatchUpdateRequest,
    access_token: &str,
) -> Result<(), String> {
    // Every cell was skipped (the sheet has none of the columns): nothing to send.
    if body.data.is_empty() {
        return Ok(());
    }
    post_json_status(url, body, access_token).await
}

// ---------------------------------------------------------------------------
// Error checking
// ---------------------------------------------------------------------------

/// Check HTTP response status and return an error with the body if non-2xx.
async fn check_status(response: &mut Response, url: &str) -> Result<(), String> {
    let status = response.status_code();
    if (200..300).contains(&status) {
        return Ok(());
    }

    let body_text = response
        .text()
        .await
        .unwrap_or_else(|_| "<failed to read body>".to_string());

    Err(format!("HTTP {status} from {url}: {body_text}"))
}
