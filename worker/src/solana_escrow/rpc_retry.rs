//! When a failed Solana RPC call is worth one more try.
//!
//! Helius limits requests per second. A burst of wallet scans on cold isolates
//! can get `429` on `getLatestBlockhash` (.issues/156), and without a retry
//! the attendee sees a failed deposit. The same holds for a brief `5xx`.
//! Any other status is an answer that repeating will not change.

/// Shortest pause before the single retry. Helius counts per second, so a
/// retry at least half a second later lands in a fresh window more often than
/// not.
pub const RETRY_BASE_DELAY_MS: u64 = 500;

/// Upper bound on the extra pause added to [`RETRY_BASE_DELAY_MS`].
/// Requests rejected together would otherwise retry together and collide
/// again.
pub const RETRY_JITTER_MS: u64 = 500;

/// Whether an HTTP status from the RPC is transient: rate limited or a
/// server-side failure.
pub fn is_transient_status(status: u16) -> bool {
    matches!(status, 429 | 500..=599)
}

/// The pause before the retry, spread by the millisecond at which the first
/// attempt failed. Requests rejected in the same burst fail at different
/// milliseconds, so their retries fan out across the jitter window. No RNG:
/// `solana_escrow` is a monetary module (`tests/deterministic_monetary_code.rs`).
/// A non-finite or negative time falls back to the base delay.
pub fn retry_delay_ms(failed_at_ms: f64) -> u64 {
    let offset = match failed_at_ms.is_finite() && failed_at_ms >= 0.0 {
        true => failed_at_ms as u64 % (RETRY_JITTER_MS + 1),
        false => 0,
    };
    RETRY_BASE_DELAY_MS + offset
}
