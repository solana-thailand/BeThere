//! Organization model for per-org contacts sheet isolation (Approach A).
//!
//! Each organization has its own Google Sheet for contacts + events tab.
//! Events are assigned to an organization via `organization_id`. When an
//! attendee registers or contacts are synced, the system resolves the
//! contacts sheet from the event's organization.

use serde::{Deserialize, Serialize};

/// Organization configuration stored in KV under `org:{org_id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationConfig {
    /// Unique organization identifier (slug-style, e.g. "solana-thailand").
    pub id: String,
    /// Display name (e.g. "Solana Thailand").
    pub name: String,
    /// Google Sheet ID for this org's contacts + events tab.
    /// If empty, falls back to the global `CONTACTS_SHEET_ID`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contacts_sheet_id: String,
    /// Tab name for contacts within the org's sheet. Defaults to "Contacts".
    #[serde(default)]
    pub contacts_sheet_name: String,
    /// Tab name for events registry within the org's sheet. Defaults to "Events".
    #[serde(default)]
    pub events_sheet_name: String,
    /// Owner emails — full control over org settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owner_emails: Vec<String>,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-update timestamp.
    #[serde(default)]
    pub updated_at: String,
}

/// Top-level index of all organizations, stored under KV key "orgs".
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgIndex {
    #[serde(default)]
    pub orgs: Vec<OrgMeta>,
}

/// Lightweight org metadata for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub owner_emails: Vec<String>,
    #[serde(default)]
    pub contacts_sheet_id: String,
}

/// Request to create a new organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrgRequest {
    /// Display name (required).
    pub name: String,
    /// Google Sheet ID for this org's contacts.
    /// If empty, events without a sheet fall back to global config.
    #[serde(default)]
    pub contacts_sheet_id: String,
    /// Tab name for contacts. Defaults to "Contacts".
    #[serde(default)]
    pub contacts_sheet_name: String,
    /// Tab name for events. Defaults to "Events".
    #[serde(default)]
    pub events_sheet_name: String,
    /// Owner email addresses (at least one required).
    #[serde(default)]
    pub owner_emails: Vec<String>,
}

/// Request to update an existing organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrgRequest {
    /// New display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New contacts sheet ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contacts_sheet_id: Option<String>,
    /// New contacts tab name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contacts_sheet_name: Option<String>,
    /// New events tab name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_sheet_name: Option<String>,
    /// Replace owner emails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_emails: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl OrganizationConfig {
    /// Convert to lightweight meta for index.
    pub fn to_meta(&self) -> OrgMeta {
        OrgMeta {
            id: self.id.clone(),
            name: self.name.clone(),
            owner_emails: self.owner_emails.clone(),
            contacts_sheet_id: self.contacts_sheet_id.clone(),
        }
    }
}

/// Resolved contacts sheet info — where to write contacts/events for a given org.
#[derive(Debug, Clone)]
pub struct ResolvedContactsSheet {
    /// Google Sheet ID.
    pub sheet_id: String,
    /// Contacts tab name.
    pub contacts_sheet_name: String,
    /// Events tab name.
    pub events_sheet_name: String,
}
