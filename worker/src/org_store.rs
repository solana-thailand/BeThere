//! Organization store — CRUD for organizations.
//!
//! Organizations are stored exclusively in D1 (Phase 3c complete).
//! Previously stored in KV under:
//!   "orgs"         → OrgIndex (list of OrgMeta summaries)
//!   "org:{org_id}" → OrganizationConfig (full org config)

use worker::D1Database;

use event_checkin_domain::config::SheetsConfig;
use event_checkin_domain::models::event::EventConfig;
use event_checkin_domain::models::org::{
    CreateOrgRequest, OrganizationConfig, ResolvedContactsSheet, UpdateOrgRequest,
};

use crate::db::organizations;

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Get a single organization config by ID.
pub async fn get_org_config(
    db: &D1Database,
    org_id: &str,
) -> Result<Option<OrganizationConfig>, String> {
    organizations::get_org_config(db, org_id).await
}

/// List all organizations (newest first).
pub async fn list_orgs(db: &D1Database) -> Result<Vec<OrganizationConfig>, String> {
    organizations::list_orgs(db).await
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/// Create a new organization.
pub async fn create_org(
    db: &D1Database,
    req: &CreateOrgRequest,
) -> Result<OrganizationConfig, String> {
    if req.name.trim().is_empty() {
        return Err("organization name is required".to_string());
    }

    let id = slugify_org(&req.name);

    // Deduplicate slug
    let existing = organizations::get_org_config(db, &id).await?;
    if existing.is_some() {
        return Err(format!("organization '{id}' already exists"));
    }

    let now = chrono::Utc::now().to_rfc3339();

    let config = OrganizationConfig {
        id: id.clone(),
        name: req.name.trim().to_string(),
        contacts_sheet_id: req.contacts_sheet_id.trim().to_string(),
        contacts_sheet_name: if req.contacts_sheet_name.is_empty() {
            "Contacts".to_string()
        } else {
            req.contacts_sheet_name.clone()
        },
        events_sheet_name: if req.events_sheet_name.is_empty() {
            "Events".to_string()
        } else {
            req.events_sheet_name.clone()
        },
        owner_emails: req
            .owner_emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect(),
        created_at: now.clone(),
        updated_at: now,
    };

    organizations::insert_org(db, &config).await?;

    tracing::info!(org_id = %id, name = %config.name, "organization created");

    Ok(config)
}

/// Update an existing organization.
pub async fn update_org(
    db: &D1Database,
    org_id: &str,
    req: &UpdateOrgRequest,
) -> Result<OrganizationConfig, String> {
    let mut config = organizations::get_org_config(db, org_id)
        .await?
        .ok_or_else(|| format!("organization '{org_id}' not found"))?;

    if let Some(ref name) = req.name {
        config.name = name.trim().to_string();
    }
    if let Some(ref sheet_id) = req.contacts_sheet_id {
        config.contacts_sheet_id = sheet_id.trim().to_string();
    }
    if let Some(ref sheet_name) = req.contacts_sheet_name {
        config.contacts_sheet_name = sheet_name.clone();
    }
    if let Some(ref sheet_name) = req.events_sheet_name {
        config.events_sheet_name = sheet_name.clone();
    }
    if let Some(ref emails) = req.owner_emails {
        config.owner_emails = emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
    }

    config.updated_at = chrono::Utc::now().to_rfc3339();

    organizations::update_org(db, &config).await?;

    tracing::info!(org_id = %org_id, "organization updated");

    Ok(config)
}

/// Delete an organization (only if it has no active events in D1).
pub async fn delete_org(db: &D1Database, org_id: &str) -> Result<(), String> {
    // Check for active events in this org via D1
    let has_active = crate::db::events::has_active_events_for_org(db, org_id).await?;
    if has_active {
        // Count for error message
        let count = crate::db::events::count_active_events_for_org(db, org_id).await?;
        return Err(format!(
            "cannot delete org '{}': still has {count} active event(s)",
            org_id,
        ));
    }

    organizations::delete_org(db, org_id).await?;

    tracing::info!(org_id = %org_id, "organization deleted");

    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Resolve the contacts sheet info for an event's organization.
///
/// If the event has an `organization_id`, loads the org config from D1 and uses
/// its sheet settings. Otherwise, falls back to the global `SheetsConfig`.
pub async fn resolve_contacts_sheet(
    db: &D1Database,
    event_config: &EventConfig,
    global_sheets: &SheetsConfig,
) -> ResolvedContactsSheet {
    if !event_config.organization_id.is_empty()
        && let Ok(Some(org)) =
            organizations::get_org_config(db, &event_config.organization_id).await
    {
        let sheet_id = if org.contacts_sheet_id.is_empty() {
            global_sheets.contacts_sheet_id.clone()
        } else {
            org.contacts_sheet_id
        };
        return ResolvedContactsSheet {
            sheet_id,
            contacts_sheet_name: org.contacts_sheet_name,
            events_sheet_name: org.events_sheet_name,
        };
    }

    // Fallback to global
    ResolvedContactsSheet {
        sheet_id: global_sheets.contacts_sheet_id.clone(),
        contacts_sheet_name: global_sheets.contacts_sheet_name.clone(),
        events_sheet_name: global_sheets.events_sheet_name.clone(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn slugify_org(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
