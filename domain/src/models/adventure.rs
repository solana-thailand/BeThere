//! Adventure game domain models for the Rust Adventures feature.
//!
//! Stores per-event adventure configuration (levels) and per-attendee progress.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Adventure configuration stored per event in KV.
///
/// Key: `event:{id}:adventure:config`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdventureConfig {
    /// Whether the adventure is enabled for this event.
    #[serde(default)]
    pub enabled: bool,
    /// Required level index (0-based) that must be completed to pass.
    /// If None, all levels must be completed.
    #[serde(default)]
    pub required_level: Option<usize>,
}

/// Per-attendee adventure progress stored in KV.
///
/// Key: `event:{id}:adventure:progress:{claim_token}`
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdventureProgress {
    /// Claim token (from check-in).
    #[serde(default)]
    pub claim_token: String,
    /// IDs of completed levels.
    #[serde(default)]
    pub levels_completed: Vec<String>,
    /// Per-level scores.
    #[serde(default)]
    pub scores: HashMap<String, LevelScore>,
    /// Total moves across all levels.
    #[serde(default)]
    pub total_moves: u32,
    /// Total time in seconds across all levels.
    #[serde(default)]
    pub total_time_seconds: u32,
    /// Whether adventure quiz is passed (required levels done).
    #[serde(default)]
    pub passed: bool,
    /// Timestamp when adventure was passed.
    #[serde(default)]
    pub passed_at: Option<String>,
    /// Last played timestamp.
    #[serde(default)]
    pub last_played_at: Option<String>,
}

/// Score for a completed level.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelScore {
    /// Number of moves taken.
    pub moves: u32,
    /// Number of puzzles solved.
    pub puzzles_solved: u32,
    /// Time taken in seconds.
    pub time_seconds: u32,
    /// Star rating (1-3) based on performance.
    pub stars: u8,
}

/// Request to save level completion progress.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdventureSaveRequest {
    /// Claim token identifying the attendee.
    pub claim_token: String,
    /// ID of the level just completed.
    pub level_id: String,
    /// Score for this level.
    pub score: LevelScore,
}

/// Adventure status for a claim token.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdventureStatus {
    /// No adventure configured for this event.
    NotRequired,
    /// Adventure configured but not started.
    NotStarted,
    /// Adventure in progress (some levels done, not all required).
    InProgress,
    /// Adventure passed (all required levels completed).
    Passed,
}

/// Calculate star rating based on moves, puzzles solved, and time.
///
/// Rating logic:
/// - 3 stars: moves <= optimal * 1.5, time <= 120s per level
/// - 2 stars: moves <= optimal * 2.5, time <= 240s per level
/// - 1 star: completed (always at least 1 star for completing)
pub fn calculate_stars(moves: u32, puzzles_solved: u32, time_seconds: u32) -> u8 {
    // Base optimal: ~10 moves per puzzle + 5 per key
    let optimal_moves = puzzles_solved * 10 + 5;

    if moves <= optimal_moves * 3 / 2 && time_seconds <= 120 {
        3
    } else if moves <= optimal_moves * 5 / 2 && time_seconds <= 240 {
        2
    } else {
        1
    }
}
