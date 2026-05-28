use event_checkin_domain::models::event::EscrowStatus;

// ---------------------------------------------------------------------------
// POST /api/escrow/init — Combined ATA + CreateEvent in one TX
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/init.
#[derive(Debug, serde::Deserialize)]
pub struct InitEscrowTxRequest {
    /// Event ID (slug or KV key).
    pub event_id: String,
}

/// Response with the combined init escrow transaction.
#[derive(Debug, serde::Serialize)]
pub struct InitEscrowTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
    /// Derived EventEscrow PDA address (base58).
    pub escrow_address: String,
    /// Derived vault ATA address (base58).
    pub vault_address: String,
    /// The on-chain event ID used for PDA derivation.
    pub on_chain_event_id: u64,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/refund — Build refund TX for attendee
// ---------------------------------------------------------------------------

/// Request body for building a refund transaction.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct RefundTxRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response with the serialized refund transaction.
#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
pub struct RefundTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Request for combined refund + close_deposit transaction.
#[derive(serde::Deserialize)]
pub struct RefundAndCloseTxRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response with the serialized combined refund+close_deposit transaction.
#[derive(Debug, serde::Serialize)]
pub struct RefundAndCloseTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/mark-checked-in
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct MarkCheckedInTxRequest {
    /// Event ID (slug or KV key).
    pub event_id: String,
    /// Attendee API ID from Google Sheets (used to look up wallet from deposit).
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    /// If not provided, looked up from the deposit record.
    #[serde(default)]
    pub attendee_wallet: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct MarkCheckedInTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/backfill-wallets
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/backfill-wallets.
#[derive(Debug, serde::Deserialize)]
pub struct BackfillWalletsRequest {
    /// Event ID to backfill. If omitted, backfills all events.
    #[serde(default)]
    pub event_id: Option<String>,
}

/// Response for POST /api/escrow/backfill-wallets.
#[derive(Debug, serde::Serialize)]
pub struct BackfillWalletsResponse {
    /// Total deposits scanned.
    pub scanned: usize,
    /// Deposits missing wallet_address.
    pub missing_wallet: usize,
    /// Successfully backfilled.
    pub backfilled: usize,
    /// Failed to resolve (TX expired, RPC error, etc.).
    pub failed: usize,
    /// Already had wallet_address.
    pub already_present: usize,
    /// Per-attendee details.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<BackfillDetail>,
}

#[derive(Debug, serde::Serialize)]
pub struct BackfillDetail {
    pub attendee_id: String,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/deactivate-event
// ---------------------------------------------------------------------------

/// Request body for deactivate_event TX builder.
#[derive(serde::Deserialize)]
pub struct DeactivateEventTxRequest {
    pub event_id: String,
}

/// Response body for deactivate_event TX builder.
#[derive(serde::Serialize)]
pub struct DeactivateEventTxResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/close-event
// ---------------------------------------------------------------------------

/// Request body for close_event TX builder.
#[derive(serde::Deserialize)]
pub struct CloseEventTxRequest {
    pub event_id: String,
}

/// Response body for close_event TX builder.
#[derive(serde::Serialize)]
pub struct CloseEventTxResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/claim-forfeited
// ---------------------------------------------------------------------------

/// Request body for claim_forfeited TX builder.
#[derive(serde::Deserialize)]
pub struct ClaimForfeitedTxRequest {
    pub event_id: String,
}

/// Response body for claim_forfeited TX builder.
#[derive(serde::Serialize)]
pub struct ClaimForfeitedTxResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/close-deposit
// ---------------------------------------------------------------------------

/// Request body for building a close_deposit transaction.
#[derive(Debug, serde::Deserialize)]
pub struct CloseDepositTxRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response with the serialized close_deposit transaction.
#[derive(Debug, serde::Serialize)]
pub struct CloseDepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

// ---------------------------------------------------------------------------
// GET /api/escrow/refund-queue
// ---------------------------------------------------------------------------

/// USDC deposit queue item for cancellation workflow.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsdcQueueItem {
    pub attendee_id: String,
    pub wallet_address: Option<String>,
    pub amount: u64,
    pub deposited_at: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/confirm-init
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/confirm-init.
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmEscrowInitRequest {
    /// Event ID (KV key).
    pub event_id: String,
}

/// Response for escrow init confirmation.
#[derive(Debug, serde::Serialize)]
pub struct ConfirmEscrowInitResponse {
    /// Derived escrow PDA address (base58).
    pub escrow_address: String,
    /// On-chain event ID used for PDA derivation.
    pub on_chain_event_id: u64,
    /// Confirmed escrow status.
    pub escrow_status: EscrowStatus,
}

// ---------------------------------------------------------------------------
// GET /api/escrow/health
// ---------------------------------------------------------------------------

/// Response for the escrow health check endpoint.
#[derive(Debug, serde::Serialize)]
pub struct EscrowHealthResponse {
    /// Event ID.
    pub event_id: String,
    /// Server-side escrow status from KV.
    pub kv_escrow_status: String,
    /// Server-side escrow address from KV.
    pub kv_escrow_address: String,
    /// Server-side on-chain event ID from KV.
    pub kv_on_chain_event_id: u64,
    /// Server-side organizer wallet from KV.
    pub kv_organizer_wallet: String,
    /// Whether the escrow account exists on-chain.
    pub on_chain_exists: bool,
    /// Derived escrow PDA address (if derivable).
    pub derived_escrow_address: Option<String>,
    /// Whether KV and on-chain states are consistent.
    pub consistent: bool,
    /// Human-readable diagnosis.
    pub diagnosis: String,
}

// ---------------------------------------------------------------------------
// POST /api/escrow/rollover-deposit
// ---------------------------------------------------------------------------

/// Request body for rollover deposit transaction.
#[derive(Debug, serde::Deserialize)]
pub struct RolloverDepositTxRequest {
    /// Source event ID (past event with checked-in deposit).
    pub source_event_id: String,
    /// Target event ID (new event to roll deposit into).
    pub target_event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// Attendee's wallet address (base58).
    pub wallet_address: String,
}

/// Response for rollover deposit transaction.
#[derive(Debug, serde::Serialize)]
pub struct RolloverDepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction: String,
    /// Human-readable message.
    pub message: String,
}
