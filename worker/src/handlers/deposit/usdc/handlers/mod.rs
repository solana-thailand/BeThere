//! USDC deposit HTTP endpoints, one module per route.

mod confirm;
mod initiate;
mod status;
mod tx;
mod webhook;

pub use confirm::confirm_deposit_handler;
pub use initiate::deposit_usdc_handler;
pub use status::get_deposit_status_handler;
pub use tx::deposit_usdc_tx_handler;
pub use webhook::deposit_webhook_handler;
