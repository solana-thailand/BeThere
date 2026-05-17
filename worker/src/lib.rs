mod adventure;
mod audit_store;
mod auth;
mod claim;
mod cleanup;
mod crypto;
mod error;
mod escrow_indexer;
mod event_store;
mod handlers;
mod http;
mod middleware;
mod quiz;

mod sheets;
mod solana;
mod solana_escrow;
mod state;

use axum::Router;
use tower_service::Service;
use worker::*;

use crate::state::AppState;

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

fn app_router(state: AppState) -> Router {
    let api_routes = handlers::routes(state);

    Router::new()
        .merge(api_routes)
        // Any path not matched by the API routes gets the SPA shell.
        // Leptos router handles /staff, /admin, /claim/xxx client-side.
        .fallback(spa_fallback)
        .layer(axum::middleware::from_fn(middleware::correlation_id_layer))
        .layer(axum::middleware::from_fn(
            middleware::security_headers_layer,
        ))
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    console_log::init_with_level(log::Level::Info).ok();

    let state = state::AppState::from_env(&env)?.with_ctx(_ctx);

    let mut router = app_router(state);
    Ok(router.call(req).await?)
}

/// Cron-triggered cleanup — runs daily at 03:00 UTC.
///
/// Deletes expired KV entries (session progress, deposits, claim locks,
/// event configs) based on retention policy defined in `cleanup.rs`.
#[event(scheduled)]
async fn scheduled(_event: worker::ScheduledEvent, env: Env, _ctx: worker::ScheduleContext) {
    console_log::init_with_level(log::Level::Info).ok();

    let events_kv = match env.kv("EVENTS").ok() {
        Some(kv) => kv,
        None => {
            tracing::warn!("cleanup: EVENTS KV namespace not bound, skipping");
            return;
        }
    };

    cleanup::run_cleanup(&events_kv).await;
}
