//! Check-in status and participation-type enumerations.

use std::fmt;
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

/// Canonical attendee participation type.
///
/// Normalized from the raw Google Sheet `participation_type` column, which has
/// inconsistent casing/formatting in production (`In-Person`, `IN_PERSON`,
/// `in person`, `Online`, `online`, `""`, `test`, ...). Use [`Self::parse`] to
/// canonicalize; [`Attendee::is_in_person`] delegates to this enum.
///
/// Canonical wire form is snake_case (`in_person` / `online` / `retrospective`
/// / `other`),
/// matching `EventFormat`'s convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParticipationType {
    #[default]
    InPerson,
    Online,
    /// Post-event learning lead. It is deliberately distinct from `Online` so
    /// retrospective enrollment cannot alter live-attendance reporting.
    Retrospective,
    /// Unrecognized value (e.g. "test", "TBD"). Treated as NOT in-person.
    Other,
}

impl ParticipationType {
    /// Canonical snake_case identifier (stored in D1).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Online => "online",
            Self::Retrospective => "retrospective",
            Self::Other => "other",
        }
    }

    /// Human-readable label for the Google Sheet and UI (display-case).
    /// Inverse of `parse` for the two participation modes; `Other` passes
    /// through as a capitalized "Other".
    pub fn display(&self) -> &'static str {
        match self {
            Self::InPerson => "In-Person",
            Self::Online => "Online",
            Self::Retrospective => "Retrospective",
            Self::Other => "Other",
        }
    }

    /// Canonicalize a raw participation_type string into a typed value.
    ///
    /// Handles all known production variants via case-insensitive substring
    /// matching (the sheet value may be longer, e.g.
    /// "In-Person (Physical Attendance)"). Empty/whitespace defaults to
    /// `InPerson` — legacy events predate this column and were all in-person.
    ///
    /// In-person is checked first to preserve prior `is_in_person` behavior
    /// for ambiguous values that mention both tracks.
    pub fn parse(s: &str) -> Self {
        let lower = s.trim().to_lowercase();
        if lower.is_empty() {
            return Self::InPerson;
        }
        if lower.contains("in-person")
            || lower.contains("in person")
            || lower.contains("in_person")
            || lower.contains("physical")
        {
            return Self::InPerson;
        }
        if lower.contains("online") || lower.contains("virtual") {
            return Self::Online;
        }
        if lower == "retrospective" {
            return Self::Retrospective;
        }
        Self::Other
    }
}

impl FromStr for ParticipationType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

impl fmt::Display for ParticipationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
