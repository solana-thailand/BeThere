//! Devnet sandbox behind `/sandbox` (`.plans/042` 0.4).
//!
//! A visitor with no login creates a two-minute test event, gets test USDC
//! from a faucet into a browser burner wallet, deposits, is checked in by the
//! sandbox organizer key, and claims the refund after the event ends.
//!
//! Off unless DEV_MODE (staging or local), the escrow cluster is devnet, and
//! both `SANDBOX_ORGANIZER_KEY` and `SANDBOX_FAUCET_KEY` parse. Neither key may
//! ever be set on prod. Nothing about a visitor is stored: the escrow lives on
//! devnet, and the only D1 writes are the 24 h quota keys.

pub mod config;
pub mod keys;
pub mod quota;
pub mod send;
pub mod tx;
