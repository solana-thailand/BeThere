//! Adventure storage backed exclusively by D1 (Phase 3a complete — Issue #053).
//!
//! Adventure config and per-attendee progress live in D1 only.
//! KV fallback and dual-write have been removed.
//!
//! Key schema (D1):
//!   adventure_configs  — one row per event
//!   adventure_progress — one row per (event_id, claim_token)

use chrono::Utc;
use worker::D1Database;

use event_checkin_domain::models::adventure::{
    AdventureConfig, AdventureProgress, AdventureStatus, LevelScore,
};

use crate::db::adventure as d1_adventure;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Read adventure config from D1. Returns `None` if no adventure is configured.
pub async fn get_adventure_config(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<AdventureConfig>, String> {
    let json_str = d1_adventure::get_adventure_config_from_d1(db, event_id).await?;
    match json_str {
        Some(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| format!("failed to parse adventure config: {e}")),
        None => Ok(None),
    }
}

/// Write adventure config to D1.
pub async fn save_adventure_config(
    db: &D1Database,
    event_id: &str,
    config: &AdventureConfig,
) -> Result<(), String> {
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize adventure config: {e:?}"))?;

    d1_adventure::upsert_adventure_config_to_d1(db, event_id, &json_str, config.enabled).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

/// Read adventure progress for an attendee from D1. Returns `None` if not found.
pub async fn get_adventure_progress(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
) -> Result<Option<AdventureProgress>, String> {
    let json_str = d1_adventure::get_adventure_progress_from_d1(db, event_id, claim_token).await?;
    match json_str {
        Some(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| format!("failed to parse adventure progress: {e}")),
        None => Ok(None),
    }
}

/// Save adventure progress to D1.
async fn save_adventure_progress(
    db: &D1Database,
    event_id: &str,
    progress: &AdventureProgress,
) -> Result<(), String> {
    let json_str = serde_json::to_string(progress)
        .map_err(|e| format!("failed to serialize adventure progress: {e:?}"))?;

    d1_adventure::upsert_adventure_progress_to_d1(
        db,
        event_id,
        &progress.claim_token,
        &json_str,
        progress.passed,
        progress.total_moves,
        progress.total_time_seconds,
        progress.levels_completed.len() as u32,
        progress.last_played_at.as_deref(),
    )
    .await?;

    Ok(())
}

/// Save level completion and return updated progress.
///
/// Creates progress record if first time. Updates scores and checks if
/// adventure is now passed.
pub async fn save_level_completion(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
    level_id: &str,
    score: LevelScore,
    required_levels: &[String],
) -> Result<AdventureProgress, String> {
    let mut progress = get_adventure_progress(db, event_id, claim_token)
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
    if !required_levels.is_empty() {
        let all_done = required_levels.iter().all(|req| {
            progress.levels_completed.iter().any(|lid| {
                lid.strip_prefix(req)
                    .is_some_and(|rest| rest.starts_with('_') || rest.is_empty())
            })
        });
        if all_done && !progress.passed {
            progress.passed = true;
            progress.passed_at = Some(Utc::now().to_rfc3339());
        }
    }

    progress.last_played_at = Some(Utc::now().to_rfc3339());

    save_adventure_progress(db, event_id, &progress).await?;

    Ok(progress)
}

/// Determine adventure status for a claim token.
pub async fn get_adventure_status(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
) -> Result<AdventureStatus, String> {
    let config = get_adventure_config(db, event_id).await?;
    match config {
        None | Some(AdventureConfig { enabled: false, .. }) => Ok(AdventureStatus::NotRequired),
        Some(_config) => {
            let progress = get_adventure_progress(db, event_id, claim_token).await?;
            match progress {
                None => Ok(AdventureStatus::NotStarted),
                Some(p) if p.passed => Ok(AdventureStatus::Passed),
                Some(_) => Ok(AdventureStatus::InProgress),
            }
        }
    }
}
