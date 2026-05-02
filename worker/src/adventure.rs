//! KV-based adventure storage for the Rust Adventures feature.
//!
//! Key schema:
//!   "event:{id}:adventure:config"              → AdventureConfig (JSON)
//!   "event:{id}:adventure:progress:{token}"    → AdventureProgress (JSON)

use chrono::Utc;
use worker::KvStore;

use event_checkin_domain::models::adventure::{
    AdventureConfig, AdventureProgress, AdventureStatus, LevelScore,
};

// Key helpers
fn adventure_config_key(event_id: &str) -> String {
    format!("event:{event_id}:adventure:config")
}

fn adventure_progress_key(event_id: &str, claim_token: &str) -> String {
    format!("event:{event_id}:adventure:progress:{claim_token}")
}

// --- Config ---

/// Read adventure config from KV.
pub async fn get_adventure_config(
    kv: &KvStore,
    event_id: &str,
) -> Result<Option<AdventureConfig>, String> {
    let key = adventure_config_key(event_id);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read adventure config from KV: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json_str) => serde_json::from_str(&json_str)
            .map(Some)
            .map_err(|e| format!("failed to parse adventure config: {e}")),
    }
}

/// Write adventure config to KV.
pub async fn save_adventure_config(
    kv: &KvStore,
    event_id: &str,
    config: &AdventureConfig,
) -> Result<(), String> {
    let key = adventure_config_key(event_id);
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize adventure config: {e:?}"))?;
    kv.put(&key, &json_str)
        .map_err(|e| format!("failed to build adventure config put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write adventure config to KV: {e:?}"))
}

// --- Progress ---

/// Read adventure progress for an attendee.
pub async fn get_adventure_progress(
    kv: &KvStore,
    event_id: &str,
    claim_token: &str,
) -> Result<Option<AdventureProgress>, String> {
    let key = adventure_progress_key(event_id, claim_token);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read adventure progress from KV: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json_str) => serde_json::from_str(&json_str)
            .map(Some)
            .map_err(|e| format!("failed to parse adventure progress: {e}")),
    }
}

/// Save adventure progress.
async fn save_adventure_progress(
    kv: &KvStore,
    event_id: &str,
    progress: &AdventureProgress,
) -> Result<(), String> {
    let key = adventure_progress_key(event_id, &progress.claim_token);
    let json_str = serde_json::to_string(progress)
        .map_err(|e| format!("failed to serialize adventure progress: {e:?}"))?;
    kv.put(&key, &json_str)
        .map_err(|e| format!("failed to build adventure progress put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write adventure progress to KV: {e:?}"))
}

/// Save level completion and return updated progress.
///
/// Creates progress record if first time. Updates scores and checks if
/// adventure is now passed.
pub async fn save_level_completion(
    kv: &KvStore,
    event_id: &str,
    claim_token: &str,
    level_id: &str,
    score: LevelScore,
    required_levels: &[String],
) -> Result<AdventureProgress, String> {
    let mut progress = get_adventure_progress(kv, event_id, claim_token)
        .await?
        .unwrap_or_else(|| AdventureProgress {
            claim_token: claim_token.to_string(),
            ..Default::default()
        });

    // Add level to completed if not already there
    if !progress.levels_completed.iter().any(|id| id == level_id) {
        progress.levels_completed.push(level_id.to_string());
    }

    // Update score (keep best)
    let existing = progress.scores.get(level_id);
    let best_score = match existing {
        Some(prev) if prev.stars >= score.stars => prev.clone(),
        _ => score,
    };
    progress.scores.insert(level_id.to_string(), best_score);

    // Recalculate totals
    progress.total_moves = progress.scores.values().map(|s| s.moves).sum();
    progress.total_time_seconds = progress.scores.values().map(|s| s.time_seconds).sum();

    // Check if adventure is now passed.
    // If required_levels is non-empty, the attendee must complete those specific levels.
    // If empty (config exists but no required_level set), mark passed when any level is completed
    // as a fallback — the get_adventure_status function re-validates against config.
    if !required_levels.is_empty() {
        let all_done = required_levels
            .iter()
            .all(|id| progress.levels_completed.iter().any(|lid| lid == id));
        if all_done && !progress.passed {
            progress.passed = true;
            progress.passed_at = Some(Utc::now().to_rfc3339());
        }
    } else if !progress.levels_completed.is_empty() && !progress.passed {
        // No specific required_levels list — mark passed when at least one level is done.
        // This covers the case where adventure is enabled but required_level is None.
        progress.passed = true;
        progress.passed_at = Some(Utc::now().to_rfc3339());
    }

    progress.last_played_at = Some(Utc::now().to_rfc3339());

    save_adventure_progress(kv, event_id, &progress).await?;

    Ok(progress)
}

/// Determine adventure status for a claim token.
pub async fn get_adventure_status(
    kv: &KvStore,
    event_id: &str,
    claim_token: &str,
) -> Result<AdventureStatus, String> {
    let config = get_adventure_config(kv, event_id).await?;
    match config {
        None | Some(AdventureConfig { enabled: false, .. }) => Ok(AdventureStatus::NotRequired),
        Some(_config) => {
            let progress = get_adventure_progress(kv, event_id, claim_token).await?;
            match progress {
                None => Ok(AdventureStatus::NotStarted),
                Some(p) if p.passed => Ok(AdventureStatus::Passed),
                Some(_) => Ok(AdventureStatus::InProgress),
            }
        }
    }
}
