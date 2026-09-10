//! Handler closures for the deposit page.
//!
//! Extracted from the page component to keep each file under 1024 lines.
//! Each function creates a self-contained handler closure that captures the
//! reactive signals it needs.

mod close;
mod polling;
mod qr;
mod refund;
mod send;
mod slip;
mod wallet;

pub use close::{make_close_deposit, make_close_deposit_connect_wallet};
pub use polling::{
    PollConfig, PollOutcome, make_poll_confirmation, make_qr_poll_confirmation,
    poll_deposit_confirmation,
};
pub use qr::make_pay_usdc_qr;
pub use refund::{make_claim_refund, make_refund_connect_wallet};
pub use send::make_send_deposit;
pub use slip::{make_copy_url, make_upload_slip};
pub use wallet::make_connect_wallet;
