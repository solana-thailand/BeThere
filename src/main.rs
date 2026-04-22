mod auth;
mod config;
mod handlers;
mod middleware;
mod models;
mod qr;
mod sheets;

use axum::Router;
use tokio::signal;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::AppState;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let config = config::AppConfig::from_env().expect("failed to load config");
    let state = AppState::new(config);

    let api_routes = handlers::routes(state.clone());

    let cors_layer = build_cors_layer(&state);

    // Serve Leptos frontend if built, otherwise fall back to legacy JS frontend
    let frontend_dir = if std::path::Path::new("frontend-leptos/dist/index.html").exists() {
        tracing::info!("📦 serving Leptos frontend from frontend-leptos/dist/");
        "frontend-leptos/dist"
    } else {
        tracing::info!("📦 serving legacy frontend from frontend/");
        "frontend"
    };

    // SPA fallback: serve static files (CSS, JS, WASM) from the frontend dir.
    // Any path that doesn't match a real file falls back to index.html so that
    // Leptos client-side router can handle routes like /staff and /admin.
    let index_path = format!("{frontend_dir}/index.html");
    let serve_dir = ServeDir::new(frontend_dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index_path));

    let app = Router::new()
        .merge(api_routes)
        .fallback_service(serve_dir)
        .layer(cors_layer)
        .layer(axum::middleware::from_fn(
            middleware::security_headers_layer,
        ))
        .layer(TraceLayer::new_for_http());

    let addr = state.config.listen_addr();
    tracing::info!("🚀 server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    tracing::info!("server shut down gracefully");
}

/// Build a restrictive CORS layer based on the configured SERVER_URL.
///
/// In production, only the server's own origin is allowed.
/// Falls back to permissive CORS for local development (localhost).
fn build_cors_layer(state: &AppState) -> CorsLayer {
    let server_url = &state.config.server_url;

    let allowed_origin = if server_url.contains("localhost") || server_url.contains("127.0.0.1") {
        tracing::info!("cors: development mode (permissive for localhost)");
        AllowOrigin::any()
    } else {
        tracing::info!("cors: production mode (origin: {server_url})");
        AllowOrigin::exact(
            server_url
                .parse()
                .expect("SERVER_URL must be a valid origin for CORS"),
        )
    };

    CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods(AllowMethods::list([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::list([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]))
}

/// Wait for a shutdown signal (Ctrl+C or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install ctrl+c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("received ctrl+c, shutting down");
        },
        _ = terminate => {
            tracing::info!("received SIGTERM, shutting down");
        },
    }
}
