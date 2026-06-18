//! Thin fetch wrapper using `web_sys` directly.
//!
//! Replaces `gloo::net::http::Request` to avoid pulling in the entire gloo crate.
//! Uses `web_sys::window().fetch_with_request()` + `wasm_bindgen_futures::JsFuture`.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCache, RequestInit, RequestMode, Response};

use super::types::ApiError;

/// Perform a fetch with the given method, URL, optional headers, and optional body.
///
/// Returns the raw `web_sys::Response`.
pub(crate) async fn fetch(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> Result<Response, ApiError> {
    fetch_inner(method, url, headers, body, false).await
}

/// Perform a fetch that bypasses the browser HTTP cache entirely.
///
/// Sets `RequestCache::NoStore` to prevent the browser from returning
/// stale cached responses. Use for user-specific data that must always
/// be fresh (e.g., ticket data, deposit status).
pub(crate) async fn fetch_no_cache(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> Result<Response, ApiError> {
    fetch_inner(method, url, headers, body, true).await
}

/// Shared fetch implementation with optional cache bypass.
async fn fetch_inner(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
    no_cache: bool,
) -> Result<Response, ApiError> {
    let opts = RequestInit::new();
    opts.set_method(method);
    opts.set_mode(RequestMode::Cors);
    if no_cache {
        opts.set_cache(RequestCache::NoStore);
    }

    if let Some(ref body_str) = body {
        opts.set_body(&JsValue::from_str(body_str));
    }

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| ApiError {
            message: format!("Failed to create request: {e:?}"),
            status: 0,
        })?;

    let req_headers = request.headers();
    for (key, value) in headers {
        req_headers
            .set(*key, *value)
            .map_err(|e| ApiError {
                message: format!("Failed to set header: {e:?}"),
                status: 0,
            })?;
    }

    let window = web_sys::window().ok_or_else(|| ApiError {
        message: "No window".to_string(),
        status: 0,
    })?;

    let promise = window.fetch_with_request(&request);
    let resp_value = JsFuture::from(promise)
        .await
        .map_err(|e| ApiError {
            message: format!("Fetch failed: {e:?}"),
            status: 0,
        })?;

    Ok(Response::from(resp_value))
}

/// Read the response body as text.
pub(crate) async fn response_text(response: &Response) -> Result<String, ApiError> {
    let promise = response.text().map_err(|e| ApiError {
        message: format!("Failed to get response text promise: {e:?}"),
        status: 0,
    })?;
    let text_value = JsFuture::from(promise)
        .await
        .map_err(|e| ApiError {
            message: format!("Failed to read response text: {e:?}"),
            status: 0,
        })?;
    text_value
        .as_string()
        .ok_or_else(|| ApiError {
            message: "Response text is not a string".to_string(),
            status: 0,
        })
}

/// Read the response body as JSON, parse into type T.
pub(crate) async fn response_json<T: serde::de::DeserializeOwned>(
    response: &Response,
) -> Result<T, ApiError> {
    let text = response_text(response).await?;
    serde_json::from_str(&text).map_err(|e| ApiError {
        message: format!("Failed to parse JSON: {e}"),
        status: 0,
    })
}

/// Convenience: GET request.
pub(crate) async fn get(url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
    fetch("GET", url, headers, None).await
}

/// Convenience: GET request that bypasses browser HTTP cache.
pub(crate) async fn get_no_cache(url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
    fetch_no_cache("GET", url, headers, None).await
}

/// Convenience: POST request with optional JSON body.
pub(crate) async fn post(
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> Result<Response, ApiError> {
    fetch("POST", url, headers, body).await
}

/// Convenience: PUT request with optional JSON body.
pub(crate) async fn put(
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> Result<Response, ApiError> {
    fetch("PUT", url, headers, body).await
}

/// Convenience: PATCH request with optional JSON body.
pub(crate) async fn patch(
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> Result<Response, ApiError> {
    fetch("PATCH", url, headers, body).await
}

/// POST request with a raw (non-string) body — used for binary uploads like
/// the event poster. The caller passes a `JsValue` body (e.g. a `web_sys::Blob`
/// from a file input) and a `Content-Type` header in `headers`.
pub(crate) async fn post_raw(
    url: &str,
    headers: &[(&str, &str)],
    body: web_sys::Blob,
) -> Result<Response, ApiError> {
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&body);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| ApiError {
            message: format!("Failed to create request: {e:?}"),
            status: 0,
        })?;

    let req_headers = request.headers();
    for (key, value) in headers {
        req_headers
            .set(*key, *value)
            .map_err(|e| ApiError {
                message: format!("Failed to set header: {e:?}"),
                status: 0,
            })?;
    }

    let window = web_sys::window().ok_or_else(|| ApiError {
        message: "No window".to_string(),
        status: 0,
    })?;

    let promise = window.fetch_with_request(&request);
    let resp_value = JsFuture::from(promise)
        .await
        .map_err(|e| ApiError {
            message: format!("Fetch failed: {e:?}"),
            status: 0,
        })?;

    Ok(Response::from(resp_value))
}

/// Convenience: DELETE request.
pub(crate) async fn delete(url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
    fetch("DELETE", url, headers, None).await
}
