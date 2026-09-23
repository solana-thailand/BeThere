//! Serve the frontend wasm pre-compressed at brotli quality 11.
//!
//! Cloudflare compresses static assets on the fly at roughly brotli q4. For the
//! 5.5 MB frontend wasm that is ~1.65 MB on the wire; the same bytes at q11 are
//! ~1.29 MB — 380 KB less for every attendee's first load (`.issues/135` §6.3).
//!
//! Workers Static Assets cannot negotiate a pre-compressed sibling itself, so
//! `wrangler.toml` routes the wasm path through the Worker (`run_worker_first`)
//! and this module does the negotiation:
//!
//! - client accepts `br` and `<path>.br` exists → the `.br` bytes, labelled
//!   `content-encoding: br`, with `encodeBody: "manual"` so the runtime does not
//!   compress them a second time;
//! - anything else → the original asset, exactly as before (Cloudflare's own
//!   on-the-fly compression).
//!
//! The fallback is the safety property: a missing `.br` file, a client without
//! brotli, or an ASSETS error all degrade to today's behaviour, never to a
//! blank app.

use axum::body::Body;
use axum::http::{HeaderValue, Response, StatusCode, header};
use worker::{EncodeBody, Env, HttpRequest};

/// Prefix + suffix of the content-hashed frontend wasm Trunk emits.
const WASM_PREFIX: &str = "/event-checkin-frontend-";
const WASM_SUFFIX: &str = "_bg.wasm";

/// Suffix of the pre-compressed sibling written by `frontend-leptos/build.sh`.
pub const BROTLI_SUFFIX: &str = ".br";

/// Content-hashed, so its URL changes every build — same policy as `_headers`.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// True for the one asset path `wrangler.toml` routes through the Worker.
pub fn is_precompressed_asset(path: &str) -> bool {
    path.len() > WASM_PREFIX.len() + WASM_SUFFIX.len()
        && path.starts_with(WASM_PREFIX)
        && path.ends_with(WASM_SUFFIX)
        && !path[1..].contains('/')
}

/// True if an `Accept-Encoding` value admits brotli (`br`, not refused by `q=0`).
pub fn accepts_brotli(accept_encoding: &str) -> bool {
    accept_encoding.split(',').any(|coding| {
        let mut parts = coding.split(';');
        let name = parts.next().unwrap_or_default().trim();
        if !name.eq_ignore_ascii_case("br") {
            return false;
        }
        !parts.any(|param| {
            let param = param.trim();
            match param.split_once('=') {
                Some((key, value)) if key.trim().eq_ignore_ascii_case("q") => {
                    value.trim().parse::<f32>().is_ok_and(|q| q <= 0.0)
                }
                _ => false,
            }
        })
    })
}

/// Serve `req` (a wasm asset path) from the `ASSETS` binding, preferring the
/// brotli-11 sibling when the client accepts it.
pub async fn serve(req: HttpRequest, env: &Env) -> worker::Result<Response<Body>> {
    let assets = env.assets("ASSETS")?;
    let wants_br = req
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(accepts_brotli);

    if wants_br && let Some(resp) = fetch_brotli(&assets, &req).await {
        return Ok(resp);
    }

    // Fallback: the plain asset, compressed on the fly as it always was.
    Ok(assets.fetch_request(req).await?.map(Body::new))
}

/// Fetch `<path>.br`; `None` means "fall back", never "fail".
async fn fetch_brotli(assets: &worker::Fetcher, req: &HttpRequest) -> Option<Response<Body>> {
    let br_url = format!("{}{BROTLI_SUFFIX}", req.uri());
    let mut init = worker::RequestInit::new();
    // Forward the validator so a revalidation can still answer 304.
    if let Some(etag) = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    {
        let headers = worker::Headers::new();
        headers.set("if-none-match", etag).ok()?;
        init.with_headers(headers);
    }
    let upstream = match assets.fetch(br_url, Some(init)).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!(error = %e, "precompressed: ASSETS fetch failed, serving plain wasm");
            return None;
        }
    };

    let status = upstream.status();
    // `not_found_handling = "single-page-application"` answers a missing file
    // with index.html and a 200, so status alone cannot prove the .br exists.
    let is_html = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/html"));
    if is_html || !(status == StatusCode::OK || status == StatusCode::NOT_MODIFIED) {
        tracing::warn!(%status, is_html, "precompressed: no .br sibling, serving plain wasm");
        return None;
    }

    let etag = upstream.headers().get(header::ETAG).cloned();
    let mut resp = upstream.map(Body::new);
    let headers = resp.headers_mut();
    headers.clear();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
    headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Some(etag) = etag {
        headers.insert(header::ETAG, etag);
    }
    if status == StatusCode::OK {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/wasm"),
        );
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
        resp.extensions_mut().insert(EncodeBody::Manual);
    }
    Some(resp)
}
