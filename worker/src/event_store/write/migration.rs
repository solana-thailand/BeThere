//! Quiz migration from legacy QUIZ namespace to event-scoped EVENTS namespace.

use worker::KvStore;

/// Result of a quiz migration operation.
pub struct MigrationResult {
    /// `true` if data was copied, `false` if destination already existed.
    pub migrated: bool,
    /// The event ID that was the migration target.
    pub event_id: String,
    /// Human-readable status message.
    pub message: String,
}

/// Migrate quiz config from legacy QUIZ namespace to event-scoped EVENTS namespace.
/// Idempotent — skips if destination key already exists.
pub async fn migrate_quiz_to_event(
    events_kv: &KvStore,
    quiz_kv: &KvStore,
    event_id: &str,
) -> Result<MigrationResult, String> {
    let dest_key = format!("event:{event_id}:quiz:questions");

    // Idempotent: skip if destination already exists
    let existing: Option<String> = events_kv
        .get(&dest_key)
        .text()
        .await
        .map_err(|e| format!("failed to check destination key '{dest_key}': {e:?}"))?;

    match existing {
        Some(_) => {
            tracing::info!(dest_key = %dest_key, "migrate: destination already exists, skipping");
            Ok(MigrationResult {
                migrated: false,
                event_id: event_id.to_string(),
                message: format!("quiz data already migrated to event '{event_id}'"),
            })
        }
        None => {
            // Read source from legacy QUIZ namespace
            let raw: Option<String> =
                quiz_kv.get("questions").text().await.map_err(|e| {
                    format!("failed to read 'questions' from QUIZ namespace: {e:?}")
                })?;

            let source: serde_json::Value =
                serde_json::from_str(raw.as_deref().ok_or_else(|| {
                    "no quiz data found in QUIZ namespace (key 'questions' is empty)".to_string()
                })?)
                .map_err(|e| format!("failed to parse quiz data from QUIZ namespace: {e}"))?;

            // Write to EVENTS namespace
            let json_str = serde_json::to_string(&source)
                .map_err(|e| format!("failed to serialize quiz data: {e:?}"))?;
            events_kv
                .put(&dest_key, &json_str)
                .map_err(|e| format!("failed to build quiz migration put: {e:?}"))?
                .execute()
                .await
                .map_err(|e| format!("failed to write quiz data to '{dest_key}': {e:?}"))?;

            tracing::info!(dest_key = %dest_key, "migrate: copied quiz data");
            Ok(MigrationResult {
                migrated: true,
                event_id: event_id.to_string(),
                message: format!("quiz data migrated to event '{event_id}'"),
            })
        }
    }
}
