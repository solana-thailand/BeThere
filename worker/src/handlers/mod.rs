pub mod attendee;
pub mod auth;
pub mod checkin;
pub mod health;
pub mod qr;

use crate::state::AppState;
use axum::{
    Router, middleware,
    routing::{get, post},
};

pub fn routes(state: AppState) -> Router<()> {
    let api = Router::new()
        .route("/health", get(health::health_check))
        // Auth routes (public)
        .route("/auth/url", get(auth::auth_url))
        .route("/auth/callback", get(auth::auth_callback))
        .route("/auth/logout", get(auth::auth_logout))
        .route("/auth/me", get(auth::auth_me))
        // Protected routes
        .route("/attendees", get(attendee::list_attendees))
        .route("/attendee/{id}", get(attendee::get_attendee))
        .route("/checkin/{id}", post(checkin::check_in))
        .route("/generate-qrs", post(qr::generate_qrs))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_auth,
        ));

    Router::new().nest("/api", api).with_state(state)
}
