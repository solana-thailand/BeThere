use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckInStatus {
    PendingApproval,
    Approved,
    Invited,
    CheckedIn,
}

impl FromStr for CheckInStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "approved" => Self::Approved,
            "pending_approval" => Self::PendingApproval,
            "invited" => Self::Invited,
            "checked_in" | "checked in" => Self::CheckedIn,
            _ => Self::PendingApproval,
        })
    }
}

impl CheckInStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PendingApproval => "pending_approval",
            Self::Approved => "approved",
            Self::Invited => "invited",
            Self::CheckedIn => "checked_in",
        }
    }
}

impl std::fmt::Display for CheckInStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub api_id: String,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: CheckInStatus,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub qr_code_url: Option<String>,
    pub solana_address: Option<String>,
    pub participation_type: String,
    pub row_index: usize,
}

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
    /// Uses substring matching since the sheet value may be longer
    /// (e.g. "In-Person (Physical Attendance)", "In Person", "IN-PERSON").
    pub fn is_in_person(&self) -> bool {
        let lower = self.participation_type.trim().to_lowercase();
        lower.contains("in-person") || lower.contains("in person")
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.email
        } else {
            &self.name
        }
    }
}

/// Represents a raw row from Google Sheets.
/// Column mapping based on the sheet structure:
/// A=api_id, B=name, C=last_name, D=display_name, E=email,
/// F=ticket_name, G=solana_address, H=approval_status,
/// I=checked_in_at, J=checked_in_by, K=qr_code_url
/// Y=participation_type (In-Person / Online)
#[derive(Debug, Clone)]
pub struct AttendeeRow {
    pub api_id: String,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub solana_address: Option<String>,
    pub approval_status: String,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub qr_code_url: Option<String>,
    pub participation_type: String,
    pub row_index: usize,
}

impl AttendeeRow {
    /// Parse a row from Google Sheets values array.
    /// `values` is a Vec of String values for a single row.
    /// `row_index` is the 1-based row number in the sheet (header is row 1).
    pub fn from_sheet_values(values: &[Vec<String>], row_index: usize) -> Option<Self> {
        let row = values.get(row_index - 2)?; // Skip header row (row 1)

        if row.is_empty() {
            return None;
        }

        let get =
            |idx: usize| -> String { row.get(idx).cloned().unwrap_or_default().trim().to_string() };

        let get_opt = |idx: usize| -> Option<String> {
            let val = get(idx);
            if val.is_empty() { None } else { Some(val) }
        };

        let api_id = get(0);
        if api_id.is_empty() {
            return None;
        }

        // Column Y = index 24 (participation_type)
        let participation_type = get(24);

        Some(Self {
            api_id,
            first_name: get(1),
            last_name: get(2),
            name: {
                let col_b = get(1);
                if !col_b.is_empty() { col_b } else { get(3) }
            },
            email: get(4),
            ticket_name: get(5),
            solana_address: get_opt(6),
            approval_status: get(7),
            checked_in_at: get_opt(8),
            checked_in_by: get_opt(9),
            qr_code_url: get_opt(10),
            participation_type,
            row_index,
        })
    }

    /// Convert raw row into a typed Attendee
    pub fn to_attendee(&self) -> Attendee {
        let mut status = self.approval_status.parse::<CheckInStatus>().unwrap();
        if self.checked_in_at.is_some() && status == CheckInStatus::Approved {
            status = CheckInStatus::CheckedIn;
        }

        Attendee {
            api_id: self.api_id.clone(),
            first_name: self.first_name.clone(),
            last_name: self.last_name.clone(),
            name: self.name.clone(),
            email: self.email.clone(),
            ticket_name: self.ticket_name.clone(),
            approval_status: status,
            checked_in_at: self.checked_in_at.clone(),
            checked_in_by: self.checked_in_by.clone(),
            qr_code_url: self.qr_code_url.clone(),
            solana_address: self.solana_address.clone(),
            participation_type: self.participation_type.clone(),
            row_index: self.row_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_attendee(participation_type: &str) -> Attendee {
        Attendee {
            api_id: "gst-test".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            ticket_name: "General".to_string(),
            approval_status: CheckInStatus::Approved,
            checked_in_at: None,
            checked_in_by: None,
            qr_code_url: None,
            solana_address: None,
            participation_type: participation_type.to_string(),
            row_index: 2,
        }
    }

    #[test]
    fn test_is_in_person_exact() {
        assert!(make_attendee("In-Person").is_in_person());
    }

    #[test]
    fn test_is_in_person_case_insensitive() {
        assert!(make_attendee("in-person").is_in_person());
        assert!(make_attendee("IN-PERSON").is_in_person());
        assert!(make_attendee("In-person").is_in_person());
    }

    #[test]
    fn test_is_in_person_with_spaces() {
        assert!(make_attendee("In Person").is_in_person());
        assert!(make_attendee("in person").is_in_person());
        assert!(make_attendee("IN PERSON").is_in_person());
    }

    #[test]
    fn test_is_in_person_long_value() {
        assert!(make_attendee("In-Person (Physical Attendance)").is_in_person());
        assert!(make_attendee("In-Person - On Site").is_in_person());
        assert!(make_attendee("  In-Person  ").is_in_person());
        assert!(make_attendee("In Person Participant").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_online() {
        assert!(!make_attendee("Online").is_in_person());
        assert!(!make_attendee("online").is_in_person());
        assert!(!make_attendee("ONLINE").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_virtual() {
        assert!(!make_attendee("Virtual").is_in_person());
        assert!(!make_attendee("Hybrid").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_empty() {
        assert!(!make_attendee("").is_in_person());
        assert!(!make_attendee("   ").is_in_person());
    }

    #[test]
    fn test_is_not_in_person_other() {
        assert!(!make_attendee("Unknown").is_in_person());
        assert!(!make_attendee("TBD").is_in_person());
    }
}
