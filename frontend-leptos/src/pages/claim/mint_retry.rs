//! Claim mint with automatic retry while the provider is still confirming.
//!
//! Issue 180: a Crossmint mainnet mint can take longer than the Worker's poll
//! budget. The Worker then answers 504 (`UpstreamPending`) and keeps the
//! provider id, so posting the same claim again resumes that mint instead of
//! starting another. The page retries a few times on its own and, if the mint
//! is still pending after that, says so instead of "Minting Failed".

use crate::api::{self, ClaimMintData};

/// HTTP status the Worker returns for a mint the provider has not confirmed yet.
pub const MINT_PENDING_STATUS: u16 = 504;
/// Automatic retries after the first attempt while the mint is pending.
pub const MAX_PENDING_RETRIES: u32 = 4;
/// Pause between pending retries, milliseconds.
const PENDING_RETRY_DELAY_MS: u32 = 3_000;

/// Why a claim mint did not return a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintFailure {
    /// The mint failed; the message is shown to the attendee.
    Failed(String),
    /// The provider is still confirming; a later retry resumes the same mint.
    StillPending,
}

/// Classify a failed `POST /api/claim/{token}` by its HTTP status.
pub fn classify(status: u16, message: String) -> MintFailure {
    match status {
        MINT_PENDING_STATUS => MintFailure::StillPending,
        _ => MintFailure::Failed(message),
    }
}

/// Whether to retry on our own after `retries_done` automatic retries.
pub fn should_retry(failure: &MintFailure, retries_done: u32) -> bool {
    matches!(failure, MintFailure::StillPending) && retries_done < MAX_PENDING_RETRIES
}

/// Post the claim, retrying while the Worker reports the mint as pending.
pub(super) async fn post_claim_until_settled(
    token: &str,
    wallet: Option<&str>,
    use_linked: bool,
) -> Result<ClaimMintData, MintFailure> {
    let mut retries_done = 0;
    loop {
        let error = match api::post_claim(token, wallet, use_linked).await {
            Ok(data) => return Ok(data),
            Err(error) => error,
        };
        let failure = classify(error.status, error.to_string());
        if !should_retry(&failure, retries_done) {
            log::error!("[claim] mint failed: {error}");
            return Err(failure);
        }
        retries_done += 1;
        log::info!("[claim] mint still pending; retry {retries_done}/{MAX_PENDING_RETRIES}");
        gloo_timers::future::TimeoutFuture::new(PENDING_RETRY_DELAY_MS).await;
    }
}
