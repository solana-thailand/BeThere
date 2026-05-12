pub mod adventure;
pub mod attendee;
pub mod auth;
pub mod checkin;
pub mod claim;
pub mod deposit;
pub mod events;
pub mod ext;
pub mod health;
pub mod metadata;
pub mod public_event;
pub mod qr;
pub mod quiz;
pub mod register;
pub mod waitlist;
pub mod walkin;

use crate::state::AppState;
use axum::{
    Router, middleware,
    routing::{get, post},
};

pub fn routes(state: AppState) -> Router<()> {
    // Public routes — no auth middleware required.
    // Rate limiting is handled by Cloudflare Rate Limiting Rules (dashboard config).
    let public = Router::new()
        .route("/health", get(health::health_check))
        // NFT metadata (public — wallets/explorers fetch these)
        .route("/metadata/{event_id}", get(metadata::get_metadata))
        .route("/badge.svg", get(metadata::get_badge_svg))
        .route("/badge-hd.svg", get(metadata::get_badge_hd_svg))
        // Public event list (no auth — landing page)
        .route("/public/events", get(public_event::list_public_events))
        // Public event details (no auth)
        .route("/public/event/{slug}", get(public_event::get_public_event))
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
        // Adventure routes (public — attendees play adventure game)
        .route(
            "/adventure/{token}/status",
            get(adventure::get_adventure_status),
        )
        .route(
            "/adventure/{token}/save",
            post(adventure::save_adventure_progress),
        )
        // Waitlist signup (public)
        .route("/waitlist", post(waitlist::join_waitlist))
        // Self-registration (public — attendees register from public event page)
        .route("/public/register", post(register::register_attendee))
        // Deposit routes (public — attendee checks/initiates deposit)
        .route(
            "/deposit/status/{attendee_id}",
            get(deposit::get_deposit_status_handler),
        )
        .route("/deposit/usdc", post(deposit::deposit_usdc_handler))
        .route("/deposit/usdc/tx", get(deposit::deposit_usdc_tx_handler))
        .route(
            "/deposit/usdc/confirm",
            get(deposit::confirm_deposit_handler),
        )
        .route(
            "/deposit/usdc/webhook",
            post(deposit::deposit_webhook_handler),
        )
        .route(
            "/deposit/thb/upload",
            post(deposit::upload_thb_slip_handler),
        )
        // Escrow refund + close deposit (combined atomic TX — attendee claims refund and reclaims rent)
        .route("/escrow/refund", post(deposit::refund_and_close_tx_handler))
        // Escrow close deposit (public — attendee closes deposit PDA to reclaim rent)
        .route(
            "/escrow/close-deposit",
            post(deposit::close_deposit_tx_handler),
        );

    // Protected routes — require staff auth
    let protected = Router::new()
        // Auth route that requires session (reads Claims from middleware)
        .route("/auth/me", get(auth::auth_me))
        .route("/attendees", get(attendee::list_attendees))
        .route("/attendee/{id}", get(attendee::get_attendee))
        .route("/checkin/{id}", post(checkin::check_in))
        .route("/attendee/{id}/undo-checkin", post(checkin::undo_check_in))
        .route("/generate-qrs", post(qr::generate_qrs))
        // Walk-in attendee registration (protected — staff registers on-the-spot)
        .route("/walkin/register", post(walkin::register_walkin))
        // Admin quiz management (protected — organizer sets questions)
        .route(
            "/admin/quiz",
            get(quiz::get_admin_quiz).post(quiz::put_quiz),
        )
        // Admin adventure management (protected — organizer configures adventure)
        .route(
            "/admin/adventure",
            get(adventure::get_admin_adventure).put(adventure::put_admin_adventure),
        )
        // Event management (protected — admin/organizer CRUD)
        .route(
            "/events",
            get(events::list_events).post(events::create_event),
        )
        // Migrate and seed routes MUST come before /events/{id} to avoid path conflicts
        .route("/events/migrate", post(events::migrate_quiz))
        .route("/events/seed", post(events::seed_event))
        .route(
            "/events/{id}",
            get(events::get_event)
                .put(events::update_event)
                .delete(events::archive_event),
        )
        .route("/events/{id}/restore", post(events::restore_event))
        // Admin deposit management (protected — organizer verifies slips, manages refunds)
        .route(
            "/deposit/thb/verify",
            post(deposit::verify_thb_slip_handler),
        )
        .route(
            "/deposit/thb/pending",
            get(deposit::pending_thb_slips_handler),
        )
        .route("/refund/queue", get(deposit::refund_queue_handler))
        .route(
            "/refund/mark/{attendee_id}",
            post(deposit::mark_refund_handler),
        )
        // Escrow management (protected — organizer initializes on-chain escrow)
        .route("/escrow/init", post(deposit::init_escrow_tx_handler))
        .route(
            "/escrow/mark-checked-in",
            post(deposit::mark_checked_in_tx_handler),
        )
        .route(
            "/escrow/backfill-wallets",
            post(deposit::backfill_wallets_handler),
        )
        .route(
            "/escrow/deactivate-event",
            post(deposit::deactivate_event_tx_handler),
        )
        .route("/escrow/close-event", post(deposit::close_event_tx_handler))
        .route(
            "/escrow/claim-forfeited",
            post(deposit::claim_forfeited_tx_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_auth,
        ));

    Router::new()
        .nest("/api", public.merge(protected))
        .with_state(state)
}
