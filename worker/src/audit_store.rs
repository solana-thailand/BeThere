//! Append-only audit log stored in the EVENTS KV namespace.
//!
//! Key schema:
//!   "event:{id}:audit"  → JSON array of `AuditEntry` (per-event log, max 500)
//!   "audit:global"      → JSON array of `AuditEntry` (system-wide log, max 1000)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use worker::KvStore;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Who performed the action (email from JWT claims, or "system")
    pub actor: String,
    /// What action was performed
    pub action: AuditAction,
    /// What entity was affected (event ID, attendee ID, etc.)
    pub target: String,
    /// Human-readable description
    pub description: String,
    /// Optional structured metadata
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    // Event lifecycle
    EventCreated,
    EventUpdated,
    EventArchived,
    EventRestored,
    EventHardDeleted,
    /// Post-event summary snapshot was frozen (Plan 008 — Phase 1).
    EventSummaryFrozen,
    /// Public recap was published for an event (Plan 008 — Phase 2).
    EventRecapPublished,
    /// Public recap was unpublished / draft-saved (Plan 008 — Phase 2).
    EventRecapUnpublished,
    /// Post-event registration (lead capture) was toggled open/closed (Plan 008 — Phase 3).
    PostEventRegistrationToggled,

    // Escrow lifecycle
    EscrowInitialized,
    EscrowDeactivated,
    EscrowClosed,
    EscrowReinitialized,
    EscrowRolloverInitiated,

    // Deposit lifecycle
    DepositSubmitted,
    DepositConfirmed,
    DepositVerified,
    DepositRejected,
    RefundIssued,
    RefundMarked,
    /// Attendee held their deposit as rolling credit for future events
    /// (off-chain THB; sibling of `RefundMarked`). Recorded for auditability
    /// of the credit-granting action (Issue #032 / #061).
    DepositHeldAsCredit,
    ClaimForfeited,

    // Check-in
    AttendeeCheckedIn,
    AttendeeCheckinUndone,

    // Walk-in
    WalkinRegistered,
    WalkinDeleted,
    WalkinSynced,
    WalkinExported,

    // Auth
    UserLogin,
    UserLogout,
    AccessDenied,

    // NFT
    NftClaimed,
    NftMinted,

    // Quiz/Adventure
    QuizSubmitted,
    AdventureCompleted,

    // Admin
    AttendeeDeleted,
    ForceDeleteUsed,
    /// Admin manually overrode an attendee's participation_type
    /// (e.g. deposit-pending attendee confirmed via out-of-band contact
    /// that they will attend online instead).
    ParticipationTypeChanged,

    // Privacy (PDPA)
    DataDeletionRequested,
    MarketingUnsubscribed,

    // On-chain indexing
    OnChainEventIndexed,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_EVENT_AUDIT: usize = 500;
const MAX_GLOBAL_AUDIT: usize = 1000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create an `AuditEntry` with the current UTC timestamp.
pub fn create_entry(
    actor: &str,
    action: AuditAction,
    target: &str,
    description: &str,
) -> AuditEntry {
    AuditEntry {
        timestamp: Utc::now().to_rfc3339(),
        actor: actor.to_string(),
        action,
        target: target.to_string(),
        description: description.to_string(),
        metadata: None,
    }
}

/// Create an `AuditEntry` with metadata.
pub fn create_entry_with_meta(
    actor: &str,
    action: AuditAction,
    target: &str,
    description: &str,
    metadata: serde_json::Value,
) -> AuditEntry {
    AuditEntry {
        metadata: Some(metadata),
        ..create_entry(actor, action, target, description)
    }
}

// ---------------------------------------------------------------------------
// Internal read / write
// ---------------------------------------------------------------------------

async fn read_entries(kv: &KvStore, key: &str) -> Vec<AuditEntry> {
    let raw: Option<String> = match kv.get(key).text().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(key, "audit KV read failed: {e:?}");
            return Vec::new();
        }
    };

    match raw {
        None => Vec::new(),
        Some(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            tracing::warn!(key, "audit parse failed: {e:?}");
            Vec::new()
        }),
    }
}

async fn write_entries(
    kv: &KvStore,
    key: &str,
    entries: &[AuditEntry],
    max: usize,
) -> Result<(), String> {
    // Keep only the newest `max` entries (append at end, truncate from front)
    let start = entries.len().saturating_sub(max);
    let trimmed = &entries[start..];

    let json =
        serde_json::to_string(trimmed).map_err(|e| format!("audit serialize failed: {e:?}"))?;

    kv.put(key, &json)
        .map_err(|e| format!("audit KV put failed: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("audit KV write failed: {e:?}"))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Append an audit entry to an event's audit log (max 500 entries).
pub async fn append_event_audit(
    kv: &KvStore,
    event_id: &str,
    entry: AuditEntry,
    d1: Option<&worker::D1Database>,
) -> Result<(), String> {
    // D1 path: O(1) INSERT — always attempt if available
    if let Some(db) = d1 {
        let action_str = serde_json::to_string(&entry.action)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        let metadata_str = entry.metadata.as_ref().map(|v| v.to_string());
        let _ = crate::db::append_audit(
            db,
            event_id,
            &entry.actor,
            &action_str,
            &entry.target,
            &entry.description,
            metadata_str.as_deref(),
        )
        .await;
        // Fire-and-forget D1 write — KV is still the primary read source
    }

    // KV fallback (always runs for read compatibility)
    let key = format!("event:{event_id}:audit");
    let mut entries = read_entries(kv, &key).await;
    entries.push(entry);
    write_entries(kv, &key, &entries, MAX_EVENT_AUDIT).await
}

/// Append an audit entry to the global audit log (max 1000 entries).
pub async fn append_global_audit(
    kv: &KvStore,
    entry: AuditEntry,
    d1: Option<&worker::D1Database>,
) -> Result<(), String> {
    // D1 path: append to global audit using "__global__" event_id
    if let Some(db) = d1 {
        let action_str = serde_json::to_string(&entry.action)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        let metadata_str = entry.metadata.as_ref().map(|v| v.to_string());
        let _ = crate::db::append_audit(
            db,
            "__global__",
            &entry.actor,
            &action_str,
            &entry.target,
            &entry.description,
            metadata_str.as_deref(),
        )
        .await;
    }

    // KV fallback (always runs for read compatibility)
    let key = "audit:global";
    let mut entries = read_entries(kv, key).await;
    entries.push(entry);
    write_entries(kv, key, &entries, MAX_GLOBAL_AUDIT).await
}

/// Get audit entries for an event, newest first (up to `limit`).
pub async fn get_event_audit(
    kv: &KvStore,
    event_id: &str,
    limit: usize,
    d1: Option<&worker::D1Database>,
) -> Result<Vec<AuditEntry>, String> {
    // D1 path: try D1 first
    if let Some(db) = d1
        && let Ok(rows) = crate::db::get_audit_entries(db, event_id, limit).await
        && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .filter_map(|r| {
                let action: Option<AuditAction> =
                    serde_json::from_str(&format!("\"{}\"", r.action)).ok();
                Some(AuditEntry {
                    timestamp: r.timestamp,
                    actor: r.actor,
                    action: action?,
                    target: r.target,
                    description: r.description,
                    metadata: r.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .collect());
    }

    // KV fallback
    let key = format!("event:{event_id}:audit");
    let mut entries = read_entries(kv, &key).await;
    entries.reverse();
    Ok(entries.into_iter().take(limit).collect())
}

/// Get global audit entries, newest first (up to `limit`).
pub async fn get_global_audit(
    kv: &KvStore,
    limit: usize,
    d1: Option<&worker::D1Database>,
) -> Result<Vec<AuditEntry>, String> {
    // D1 path: try D1 first
    if let Some(db) = d1
        && let Ok(rows) = crate::db::get_global_audit_entries(db, limit).await
        && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .filter_map(|r| {
                let action: Option<AuditAction> =
                    serde_json::from_str(&format!("\"{}\"", r.action)).ok();
                Some(AuditEntry {
                    timestamp: r.timestamp,
                    actor: r.actor,
                    action: action?,
                    target: r.target,
                    description: r.description,
                    metadata: r.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .collect());
    }

    // KV fallback
    let mut entries = read_entries(kv, "audit:global").await;
    entries.reverse();
    Ok(entries.into_iter().take(limit).collect())
}
