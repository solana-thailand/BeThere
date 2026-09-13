//! Walk-in attendee types.

use serde::{Deserialize, Serialize};

/// Walk-in attendee registered on-the-spot by staff.
///
/// Stored in KV under `walkin:{event_id}:{email_lower}` with a 90-day TTL.
/// A reverse mapping `claim_walkin:{claim_token}` → `{event_id}:{email_lower}`
/// enables claim-token lookup without scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkinAttendee {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub claim_token: String,
    /// ISO 8601 timestamp when the walk-in was registered.
    pub checked_in_at: String,
    /// Email of the staff member who registered the walk-in.
    pub checked_in_by: String,
    /// Solana wallet address — set later when the attendee claims their NFT.
    pub wallet_address: Option<String>,
    /// ISO 8601 timestamp when the NFT was claimed.
    pub claimed_at: Option<String>,
}
