//! The `Attendee` record and its check-in validation error.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::status::{CheckInStatus, ParticipationType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub api_id: String,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: CheckInStatus,
    pub participation_type: String,
    pub registration_date: Option<String>,
    pub phone: Option<String>,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    pub deposit_agreed: Option<String>,
    pub deposit_method: Option<String>,
    pub deposit_amount: Option<String>,
    pub deposit_tx_signature: Option<String>,
    pub deposit_verified: Option<String>,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub solana_address: Option<String>,
    pub qr_code_url: Option<String>,
    pub claim_token: Option<String>,
    pub claimed_at: Option<String>,
    pub nft_proof_url: Option<String>,
    // Section 6: Bank & Refund (Y–AD)
    pub bank_account: Option<String>,
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub refund_status: Option<String>,
    pub refund_link: Option<String>,
    pub send_email_status: Option<String>,
    pub row_index: usize,
}

/// Domain error for check-in validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckInError {
    /// Attendee is not in an approved state.
    NotApproved(String),
    /// Attendee was already checked in at the given time.
    AlreadyCheckedIn(String),
    /// Online/virtual attendees cannot check in on-site.
    OnlineAttendee,
}

impl fmt::Display for CheckInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotApproved(status) => write!(f, "attendee not approved: {status}"),
            Self::AlreadyCheckedIn(at) => write!(f, "already checked in at {at}"),
            Self::OnlineAttendee => write!(f, "online/virtual attendees cannot check in on-site"),
        }
    }
}

impl std::error::Error for CheckInError {}

impl Attendee {
    pub fn is_approved(&self) -> bool {
        matches!(
            self.approval_status,
            CheckInStatus::Approved | CheckInStatus::CheckedIn
        )
    }

    pub fn is_checked_in(&self) -> bool {
        self.checked_in_at.is_some()
    }

    /// Check if attendee's participation type is "In-Person".
    /// Online attendees should not be checked in at the physical event.
    ///
    /// Delegates to [`ParticipationType::parse`]; defaults to in-person when
    /// `participation_type` is empty (legacy events predate this field).
    pub fn is_in_person(&self) -> bool {
        self.participation_type_enum() == ParticipationType::InPerson
    }

    /// Canonical typed participation type (see [`ParticipationType`]).
    pub fn participation_type_enum(&self) -> ParticipationType {
        ParticipationType::parse(&self.participation_type)
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.email
        } else {
            &self.name
        }
    }

    /// Validate whether this attendee can be checked in on-site.
    ///
    /// Checks (in order):
    /// 1. Not already checked in
    /// 2. Approval status is Approved or CheckedIn
    /// 3. Participation type is In-Person
    pub fn can_check_in(&self) -> Result<(), CheckInError> {
        if self.is_checked_in() {
            return Err(CheckInError::AlreadyCheckedIn(
                self.checked_in_at.clone().unwrap_or_default(),
            ));
        }
        if !self.is_approved() {
            return Err(CheckInError::NotApproved(self.approval_status.to_string()));
        }
        if !self.is_in_person() {
            return Err(CheckInError::OnlineAttendee);
        }
        Ok(())
    }

    /// Validate whether this attendee can be checked in *virtually* (online
    /// track: the hybrid `online=true` scan, or a self-serve quest completion).
    ///
    /// Same gates as [`Attendee::can_check_in`] minus the in-person requirement,
    /// which is inverted for this path. Kept here rather than inline in each
    /// handler because the approval gate is what makes a check-in the only way
    /// an unapproved registrant is kept out of the claim flow — the claim path
    /// does not re-check approval, it relies on `checked_in_at` being reachable
    /// only through a gate like this one. A handler that flips `checked_in_at`
    /// without calling this silently opens that door.
    ///
    /// Checks (in order):
    /// 1. Not already checked in
    /// 2. Approval status is Approved or CheckedIn
    ///
    /// The caller is still responsible for the event-level gate (the event must
    /// have an online track); that is not attendee state.
    pub fn can_check_in_virtually(&self) -> Result<(), CheckInError> {
        if self.is_checked_in() {
            return Err(CheckInError::AlreadyCheckedIn(
                self.checked_in_at.clone().unwrap_or_default(),
            ));
        }
        if !self.is_approved() {
            return Err(CheckInError::NotApproved(self.approval_status.to_string()));
        }
        Ok(())
    }

    /// Is deposit verified (USDC confirmed on-chain or THB slip approved)?
    /// The `deposit_verified` field is a string from Google Sheets ("true"/"false"
    /// or a timestamp). Non-empty means verified.
    pub fn has_verified_deposit(&self) -> bool {
        self.deposit_verified
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty())
    }

    /// Is this attendee eligible for refund?
    /// Requires a verified deposit and not already refunded.
    pub fn is_refund_eligible(&self) -> bool {
        self.has_verified_deposit() && self.refund_status.as_deref() != Some("refunded")
    }
}

// ---------------------------------------------------------------------------
// Dynamic column mapping
// ---------------------------------------------------------------------------
