pub mod attendee;
pub mod auth;
pub mod checkin;
pub mod claim;
pub mod health;
pub mod qr;
pub mod quiz;
pub mod waitlist;

use crate::state::AppState;
use axum::{
    Router, middleware,
    routing::{get, post},
};

pub fn routes(state: AppState) -> Router<()> {
    // Public routes — no auth middleware required
    let public = Router::new()
        .route("/health", get(health::health_check))
        // Auth routes (public)
        .route("/auth/url", get(auth::auth_url))
        .route("/auth/callback", get(auth::auth_callback))
        .route("/auth/logout", get(auth::auth_logout))
        // Claim routes (public — attendees claim NFTs without staff login)
        .route("/claim/{token}", get(claim::get_claim))
        .route("/claim/{token}", post(claim::post_claim))
        // Quiz routes (public — attendees take quiz after check-in)
        .route("/quiz", get(quiz::get_quiz))
        .route("/quiz/{token}/submit", post(quiz::submit_quiz))
        .route("/quiz/{token}/status", get(quiz::get_quiz_status))
        // Waitlist signup (public)
        .route("/waitlist", post(waitlist::join_waitlist));

    // Protected routes — require staff auth
    let protected = Router::new()
        // Auth route that requires session (reads Claims from middleware)
        .route("/auth/me", get(auth::auth_me))
        .route("/attendees", get(attendee::list_attendees))
        .route("/attendee/{id}", get(attendee::get_attendee))
        .route("/checkin/{id}", post(checkin::check_in))
        .route("/generate-qrs", post(qr::generate_qrs))
        // Admin quiz management (protected — organizer sets questions)
        .route("/admin/quiz", post(quiz::put_quiz))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_auth,
        ));

    Router::new()
        .nest("/api", public.merge(protected))
        .with_state(state)
}
