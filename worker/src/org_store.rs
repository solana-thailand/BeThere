//! Organization KV store — CRUD for organizations.
//!
//! Organizations are stored in the EVENTS KV namespace:
//!   "orgs"         → OrgIndex (list of OrgMeta summaries)
//!   "org:{org_id}" → OrganizationConfig (full org config)

use worker::KvStore;

use event_checkin_domain::models::event::EventStatus;
use event_checkin_domain::models::org::{
    CreateOrgRequest, OrgIndex, OrganizationConfig, UpdateOrgRequest,
};

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

pub async fn get_org_index(kv: &KvStore) -> Result<OrgIndex, String> {
    kv.get("orgs")
        .json::<OrgIndex>()
        .await
        .map_err(|e| format!("failed to read org index: {e:?}"))
        .map(|opt| opt.unwrap_or_default())
}

async fn save_org_index(kv: &KvStore, index: &OrgIndex) -> Result<(), String> {
    kv.put("orgs", index)
        .map_err(|e| format!("failed to create org index KV entry: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to save org index: {e:?}"))
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

fn org_config_key(org_id: &str) -> String {
    format!("org:{org_id}")
}

pub async fn get_org_config(
    kv: &KvStore,
    org_id: &str,
) -> Result<Option<OrganizationConfig>, String> {
    let key = org_config_key(org_id);
    kv.get(&key)
        .json::<OrganizationConfig>()
        .await
        .map_err(|e| format!("failed to read org config '{key}': {e:?}"))
}

async fn save_org_config(kv: &KvStore, config: &OrganizationConfig) -> Result<(), String> {
    let key = org_config_key(&config.id);
    kv.put(&key, config)
        .map_err(|e| format!("failed to create org config KV entry: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to save org config: {e:?}"))
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/// Create a new organization.
pub async fn create_org(
    kv: &KvStore,
    req: &CreateOrgRequest,
) -> Result<OrganizationConfig, String> {
    if req.name.trim().is_empty() {
        return Err("organization name is required".to_string());
    }

    let id = slugify_org(&req.name);

    // Deduplicate slug
    let index = get_org_index(kv).await?;
    if index.orgs.iter().any(|o| o.id == id) {
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

    save_org_config(kv, &config).await?;

    let mut index = index;
    index.orgs.insert(0, config.to_meta());
    save_org_index(kv, &index).await?;

    tracing::info!(org_id = %id, name = %config.name, "organization created");

    Ok(config)
}

/// Update an existing organization.
pub async fn update_org(
    kv: &KvStore,
    org_id: &str,
    req: &UpdateOrgRequest,
) -> Result<OrganizationConfig, String> {
    let mut config = get_org_config(kv, org_id)
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

    save_org_config(kv, &config).await?;

    // Update index
    let mut index = get_org_index(kv).await?;
    if let Some(entry) = index.orgs.iter_mut().find(|o| o.id == org_id) {
        *entry = config.to_meta();
    }
    save_org_index(kv, &index).await?;

    tracing::info!(org_id = %org_id, "organization updated");

    Ok(config)
}

/// Delete an organization (only if it has no active events).
pub async fn delete_org(kv: &KvStore, org_id: &str) -> Result<(), String> {
    // Check for active events in this org
    let event_index = crate::event_store::get_event_index(kv).await?;
    let active_events: Vec<_> = event_index
        .events
        .iter()
        .filter(|e| e.organization_id == org_id && !matches!(e.status, EventStatus::Archived))
        .collect();

    if !active_events.is_empty() {
        return Err(format!(
            "cannot delete org '{}': still has {} active event(s)",
            org_id,
            active_events.len()
        ));
    }

    // Delete config
    let key = org_config_key(org_id);
    kv.delete(&key)
        .await
        .map_err(|e| format!("failed to delete org config: {e:?}"))?;

    // Remove from index
    let mut index = get_org_index(kv).await?;
    index.orgs.retain(|o| o.id != org_id);
    save_org_index(kv, &index).await?;

    tracing::info!(org_id = %org_id, "organization deleted");

    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Resolve the contacts sheet info for an event's organization.
///
/// If the event has an `organization_id`, loads the org config and uses its
/// sheet settings. Otherwise, falls back to the global `SheetsConfig`.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_contacts_sheet(
    kv: &KvStore,
    event_config: &event_checkin_domain::models::event::EventConfig,
    global_sheets: &event_checkin_domain::config::SheetsConfig,
) -> event_checkin_domain::models::org::ResolvedContactsSheet {
    if !event_config.organization_id.is_empty()
        && let Ok(Some(org)) = get_org_config(kv, &event_config.organization_id).await
    {
        let sheet_id = if org.contacts_sheet_id.is_empty() {
            global_sheets.contacts_sheet_id.clone()
        } else {
            org.contacts_sheet_id
        };
        return event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id,
            contacts_sheet_name: org.contacts_sheet_name,
            events_sheet_name: org.events_sheet_name,
        };
    }

    // Fallback to global
    event_checkin_domain::models::org::ResolvedContactsSheet {
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
