//! Event status, format, visibility and escrow enumerations.

use serde::{Deserialize, Serialize};

/// Event status (mirrors backend EventStatus).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    #[default]
    Draft,
    Active,
    Completed,
    Archived,
}

/// On-chain escrow lifecycle status (mirrors backend EscrowStatus).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    #[default]
    None,
    Initialized,
    Deactivated,
    Closed,
    Cancelled,
}

impl EscrowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Initialized => "initialized",
            Self::Deactivated => "deactivated",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether the escrow is considered "active" (blocking archive/delete).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Initialized | Self::Deactivated)
    }
}

/// Event format (mirrors backend EventFormat).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventFormat {
    #[default]
    InPerson,
    Online,
    Hybrid,
}

impl EventFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::InPerson => "In-Person",
            Self::Online => "Online",
            Self::Hybrid => "Hybrid",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Online => "online",
            Self::Hybrid => "hybrid",
        }
    }

    /// Whether this format includes an in-person track.
    pub fn has_in_person(&self) -> bool {
        matches!(self, Self::InPerson | Self::Hybrid)
    }

    /// Whether this format includes an online track.
    pub fn has_online(&self) -> bool {
        matches!(self, Self::Online | Self::Hybrid)
    }
}

/// Controls when online registration opens for hybrid events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnlineOpenMode {
    #[default]
    Always,
    AutoOnFull,
    Manual,
}

impl OnlineOpenMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Always => "Always Open",
            Self::AutoOnFull => "Auto (when in-person full)",
            Self::Manual => "Manual Toggle",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::AutoOnFull => "auto_on_full",
            Self::Manual => "manual",
        }
    }
}

/// Event visibility (mirrors backend EventVisibility).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
    #[default]
    Public,
    Private,
}

impl EventVisibility {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Public => "Public",
            Self::Private => "Private",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// Default true helper for serde.
pub(super) fn default_true_fn() -> bool {
    true
}
