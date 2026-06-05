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

use axum::Router;
use tower_service::Service;
use worker::*;

/// Logger initialized once per Workers isolate.
static LOG_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Cached router skeleton — route definitions + middleware layers are static.
/// Only the per-request state (worker_ctx) changes, injected via Extension.
static CACHED_ROUTER: OnceLock<Router> = OnceLock::new();

/// Embedded `index.html` for SPA fallback — serves the Leptos WASM frontend
/// for any non-API route (e.g. `/staff`, `/admin`, `/claim/xxx`).
///
/// Rebuild after frontend changes: `cd frontend-leptos && trunk build`
const INDEX_HTML: &str = include_str!("../../frontend-leptos/dist/index.html");

/// SPA fallback handler — returns the embedded `index.html` for non-API routes.
///
/// The `[assets]` binding in wrangler.toml serves static files (JS, CSS, WASM)
/// from the edge. For HTML navigation routes that don't match a static file,
/// this fallback serves `index.html` so the Leptos client-side router can
/// handle the path.
#[worker::send]
async fn spa_fallback() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}

/// Build the router skeleton — route definitions + middleware layers.
///
/// Route tables and middleware stacks are identical across all requests.
/// State is injected per-request via `Extension` for `worker_ctx`.
fn build_router_skeleton() -> Router {
    Router::new().fallback(spa_fallback)
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

    let state = state::AppState::from_env(&env)?.with_ctx(_ctx);

    // H1: Cache router skeleton — routes + middleware are static
    let router = match CACHED_ROUTER.get() {
        Some(cached) => cached.clone(),
        None => {
            let skeleton = build_router_skeleton();
            let _ = CACHED_ROUTER.set(skeleton.clone());
            skeleton
        }
    };

    // Merge state-dependent API routes per request
    // Middleware from the skeleton covers its own routes (SPA fallback),
    // but merged routes need their own layer application.
    let api_routes = handlers::routes(state)
        .layer(axum::middleware::from_fn(middleware::rate_limit_layer))
        .layer(axum::middleware::from_fn(middleware::correlation_id_layer))
        .layer(axum::middleware::from_fn(
            middleware::security_headers_layer,
        ));
    let mut router = router.merge(api_routes);
    let resp = router.call(req).await.expect("router call is infallible");
    Ok(resp)
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

    cleanup::run_cleanup(&events_kv).await;
}
