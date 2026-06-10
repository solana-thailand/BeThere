//! D1 organization query helpers.
//!
//! Organizations stored exclusively in D1 (Phase 3c complete).

use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Read a single organization config from D1. Returns `None` if not found.
pub async fn get_org_config(
    db: &D1Database,
    org_id: &str,
) -> Result<Option<event_checkin_domain::models::org::OrganizationConfig>, String> {
    let stmt = db.prepare("SELECT * FROM organizations WHERE id = ?1");
    let bound = stmt
        .bind_refs(&[D1Type::Text(org_id)])
        .map_err(|e| format!("D1 get_org bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_org first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_org first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_org: deserialize failed"
        );
        format!("D1 get_org deserialize: {e}")
    })?;

    Ok(Some(row_to_org_config(row)?))
}

/// List all organizations from D1 (newest first).
pub async fn list_orgs(
    db: &D1Database,
) -> Result<Vec<event_checkin_domain::models::org::OrganizationConfig>, String> {
    let stmt = db.prepare("SELECT * FROM organizations ORDER BY created_at DESC");
    let result = stmt
        .all()
        .await
        .map_err(|e| format!("D1 list_orgs: {e:?}"))?;

    let rows: Vec<serde_json::Value> = result
        .results()
        .map_err(|e| format!("D1 list_orgs results: {e:?}"))?;

    rows.into_iter()
        .map(row_to_org_config)
        .collect::<Result<Vec<_>, _>>()
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Insert a new organization into D1.
pub async fn insert_org(
    db: &D1Database,
    config: &event_checkin_domain::models::org::OrganizationConfig,
) -> Result<(), String> {
    let owner_emails_json = serde_json::to_string(&config.owner_emails)
        .map_err(|e| format!("serialize owner_emails: {e}"))?;

    let stmt = db.prepare(
        "INSERT INTO organizations (id, name, contacts_sheet_id, contacts_sheet_name, events_sheet_name, owner_emails, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    );
    stmt.bind_refs(&[
        D1Type::Text(&config.id),
        D1Type::Text(&config.name),
        D1Type::Text(&config.contacts_sheet_id),
        D1Type::Text(&config.contacts_sheet_name),
        D1Type::Text(&config.events_sheet_name),
        D1Type::Text(&owner_emails_json),
        D1Type::Text(&config.created_at),
        D1Type::Text(&config.updated_at),
    ])
    .map_err(|e| format!("D1 insert_org bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 insert_org run: {e:?}"))?;

    Ok(())
}

/// Update an existing organization in D1.
pub async fn update_org(
    db: &D1Database,
    config: &event_checkin_domain::models::org::OrganizationConfig,
) -> Result<(), String> {
    let owner_emails_json = serde_json::to_string(&config.owner_emails)
        .map_err(|e| format!("serialize owner_emails: {e}"))?;

    let stmt = db.prepare(
        "UPDATE organizations SET name = ?1, contacts_sheet_id = ?2, contacts_sheet_name = ?3, events_sheet_name = ?4, owner_emails = ?5, updated_at = ?6 \
         WHERE id = ?7",
    );
    stmt.bind_refs(&[
        D1Type::Text(&config.name),
        D1Type::Text(&config.contacts_sheet_id),
        D1Type::Text(&config.contacts_sheet_name),
        D1Type::Text(&config.events_sheet_name),
        D1Type::Text(&owner_emails_json),
        D1Type::Text(&config.updated_at),
        D1Type::Text(&config.id),
    ])
    .map_err(|e| format!("D1 update_org bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 update_org run: {e:?}"))?;

    Ok(())
}

/// Delete an organization from D1.
pub async fn delete_org(db: &D1Database, org_id: &str) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM organizations WHERE id = ?1");
    stmt.bind_refs(&[D1Type::Text(org_id)])
        .map_err(|e| format!("D1 delete_org bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_org run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Row → Domain conversion
// ---------------------------------------------------------------------------

fn row_to_org_config(
    row: serde_json::Value,
) -> Result<event_checkin_domain::models::org::OrganizationConfig, String> {
    let get_str = |field: &str| -> String {
        row.get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let owner_emails: Vec<String> = row
        .get("owner_emails")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Ok(event_checkin_domain::models::org::OrganizationConfig {
        id: get_str("id"),
        name: get_str("name"),
        contacts_sheet_id: get_str("contacts_sheet_id"),
        contacts_sheet_name: get_str("contacts_sheet_name"),
        events_sheet_name: get_str("events_sheet_name"),
        owner_emails,
        created_at: get_str("created_at"),
        updated_at: get_str("updated_at"),
    })
}
