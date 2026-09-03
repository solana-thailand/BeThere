//! Adventure configuration and per-attendee progress.

use serde::{Deserialize, Serialize};

use crate::pages::adventure::types::*;

/// Adventure configuration (stored in KV per event).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdventureConfig {
    /// List of levels for this event's adventure.
    pub levels: Vec<LevelData>,
}

/// Adventure progress for a user (stored in KV).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdventureProgress {
    /// User email (from auth).
    #[serde(default)]
    pub user_id: String,
    /// Claim token (if playing from claim flow).
    #[serde(default)]
    pub claim_token: Option<String>,
    /// IDs of completed levels.
    #[serde(default)]
    pub levels_completed: Vec<String>,
    /// All keys ever collected.
    #[serde(default)]
    pub total_keys_collected: Vec<String>,
    /// Per-level scores.
    #[serde(default)]
    pub scores: std::collections::HashMap<String, LevelScore>,
    /// Last played timestamp.
    #[serde(default)]
    pub last_played_at: Option<String>,
}

impl AdventureProgress {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            ..Default::default()
        }
    }

    pub fn is_level_completed(&self, level_id: &str) -> bool {
        self.levels_completed.iter().any(|id| id == level_id)
    }
}
