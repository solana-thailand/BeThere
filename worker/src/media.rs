//! `/media/*`: the landing's "why a deposit" film, served with byte ranges.
//!
//! Workers static assets ignore `Range` and always answer `200` with the
//! whole file (probed on staging 2026-10-06). iOS Safari plays video only
//! from a server that answers ranges with `206`, so the film would not play
//! on a phone. `run_worker_first` routes `/media/*` here; the file is read
//! once per colo from `ASSETS` into the edge cache, and the Cache API turns
//! a `Range` request into a `206` from there ("Results in a 206 response if
//! a matching response with a Content-Length header is found").
//!
//! Without a cache (local `wrangler dev`) the plain asset is served: it
//! plays everywhere except iOS Safari, as before.

use axum::body::Body;
use axum::http::{HeaderValue, Response, StatusCode, header};
use worker::{Env, HttpRequest};

/// Path prefix routed here by `run_worker_first` in `wrangler.toml`.
pub const MEDIA_PREFIX: &str = "/media/";

/// How long the edge and the browser keep a film. The files are renamed to
/// change them, so a day is only a bound on a mistake.
pub const MEDIA_CACHE_CONTROL: &str = "public, max-age=86400";

/// Bumped when what is stored under a key changes: its shape (v=2, fixed
/// length) or the files themselves (v=3, the re-rendered films, 6 Oct).
const CACHE_KEY_VERSION: &str = "v=3";

/// The content type for a servable media path, or `None` for anything else
/// (a nested path, a dot segment or an unknown extension). The films are
/// `mp4`, their posters `jpg`, their captions `vtt`.
pub fn media_content_type(path: &str) -> Option<&'static str> {
    let name = path.strip_prefix(MEDIA_PREFIX)?;
    let valid = !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if !valid {
        return None;
    }
    match name.rsplit_once('.')?.1 {
        "mp4" => Some("video/mp4"),
        "jpg" => Some("image/jpeg"),
        "vtt" => Some("text/vtt; charset=utf-8"),
        _ => None,
    }
}

/// Serve a `/media/*` GET, answering `Range` with `206` when the edge cache
/// is available.
pub async fn serve(
    req: HttpRequest,
    env: &Env,
    content_type: &'static str,
) -> worker::Result<Response<Body>> {
    // The query string is dropped: it cannot change the file.
    let uri = req.uri();
    let url = format!(
        "{}://{}{}",
        uri.scheme_str().unwrap_or("https"),
        uri.authority().map_or("", |a| a.as_str()),
        uri.path()
    );
    // The edge-cache key; the version retires entries stored by an older
    // serving path (the first one stored the stream with no length).
    let key = format!("{url}?{CACHE_KEY_VERSION}");
    let range = req
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let cache = worker::Cache::default();

    if let Some(hit) = cache_lookup(&cache, &key, range.as_deref()).await {
        return finish(hit, content_type);
    }

    let assets = env.assets("ASSETS")?;
    let upstream = assets.fetch(url.clone(), None).await?;
    // `not_found_handling = "single-page-application"` answers a missing file
    // with index.html and a 200.
    let is_html = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/html"));
    if upstream.status() != StatusCode::OK || is_html {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .expect("static response"));
    }

    let stored = match fixed_length(upstream, content_type).await {
        Ok(resp) => cache.put(key.as_str(), resp).await,
        Err(e) => Err(e),
    };
    if let Err(e) = &stored {
        tracing::warn!(error = %e, "media: cache put failed, serving the whole file");
    }
    if stored.is_ok()
        && let Some(hit) = cache_lookup(&cache, &key, range.as_deref()).await
    {
        return finish(hit, content_type);
    }
    // No cache here: the whole file, which every browser but iOS Safari plays.
    let whole = assets.fetch(url, None).await?.map(Body::new);
    finish_http(whole, content_type)
}

/// A copy of the asset with a fixed length. The asset arrives as a stream
/// with no `Content-Length`, and the Cache API cuts a range only from an
/// entry that has one. The bytes stay in a JS `ArrayBuffer`; they are not
/// copied into WASM memory.
async fn fixed_length(
    upstream: Response<worker::Body>,
    content_type: &'static str,
) -> worker::Result<worker::Response> {
    let streamed: web_sys::Response = worker::Response::try_from(upstream)?.into();
    let bytes = wasm_bindgen_futures::JsFuture::from(streamed.array_buffer()?).await?;
    let length = js_sys::ArrayBuffer::from(bytes.clone()).byte_length();
    let headers = web_sys::Headers::new()?;
    headers.set("content-type", content_type)?;
    headers.set("content-length", &length.to_string())?;
    headers.set("cache-control", MEDIA_CACHE_CONTROL)?;
    let init = web_sys::ResponseInit::new();
    init.set_status(200);
    init.set_headers(&headers);
    let fixed = web_sys::Response::new_with_opt_buffer_source_and_init(
        Some(&js_sys::Object::from(bytes)),
        &init,
    )?;
    Ok(worker::Response::from(fixed))
}

/// The cached film for `url`, cut to `range` when one was asked for.
async fn cache_lookup(
    cache: &worker::Cache,
    url: &str,
    range: Option<&str>,
) -> Option<worker::Response> {
    let found = match range {
        Some(range) => {
            let headers = worker::Headers::new();
            headers.set("range", range).ok()?;
            let mut init = worker::RequestInit::new();
            init.with_headers(headers);
            let request = worker::Request::new_with_init(url, &init).ok()?;
            cache.get(&request, false).await
        }
        None => cache.get(url, false).await,
    };
    match found {
        Ok(hit) => hit,
        Err(e) => {
            tracing::warn!(error = %e, "media: cache lookup failed");
            None
        }
    }
}

fn finish(resp: worker::Response, content_type: &'static str) -> worker::Result<Response<Body>> {
    let resp = worker::HttpResponse::try_from(resp)?.map(Body::new);
    finish_http(resp, content_type)
}

/// Pin the headers a player and a shared cache need, whatever the source.
fn finish_http(
    mut resp: Response<Body>,
    content_type: &'static str,
) -> worker::Result<Response<Body>> {
    let headers = resp.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(MEDIA_CACHE_CONTROL),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(resp)
}
