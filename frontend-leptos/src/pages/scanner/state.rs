//! Check-in state types.

use crate::api::{AttendeeData, CheckInData};

// ===== State Types =====

/// Current state of the check-in flow.
#[derive(Clone)]
pub(super) enum CheckInState {
    /// No active check-in.
    Idle,
    /// Looking up an attendee by ID.
    LookingUp,
    /// Attendee found, approved, and in-person — ready to confirm.
    Found(Box<AttendeeData>),
    /// Attendee is already checked in.
    AlreadyCheckedIn(Box<AttendeeData>),
    /// Attendee is not approved (status ≠ "Approved").
    NotApproved(Box<AttendeeData>),
    /// Attendee is not In-Person (e.g. Online/Virtual).
    NotInPerson(Box<AttendeeData>),
    /// Attendee not found by api_id.
    NotFound,
    /// Performing the check-in POST request.
    CheckingIn { name: String, _id: String },
    /// Check-in succeeded.
    Success(Box<CheckInData>),
    /// An error occurred at any step.
    Error,
    // --- Escrow on-chain check-in states (after off-chain Success) ---
    /// Organizer choosing which wallet to connect for on-chain check-in.
    EscrowChooseWallet {
        check_in_data: Box<CheckInData>,
        attendee_id: String,
        event_id: String,
    },
    /// Organizer wallet connected, ready to sign on-chain TX.
    EscrowWalletConnected {
        check_in_data: Box<CheckInData>,
        attendee_id: String,
        event_id: String,
        wallet_name: String,
        public_key: String,
    },
    /// On-chain TX being signed/sent.
    EscrowSigning { wallet_name: String },
    /// On-chain check-in confirmed.
    EscrowConfirmed {
        check_in_data: Box<CheckInData>,
        signature: String,
    },
    /// On-chain check-in failed.
    EscrowError {
        check_in_data: Box<CheckInData>,
        message: String,
    },
    // --- Walk-in registration states ---
    /// Walk-in registration form is displayed.
    WalkinForm,
    /// Walk-in registration request in progress.
    WalkinRegistering,
    /// Walk-in registration succeeded — show claim QR to attendee.
    WalkinSuccess { claim_url: String, name: String },
    /// Walk-in capacity reached — show warning dialog with override option.
    WalkinCapacityWarning {
        pending_name: String,
        pending_email: String,
        pending_phone: Option<String>,
    },
}
