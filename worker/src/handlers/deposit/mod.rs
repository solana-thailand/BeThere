//! Deposit/refund API handlers for dual-track payment (USDC + THB).
//!
//! Issue 010 Phase 5 — USDC on-chain TX building + Worker Deposit/Refund API.
//!
//! Endpoints:
//!   GET  /api/deposit/status/{attendee_id}  — check deposit status
//!   POST /api/deposit/usdc                  — initiate USDC deposit (Solana Pay URL)
//!   GET  /api/deposit/usdc/tx               — Solana Pay TX callback (wallet fetches TX)
//!   POST /api/deposit/thb/upload            — record THB slip upload
//!   POST /api/deposit/thb/admin-upload      — admin records slip on behalf of attendee
//!   POST /api/deposit/thb/verify            — admin verifies/rejects slip
//!   GET  /api/deposit/thb/pending           — list unverified slips (admin)
//!   POST /api/refund/mark/{attendee_id}     — mark THB refund as done (admin)
//!   GET  /api/refund/queue                  — refund queue (THB pending)

pub mod escrow;
pub mod thb;
pub mod usdc;

// Re-export all public handlers so that `handlers/mod.rs` can reference them
// as `deposit::<handler>` without knowing the submodule layout.
pub use escrow::{
    backfill_wallets_handler, cancel_status_handler, claim_forfeited_tx_handler,
    close_deposit_tx_handler, close_event_tx_handler, confirm_escrow_init_handler,
    deactivate_event_tx_handler, escrow_health_handler, init_escrow_tx_handler,
    mark_checked_in_tx_handler, refund_and_close_tx_handler, rollover_deposit_tx_handler,
    usdc_refund_queue_handler,
};
pub use thb::{
    admin_apply_credit_handler, admin_hold_deposit_handler, admin_upload_thb_slip_handler,
    batch_thb_refund_handler, clear_credit_refund_request_handler, comp_thb_deposit_handler,
    credit_balance_handler, credit_liability_handler, credit_refund_request_status_handler,
    credit_refund_requests_handler, credit_used_handler, held_list_handler, hold_deposit_handler,
    mark_manual_refund_handler, mark_refund_handler, pending_thb_slips_handler,
    refund_queue_handler, refunded_list_handler, request_credit_refund_handler,
    upload_thb_slip_handler, verify_thb_slip_handler,
};
pub use usdc::{
    confirm_deposit_handler, deposit_usdc_handler, deposit_usdc_tx_handler,
    deposit_webhook_handler, get_deposit_status_handler,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Derive the u64 on-chain event id that seeds the escrow PDA.
/// The hash lives in `event_checkin_domain::onchain` and must never change.
pub(crate) fn derive_on_chain_event_id(event_id: &str) -> u64 {
    event_checkin_domain::onchain::on_chain_event_id(event_id)
}
