//! Event index and per-event config writes, plus D1 dual-write sync.

use worker::KvStore;

use event_checkin_domain::models::event::{EventConfig, EventIndex};

use crate::event_store::schema::event_config_key;

// ---------------------------------------------------------------------------
// Event index writes
// ---------------------------------------------------------------------------

/// Write the event index to KV.
pub async fn save_event_index(kv: &KvStore, index: &EventIndex) -> Result<(), String> {
    let json_str = serde_json::to_string(index)
        .map_err(|e| format!("failed to serialize event index: {e:?}"))?;
    kv.put("events", &json_str)
        .map_err(|e| format!("failed to build event index put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event index to KV: {e:?}"))
}

// ---------------------------------------------------------------------------
// Per-event config writes
// ---------------------------------------------------------------------------

/// Write a single event's full configuration.
pub async fn save_event_config(kv: &KvStore, config: &EventConfig) -> Result<(), String> {
    let key = event_config_key(&config.id);
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize event config: {e:?}"))?;
    kv.put(&key, &json_str)
        .map_err(|e| format!("failed to build event config put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event config to KV: {e:?}"))
}

/// Outcome of the D1 event dual-write.
///
/// `#[must_use]` so a handler can't drop a failure silently: the D1-first
/// public list serves the old row until the next successful save (`.issues/152`).
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D1Sync {
    Synced,
    NoDatabase,
    Failed,
}

impl D1Sync {
    /// Warning for the admin who saved, when the D1 copy did not update.
    pub fn warning(self) -> Option<&'static str> {
        match self {
            D1Sync::Failed => Some(
                "Saved, but the database copy did not update. Public pages may show the old details. Save again to retry.",
            ),
            D1Sync::Synced | D1Sync::NoDatabase => None,
        }
    }

    /// `warning()` as the `warnings` list the admin UI renders.
    pub fn warnings(self) -> Vec<&'static str> {
        self.warning().into_iter().collect()
    }
}

/// An event the store saved, with the outcome of its D1 dual-write, so the
/// handler that owns the admin response can report it (`D1Sync::warnings`).
#[must_use]
#[derive(Debug, Clone)]
pub struct SavedEvent {
    pub config: EventConfig,
    pub d1_sync: D1Sync,
}

/// Why an event create was refused. Invalid input is the admin's to fix and
/// must reach them as a 400 with its message; a storage failure is a 500.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventWriteError {
    Invalid(String),
    Storage(String),
}

impl std::fmt::Display for EventWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventWriteError::Invalid(msg) | EventWriteError::Storage(msg) => f.write_str(msg),
        }
    }
}

impl EventWriteError {
    /// Log at the level the kind deserves (refused input is a warning, a
    /// storage failure an error) and convert for the response.
    pub fn logged(self, action: &'static str) -> event_checkin_domain::models::error::AppError {
        match &self {
            EventWriteError::Invalid(msg) => {
                tracing::warn!(error = %msg, action, "event write refused")
            }
            EventWriteError::Storage(msg) => {
                tracing::error!(error = %msg, action, "event write failed")
            }
        }
        self.into()
    }
}

impl From<EventWriteError> for event_checkin_domain::models::error::AppError {
    fn from(e: EventWriteError) -> Self {
        match e {
            EventWriteError::Invalid(msg) => Self::Validation(msg),
            EventWriteError::Storage(msg) => Self::Internal(msg),
        }
    }
}

/// Dual-write: persist event config to D1 alongside KV.
/// Non-blocking — errors are logged and returned as [`D1Sync::Failed`], not
/// propagated, so the KV write still happens.
pub async fn sync_event_to_d1(d1: Option<&worker::D1Database>, config: &EventConfig) -> D1Sync {
    let Some(db) = d1 else {
        return D1Sync::NoDatabase;
    };
    match crate::db::events::upsert_event(db, config).await {
        Ok(()) => D1Sync::Synced,
        Err(e) => {
            tracing::warn!(event_id = %config.id, error = %e, "D1 event dual-write failed");
            D1Sync::Failed
        }
    }
}

/// Dual-write: delete an event from D1 alongside KV, with its derived rows.
///
/// `event_summaries` holds a frozen funnel snapshot keyed by `event_id` with no
/// FK to `events`, so a permanent delete would otherwise orphan it forever.
/// Rows that carry standalone record-keeping value (`audit_log`,
/// `credit_ledger`, `attendees`) are deliberately left in place.
///
/// Errors are logged, not propagated — KV remains the source of truth, and the
/// summary delete must not mask a successful event delete.
pub async fn sync_delete_event_from_d1(d1: Option<&worker::D1Database>, event_id: &str) {
    let Some(db) = d1 else { return };

    if let Err(e) = crate::db::events::delete_event(db, event_id).await {
        tracing::warn!(event_id = %event_id, error = %e, "D1 event delete failed");
    }

    if let Err(e) = crate::db::event_summaries::delete_summary(db, event_id).await {
        tracing::warn!(event_id = %event_id, error = %e, "D1 event summary delete failed");
    }
}
