pub mod adventure;
pub mod attendee;
pub mod auth;
pub mod checkin;
pub mod claim;
pub mod contacts;
pub mod deposit;
pub mod escrow_index;
pub mod events;
pub mod ext;
pub mod health;
pub mod metadata;
pub mod orgs;
pub mod public_event;
pub mod qr;
pub mod quiz;
pub mod register;
pub mod waitlist;
pub mod walkin;

use crate::state::AppState;
use axum::{
    Router, middleware,
    routing::{delete, get, patch, post, put},
};

pub fn routes(state: AppState) -> Router<()> {
    // Cache-Control layers — applied per route group via sub-routers.
    // Public event list: 60s cache (changes infrequently)
    let public_events_list = Router::new()
        .route("/public/events", get(public_event::list_public_events))
        .layer(middleware::from_fn(
            crate::middleware::cache_public_60_layer,
        ));

    // Public event detail: 120s cache (individual events rarely change)
    let public_events_detail = Router::new()
        .route("/public/event/{slug}", get(public_event::get_public_event))
        .layer(middleware::from_fn(
            crate::middleware::cache_public_120_layer,
        ));

    // Health check: no-cache (should always be fresh)
    let health_routes = Router::new()
        .route("/health", get(health::health_check))
        .layer(middleware::from_fn(crate::middleware::cache_no_cache_layer));

    // Auth routes: no-store (sensitive data)
    let auth_routes = Router::new()
        .route("/auth/url", get(auth::auth_url))
        .route("/auth/callback", get(auth::auth_callback))
        .route("/auth/logout", post(auth::auth_logout))
        .layer(middleware::from_fn(crate::middleware::cache_no_store_layer));

    // Public routes — no auth middleware required.
    // Rate limiting is handled by Cloudflare Rate Limiting Rules (dashboard config).
    let public = Router::new()
        .merge(health_routes)
        // NFT metadata (public — wallets/explorers fetch these)
        .route("/metadata/{event_id}", get(metadata::get_metadata))
        .route("/badge.svg", get(metadata::get_badge_svg))
        .route("/badge-hd.svg", get(metadata::get_badge_hd_svg))
        .merge(public_events_list)
        .merge(public_events_detail)
        .merge(auth_routes)
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
        // Public ticket view (no auth — attendees view their QR slip)
        .route("/public/ticket/{id}", get(attendee::get_public_ticket))
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
        // Escrow refund + close deposit (combined atomic TX — attendee claims refund and reclaims rent)
        .route("/escrow/refund", post(deposit::refund_and_close_tx_handler))
        // Escrow close deposit (public — attendee closes deposit PDA to reclaim rent)
        .route(
            "/escrow/close-deposit",
            post(deposit::close_deposit_tx_handler),
        )
        // On-chain event indexing webhook (public — called by Helius)
        .route(
            "/escrow/onchain-webhook",
            post(escrow_index::onchain_webhook_handler),
        )
        // R2 object serving — separate routes per prefix to avoid SPA fallback shadowing
        // the {key:path} wildcard. Each route maps to a specific R2 prefix.
        .route(
            "/storage/slips/{event_id}/{attendee_id}",
            get(crate::storage::serve_slip),
        )
        .route(
            "/storage/refunds/{event_id}/{attendee_id}",
            get(crate::storage::serve_refund),
        )
        .route(
            "/storage/badges/{event_id}",
            get(crate::storage::serve_badge),
        );

    // Attendee-authenticated routes — require JWT identity but NOT staff status.
    // Used for endpoints where a verified email is enough (registration, my-registration).
    let attendee_authed = Router::new()
        // Auth route that requires session (reads Claims from middleware)
        // Works for both staff and non-staff — only requires valid JWT.
        .route("/auth/me", get(auth::auth_me))
        // Self-registration (requires verified email from JWT)
        .route("/public/register", post(register::register_attendee))
        // Attendee's own registration lookup (requires verified email from JWT)
        .route("/my-registration/{slug}", get(register::my_registration))
        // All registrations for the signed-in user across all events
        .route("/my-registrations", get(register::my_registrations))
        // THB slip upload (requires verified email — attendee uploads their own slip)
        .route(
            "/deposit/thb/upload",
            post(deposit::upload_thb_slip_handler),
        )
        // Hold deposit as rolling credit (attendee chooses credit over refund)
        .route("/deposit/hold", post(deposit::hold_deposit_handler))
        // Check deposit credit balance
        .route(
            "/deposit/credit-balance",
            get(deposit::credit_balance_handler),
        )
        // Roll deposit to next event (attendee-authed — attendee signs the TX)
        .route(
            "/escrow/rollover-deposit",
            post(deposit::rollover_deposit_tx_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_identity,
        ));

    // Protected routes — require staff auth
    let protected = Router::new()
        .route("/attendees", get(attendee::list_attendees))
        .route(
            "/attendee/{id}",
            get(attendee::get_attendee).delete(attendee::delete_attendee),
        )
        .route("/checkin/{id}", post(checkin::check_in))
        .route("/attendee/{id}/undo-checkin", post(checkin::undo_check_in))
        .route("/generate-qrs", post(qr::generate_qrs))
        // Flush server-side caches (attendee list + column mapping)
        .route("/admin/flush-cache", post(attendee::flush_cache))
        // Walk-in attendee registration (protected — staff registers on-the-spot)
        .route("/walkin/register", post(walkin::register_walkin))
        // Walk-in attendee management
        .route("/walkin/list", get(walkin::list_walkin_handler))
        .route("/walkin/export", get(walkin::walkin_export_csv_handler))
        .route("/walkin/sync", post(walkin::walkin_sync_handler))
        // Admin quiz management (protected — organizer sets questions)
        .route(
            "/admin/quiz",
            get(quiz::get_admin_quiz).post(quiz::put_quiz),
        )
        // Individual quiz question CRUD (Issue 034 Phase 2)
        .route("/admin/quiz/questions", post(quiz::add_quiz_question))
        .route(
            "/admin/quiz/questions/{id}",
            put(quiz::update_quiz_question).delete(quiz::delete_quiz_question),
        )
        .route(
            "/admin/quiz/questions/{id}/toggle",
            patch(quiz::toggle_quiz_question),
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
        .route("/events/{id}/delete", delete(events::hard_delete_event))
        .route("/events/{id}/audit", get(events::get_event_audit))
        .route("/audit/global", get(events::get_global_audit))
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
        .route("/refund/refunded", get(deposit::refunded_list_handler))
        .route(
            "/refund/mark/{attendee_id}",
            post(deposit::mark_refund_handler),
        )
        .route(
            "/refund/manual/{attendee_id}",
            post(deposit::mark_manual_refund_handler),
        )
        // Escrow management (protected — organizer initializes on-chain escrow)
        .route("/escrow/init", post(deposit::init_escrow_tx_handler))
        .route(
            "/escrow/confirm-init",
            post(deposit::confirm_escrow_init_handler),
        )
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
        // Cancellation workflow (admin — batch refunds + status)
        .route("/refund/batch-thb", post(deposit::batch_thb_refund_handler))
        .route(
            "/escrow/refund-queue",
            get(deposit::usdc_refund_queue_handler),
        )
        .route("/escrow/cancel-status", get(deposit::cancel_status_handler))
        .route("/escrow/health", get(deposit::escrow_health_handler))
        // Contacts management (protected — organizer manages master contacts)
        .route("/contacts", get(contacts::list_contacts_handler))
        .route("/contacts/events", get(contacts::list_events_tab_handler))
        .route("/contacts/stats", get(contacts::contacts_stats_handler))
        .route("/contacts/sync", post(contacts::sync_contacts_handler))
        // Organization management (protected — super admin CRUD)
        .route("/orgs", get(orgs::list_orgs).post(orgs::create_org))
        .route(
            "/orgs/{id}",
            get(orgs::get_org)
                .put(orgs::update_org)
                .delete(orgs::delete_org),
        )
        // On-chain event indexing (protected — manual sync + query)
        .route("/escrow/sync", post(escrow_index::escrow_sync_handler))
        .route(
            "/escrow/events/{event_id}",
            get(escrow_index::get_onchain_events_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_auth,
        ));

    Router::new()
        .nest("/api", public.merge(attendee_authed).merge(protected))
        .with_state(state)
}
