//! Event lifecycle, format, visibility and escrow enumerations.

use serde::{Deserialize, Serialize};

/// Controls when online registration opens for hybrid events.
///
/// - `Always`: Both tracks open from registration start.
/// - `AutoOnFull`: Online opens automatically when in-person capacity is reached.
/// - `Manual`: Organizer flips toggle manually via staff UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnlineOpenMode {
    #[default]
    Always,
    #[serde(alias = "auto")]
    AutoOnFull,
    Manual,
}

impl OnlineOpenMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::AutoOnFull => "auto_on_full",
            Self::Manual => "manual",
        }
    }
}

impl std::fmt::Display for OnlineOpenMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Controls event discoverability — whether the event appears publicly or requires auth.
///
/// - `Public`: Visible on landing page, accessible to anyone via `/e/{slug}`.
/// - `Private`: Hidden from landing page, requires auth + access check via `/e/{slug}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
    #[default]
    Public,
    Private,
}

impl EventVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

impl std::fmt::Display for EventVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Event format — controls deposit, check-in, claim, and escrow paths.
///
/// - `InPerson`: Physical event, deposit auto-enabled, physical check-in required.
/// - `Online`: Virtual event, no deposit, quest completion = virtual check-in.
/// - `Hybrid`: Both tracks in one event, one Google Sheet with participation_type column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EventFormat {
    #[default]
    InPerson,
    Online,
    Hybrid,
}

impl EventFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Online => "online",
            Self::Hybrid => "hybrid",
        }
    }

    /// Whether this format includes an in-person track (requires deposit, physical check-in).
    pub fn has_in_person(&self) -> bool {
        matches!(self, Self::InPerson | Self::Hybrid)
    }

    /// Whether this format includes an online track (quest-based virtual check-in).
    pub fn has_online(&self) -> bool {
        matches!(self, Self::Online | Self::Hybrid)
    }
}

impl std::fmt::Display for EventFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// On-chain escrow lifecycle status.
/// Tracks the state machine: None → Initialized → Deactivated → Closed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    /// No escrow initialized on-chain (or never set).
    #[default]
    None,
    /// Escrow PDA created on-chain, accepting deposits.
    Initialized,
    /// Escrow deactivated — no new deposits, refunds still allowed.
    Deactivated,
    /// Escrow closed — all on-chain accounts reclaimed, rent refunded.
    Closed,
    /// Event cancelled — refunds in progress (organizer-initiated cancellation).
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

impl std::fmt::Display for EscrowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Lifecycle status of an event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EventStatus {
    /// Event is being configured, not yet visible to attendees.
    #[default]
    Draft,
    /// Event is live — attendees can check in, claim, etc.
    Active,
    /// Event has ended — attendance frozen, claims still possible.
    Completed,
    /// Event is soft-deleted / hidden from listings.
    Archived,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}
