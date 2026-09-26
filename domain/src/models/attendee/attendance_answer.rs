//! What a registrant told the organizer when asked "can you still come?".
//!
//! Recorded by staff after a postponement or any other change of plan, so the
//! roster shows who is confirmed, who has not decided and who is out. It is
//! deliberately separate from `approval_status` (which gates check-in) and
//! from `participation_type` (which the organizer flips separately when
//! somebody moves online): an answer is information, not a state change.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttendanceAnswer {
    Coming,
    Undecided,
    NotComing,
}

impl AttendanceAnswer {
    pub const ALL: [Self; 3] = [Self::Coming, Self::Undecided, Self::NotComing];

    /// Canonical snake_case identifier (stored in D1, sent over the wire).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Coming => "coming",
            Self::Undecided => "undecided",
            Self::NotComing => "not_coming",
        }
    }

    /// Strict inverse of [`Self::as_str`]; anything else is `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == raw.trim())
    }

    /// Label for the admin roster and CSV.
    pub fn label(self) -> &'static str {
        match self {
            Self::Coming => "Coming",
            Self::Undecided => "Not sure yet",
            Self::NotComing => "Can't come",
        }
    }
}
