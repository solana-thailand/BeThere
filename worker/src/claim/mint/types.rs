//! Result types returned by the claim lookup and execute flows.

use event_checkin_domain::models::api::{EventConfig as ApiEventConfig, QuizStatus};
use event_checkin_domain::models::attendee::{Attendee, WalkinAttendee};

/// Result of a successful claim lookup (GET).
pub struct ClaimLookup {
    pub name: String,
    pub checked_in_at: String,
    pub claim_token: String,
    pub claimed: bool,
    pub claimed_at: Option<String>,
    pub nft_available: bool,
    /// Per-event pre-registered wallet (Sheet column P). When set, the claim is
    /// LOCKED to this address (often the on-chain depositor).
    pub locked_wallet: Option<String>,
    /// Masked display (e.g. `7Xk9…Qm3p`) of the attendee's profile-bound wallet
    /// (developer_profiles.wallet_address), surfaced only when there is no
    /// per-event lock. The full address is NEVER sent to the client — the mint
    /// resolves it server-side by email. Presence signals "one-tap linked-wallet
    /// claim is available"; the attendee may still opt to use a different wallet.
    pub linked_wallet_display: Option<String>,
    pub event: ApiEventConfig,
    pub quiz_status: QuizStatus,
    pub total_checked_in: usize,
    pub total_claimed: usize,
    pub api_id: String,
    pub event_id: String,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: u64,
    pub deposit_amount_thb: u64,
    pub participation_type: String,
    /// Transaction signature from the finalized claim lock KV (if available).
    pub claimed_signature: Option<String>,
    /// Asset ID from the finalized claim lock KV (if available).
    pub claimed_asset_id: Option<String>,
    /// Wallet address from the finalized claim lock KV (if available).
    pub claimed_wallet: Option<String>,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    pub cluster: Option<String>,
}

/// Result of a successful NFT claim (POST).
pub struct ClaimResult {
    pub name: String,
    pub asset_id: String,
    pub signature: String,
    pub wallet_address: String,
    pub claimed_at: String,
    pub cluster: String,
}

/// What the one D1 read by claim token said about the attendee (plan 028 W6).
pub(super) enum D1Claim {
    /// A row with this token, inside its replay window.
    Found(Box<Attendee>),
    /// D1 answered: no row, or the token is outside its replay window.
    Missing,
    /// No D1 binding, or the read failed. Callers retry through the old
    /// D1-first paths so a transient error never reads as "not found".
    Unavailable,
}

/// Event context plus the D1 attendee for one claim token, read once and
/// shared by the lookup (GET) and execute (POST) flows.
pub(super) struct ClaimContext {
    /// Caller-supplied event id if non-empty, otherwise the token row's.
    pub(super) event_id: Option<String>,
    pub(super) d1: D1Claim,
}

impl ClaimContext {
    /// The walk-in attendee for `event_id`, when the D1 row is a walk-in.
    pub(super) fn walkin(&self, event_id: &str) -> Option<WalkinAttendee> {
        match &self.d1 {
            D1Claim::Found(a) if a.participation_type == "walkin" => Some(WalkinAttendee {
                event_id: event_id.to_string(),
                email: a.email.clone(),
                name: a.name.clone(),
                phone: None,
                claim_token: a.claim_token.clone().unwrap_or_default(),
                checked_in_at: a.checked_in_at.clone().unwrap_or_default(),
                checked_in_by: a.checked_in_by.clone().unwrap_or_default(),
                wallet_address: None,
                claimed_at: a.claimed_at.clone(),
            }),
            _ => None,
        }
    }
}
