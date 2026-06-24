mod adventure;
mod audit_store;
mod auth;
mod claim;
mod cleanup;
mod crypto;
mod db;
mod durable_objects;
mod error;
mod escrow_indexer;
mod event_store;
mod handlers;
mod http;
mod middleware;
mod org_store;
mod quiz;

mod sheets;
mod solana;
mod solana_escrow;
mod state;
mod storage;

// Export DO class for workers-rs macro registration
pub use durable_objects::EventDurableObject;

use std::sync::OnceLock;

use axum::response::IntoResponse;
use tower_service::Service;
use worker::*;

/// Logger initialized once per Workers isolate.
static LOG_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Embedded `index.html` for SPA fallback — serves the Leptos WASM frontend
/// for any non-API route (e.g. `/staff`, `/admin`, `/claim/xxx`).
///
/// Rebuild after frontend changes: `cd frontend-leptos && trunk build`
const INDEX_HTML: &str = include_str!("../../frontend-leptos/dist/index.html");

/// `Cache-Control: no-store` for the SPA shell.
///
/// `index.html` references content-hashed JS/WASM whose filenames change every
/// deploy. A stale `index.html` (cached in the browser or at the edge) would
/// reference purged asset hashes → 404 → blank page, only fixed by a hard
/// refresh. `no-store` forces a fresh shell on every navigation.
///
/// NOTE: Cloudflare's `_headers` file does NOT apply to Worker-generated
/// responses (only to responses served by the Static Assets binding). The SPA
/// fallback is Worker-generated for non-asset routes (`/claim/*`, `/staff`,
/// `/admin`), so the header must be set here, not in `_headers`.
static SPA_NO_STORE: std::sync::LazyLock<axum::http::HeaderValue> =
    std::sync::LazyLock::new(|| axum::http::HeaderValue::from_static("no-store"));

/// SPA fallback handler — returns the embedded `index.html` for non-API routes
/// with `Cache-Control: no-store` and the standard security headers.
///
/// The `[assets]` binding in wrangler.toml serves static files (JS, CSS, WASM)
/// from the edge. For HTML navigation routes that don't match a static file,
/// this fallback serves `index.html` so the Leptos client-side router can
/// handle the path. Because this response is Worker-generated, the
/// `security_headers_layer` middleware (which only wraps `api_routes`) and the
/// `_headers` file (which only applies to Static Asset responses) do NOT run —
/// headers must be attached here.
#[worker::send]
async fn spa_fallback() -> axum::http::Response<axum::body::Body> {
    let mut resp = axum::response::Html(INDEX_HTML).into_response();
    let headers = resp.headers_mut();
    headers.insert(axum::http::header::CACHE_CONTROL, SPA_NO_STORE.clone());
    middleware::headers::add_security_headers(resp)
}

/// Returns true if the request path should be served as the SPA fallback
/// (any non-API HTML navigation route). Static assets (JS/CSS/WASM) are
/// served by Cloudflare's `[assets]` binding before the worker is invoked.
fn is_spa_route(path: &str) -> bool {
    !path.starts_with("/api/")
}

/// Build a minimal 503 JSON response for when the worker cannot initialize
/// state or process a request. Avoids opaque Cloudflare error 1101 by
/// returning a structured error instead of propagating via `?` or panicking.
fn service_unavailable() -> axum::http::Response<axum::body::Body> {
    axum::http::Response::builder()
        .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            r#"{"success":false,"error":"service unavailable"}"#,
        ))
        .expect("static 503 response is always valid")
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    // Initialize logger + panic hook once per isolate, not per request.
    // tracing-wasm sends tracing events to console.log (visible in wrangler tail).
    // console_error_panic_hook logs panics to console.error before abort.
    let _ = LOG_INITIALIZED.get_or_init(|| {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();
    });

    // SPA fallback: serve index.html for non-API routes WITHOUT requiring
    // state initialization. This guarantees the frontend (login, claim pages,
    // admin UI) remains available even if a secret is missing or AppState
    // build fails. Static assets (JS/CSS/WASM) are served by Cloudflare's
    // [assets] binding before the worker is invoked.
    let path = req.uri().path();
    if is_spa_route(path) {
        return Ok(spa_fallback().await);
    }

    // API routes require AppState (bindings + config + secrets).
    // Build state; on failure return 503 instead of propagating via `?`
    // (which would surface as opaque Cloudflare error 1101 on every request).
    let state = match state::AppState::from_env(&env) {
        Ok(s) => s.with_ctx(_ctx),
        Err(e) => {
            tracing::error!(error = %e, "AppState::from_env failed");
            return Ok(service_unavailable());
        }
    };

    // Build the API router with middleware stack.
    let mut api_routes = handlers::routes(state)
        .layer(axum::middleware::from_fn(middleware::rate_limit_layer))
        .layer(axum::middleware::from_fn(middleware::correlation_id_layer))
        .layer(axum::middleware::from_fn(
            middleware::security_headers_layer,
        ));

    // Dispatch to the API router. Router::call's error type is Infallible
    // (axum always produces a response), so the Err branch is unreachable —
    // `match e {}` exhausts the Infallible enum.
    match api_routes.call(req).await {
        Ok(resp) => Ok(resp),
        Err(e) => match e {},
    }
}

/// Cron-triggered cleanup — runs daily at 03:00 UTC.
///
/// Deletes expired KV entries (session progress, deposits, claim locks,
/// event configs) based on retention policy defined in `cleanup.rs`.
#[event(scheduled)]
async fn scheduled(_event: worker::ScheduledEvent, env: Env, _ctx: worker::ScheduleContext) {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let events_kv = match env.kv("EVENTS").ok() {
        Some(kv) => kv,
        None => {
            tracing::warn!("cleanup: EVENTS KV namespace not bound, skipping");
            return;
        }
    };
    let d1 = env.d1("DB").ok();

    cleanup::run_cleanup(&events_kv, d1.as_ref()).await;
}
