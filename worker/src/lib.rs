mod adventure;
mod auth;
mod crypto;
mod event_store;
mod handlers;
mod http;
mod middleware;
mod quiz;
mod sheets;
mod solana;
mod state;

use axum::Router;
use tower_service::Service;
use worker::*;

use crate::state::AppState;

/// Embedded `index.html` for SPA fallback — serves the Leptos WASM frontend
/// for any non-API route (e.g. `/staff`, `/admin`).
///
/// Rebuild after frontend changes: `cd frontend-leptos && trunk build`
const INDEX_HTML: &str = include_str!("../../frontend-leptos/dist/index.html");

/// SPA fallback handler — returns the embedded `index.html` for non-API routes.
///
/// The browser loads JS/WASM from the asset layer, then the Leptos router
/// handles the actual path client-side after the WASM app loads.
#[worker::send]
async fn spa_fallback() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}

fn app_router(state: AppState) -> Router {
    let api_routes = handlers::routes(state);

    Router::new()
        .merge(api_routes)
        // Any path not matched by the API routes gets the SPA shell.
        // Leptos router handles /staff, /admin, etc. client-side.
        .fallback(spa_fallback)
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

    let state = state::AppState::from_env(&env)?;

    let mut router = app_router(state);
    Ok(router.call(req).await?)
}
