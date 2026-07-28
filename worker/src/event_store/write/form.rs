//! Registration form config writes (Issue #049).

use worker::{D1Database, KvStore};

/// Save per-event registration form config.
/// D1-first: writes to the `form_config` column on the events table.
/// Falls back to KV if D1 is unavailable.
pub async fn save_form_config(
    event_id: &str,
    config: &event_checkin_domain::models::event::RegistrationFormConfig,
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    // D1-first
    if let Some(db) = d1 {
        return crate::db::events::save_form_config(db, event_id, config).await;
    }

    // KV fallback
    if let Some(kv) = kv {
        use crate::event_store::schema::form_config_key;
        let key = form_config_key(event_id);
        let json_str = serde_json::to_string(config)
            .map_err(|e| format!("failed to serialize form config: {e:?}"))?;
        kv.put(&key, &json_str)
            .map_err(|e| format!("failed to build form config put: {e:?}"))?
            .execute()
            .await
            .map_err(|e| format!("failed to write form config to KV: {e:?}"))?;
    }

    Ok(())
}
