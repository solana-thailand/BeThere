//! Devnet sandbox behind `/sandbox` (`.plans/042` 0.4).
//!
//! A visitor with no login creates a two-minute test event, gets test USDC
//! from a faucet into a browser burner wallet, deposits, is checked in by the
//! sandbox organizer key, and claims the refund after the event ends.
//!
//! Off unless `SANDBOX_ENABLED = "1"` (staging, and prod since the owner's
//! go of 2026-10-11; it was DEV_MODE before), the escrow cluster is devnet
//! (so a prod that moves its escrow to mainnet turns it off by itself), and
//! both `SANDBOX_ORGANIZER_KEY` and `SANDBOX_FAUCET_KEY` parse. The keys are
//! devnet-only keys with no real value. Nothing about a visitor is stored:
//! the escrow lives on devnet, and the only D1 writes are the 24 h quota keys.

pub mod config;
pub mod keys;
pub mod quota;
pub mod send;
pub mod tx;
