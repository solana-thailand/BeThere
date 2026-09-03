//! Route params and claim page states.

use leptos::prelude::*;
use leptos_router::params::Params;

use crate::api::{
    AdventureStatusType, ClaimLookupData, ClaimMintData, QuizQuestionsData,
    QuizSubmitData,
};


// ---------------------------------------------------------------------------
// Route params
// ---------------------------------------------------------------------------

/// Route parameters for `/claim/:token`.
/// `token` is the UUID v7 claim token generated at check-in.
#[derive(Params, PartialEq, Clone)]
pub(super) struct ClaimParams {
    pub(super) token: Option<String>,
}

// ---------------------------------------------------------------------------
// Claim page states
// ---------------------------------------------------------------------------

/// Top-level state machine for the claim page flow.
#[derive(Clone, Debug)]
pub(super) enum ClaimState {
    /// Loading claim info from backend.
    Loading,
    /// Claim token not found or lookup failed.
    NotFound(String),
    /// Attendee found, has not yet claimed. Ready for wallet input.
    Ready(ClaimLookupData),
    /// Attendee found but NFT minting is not configured yet.
    NftComingSoon(ClaimLookupData),
    /// Quiz required — attendee must complete quiz before claiming.
    /// Holds claim data + fetched quiz questions.
    Quiz(ClaimLookupData, QuizQuestionsData),
    /// Quiz submitted — showing results. If passed, transition to Ready.
    QuizSubmitted(ClaimLookupData, QuizQuestionsData, QuizSubmitData),
    /// Adventure required — attendee must complete adventure before claiming.
    /// Holds claim data + adventure status.
    Adventure(ClaimLookupData, AdventureStatusType),
    /// Minting in progress (POST /api/claim/{token} sent).
    Minting(ClaimLookupData),
    /// NFT minted successfully.
    Success(ClaimMintData),
    /// Already claimed previously.
    AlreadyClaimed(ClaimLookupData),
    /// Error during minting.
    MintError(ClaimLookupData, String),
}
