//! RPC request/response types for the EventDurableObject.

use serde::{Deserialize, Serialize};

/// RPC request enum — Worker sends these as JSON in the DO fetch body.
#[derive(Deserialize, Serialize)]
#[serde(tag = "action")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum DoRequest {
    // ── Phase 1: Claim lock operations ──
    #[serde(rename = "acquire_claim_lock")]
    AcquireClaimLock {
        lock_id: String,
        event_id: String,
        token: String,
        wallet: String,
        expires_at: String,
    },
    #[serde(rename = "finalize_claim_lock")]
    FinalizeClaimLock {
        event_id: String,
        token: String,
        asset_id: String,
        signature: String,
        claimed_at: String,
    },
    #[serde(rename = "release_claim_lock")]
    ReleaseClaimLock { event_id: String, token: String },

    // ── Phase 2: Check-in & claim operations ──
    #[serde(rename = "check_in")]
    CheckIn {
        attendee_id: String,
        event_id: String,
        checked_in_at: String,
        checked_in_by: String,
        claim_token: String,
    },
    #[serde(rename = "undo_check_in")]
    UndoCheckIn {
        attendee_id: String,
        event_id: String,
    },
    #[serde(rename = "claim_attendee")]
    ClaimAttendee {
        event_id: String,
        claim_token: String,
        claimed_at: String,
        claim_asset_id: String,
        claim_signature: String,
    },
    #[serde(rename = "upsert_attendee")]
    UpsertAttendee {
        id: String,
        event_id: String,
        email: String,
        name: String,
        approval_status: String,
        participation_type: String,
        contact_channel: String,
        contact_handle: String,
    },
}

/// RPC response — DO returns this as JSON.
#[derive(Serialize)]
pub(super) struct DoResponse {
    pub(super) success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl DoResponse {
    pub(super) fn ok() -> Self {
        Self {
            success: true,
            error: None,
        }
    }
    pub(super) fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(msg.into()),
        }
    }
}

/// Parameters for the `UpsertAttendee` DO RPC.
pub(crate) struct UpsertAttendeeParams<'a> {
    pub(crate) id: &'a str,
    pub(crate) event_id: &'a str,
    pub(crate) email: &'a str,
    pub(crate) name: &'a str,
    pub(crate) approval_status: &'a str,
    pub(crate) participation_type: &'a str,
    pub(crate) contact_channel: &'a str,
    pub(crate) contact_handle: &'a str,
}

// ---------------------------------------------------------------------------
// Deserialization helpers (Phase 1)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(super) struct ExistingLock {
    #[allow(dead_code)]
    pub(super) lock_id: String,
    pub(super) claimed_at: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ClaimLockSyncRow {
    pub(super) lock_id: String,
    pub(super) event_id: String,
    pub(super) token: String,
    pub(super) wallet: String,
    pub(super) started_at: String,
    pub(super) asset_id: Option<String>,
    pub(super) signature: Option<String>,
    pub(super) claimed_at: Option<String>,
    pub(super) expires_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Deserialization helpers (Phase 2)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(super) struct ExistingCheckIn {
    pub(super) checked_in_at: Option<String>,
    pub(super) claim_token: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct AttendeeSyncRow {
    pub(super) id: String,
    pub(super) event_id: String,
    pub(super) email: String,
    pub(super) name: String,
    pub(super) approval_status: String,
    pub(super) participation_type: String,
    pub(super) contact_channel: String,
    pub(super) contact_handle: String,
    pub(super) checked_in_at: Option<String>,
    pub(super) checked_in_by: Option<String>,
    pub(super) claim_token: Option<String>,
    pub(super) claimed_at: Option<String>,
    pub(super) claim_asset_id: Option<String>,
    pub(super) claim_signature: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct IdRow {
    pub(super) id: String,
}
