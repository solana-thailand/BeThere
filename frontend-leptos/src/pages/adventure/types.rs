//! Core types for the Rust Adventures game engine.
//!
//! Defines tile types, level data, puzzle definitions, and game state.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// === Tile Types ===

/// A single tile in the game grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    /// Walkable floor
    Floor,
    /// Impassable wall
    Wall,
    /// Player start position (becomes Floor after spawn)
    PlayerStart,
    /// Level exit (walkable when level conditions met)
    Exit,
    /// Collectible Rust keyword key
    Key { name: String, description: String },
    /// NPC character (walkable, triggers dialog on bump)
    Npc { name: String, dialog: String },
    /// Gate blocked until puzzle solved
    Gate { puzzle_id: String },
    /// Code block (interactive, shows puzzle)
    CodeBlock { puzzle_id: String },
    /// Impassable water (visual variety)
    Water,
    /// Sign with read-only hint text
    Sign { text: String },
}

impl Tile {
    /// Whether the player can walk onto this tile.
    pub fn walkable(
        &self,
        solved_puzzles: &HashSet<String>,
        _collected_keys: &HashSet<String>,
    ) -> bool {
        match self {
            Tile::Floor | Tile::PlayerStart | Tile::Exit | Tile::Npc { .. } | Tile::Sign { .. } => {
                true
            }
            Tile::Wall | Tile::Water => false,
            Tile::Gate { puzzle_id } => solved_puzzles.contains(puzzle_id),
            Tile::CodeBlock { .. } => false, // interact, don't walk on
            Tile::Key { .. } => true,        // always walkable; collection is handled in apply_move
        }
    }

    /// Render character for the tile in the grid view.
    pub fn display_char(&self) -> &str {
        match self {
            Tile::Floor => "·",
            Tile::Wall => "█",
            Tile::PlayerStart => "·",
            Tile::Exit => "▶",
            Tile::Key { name, .. } => name, // show the key name
            Tile::Npc { .. } => "🦀",
            Tile::Gate { .. } => "▓",
            Tile::CodeBlock { .. } => "▒",
            Tile::Water => "~",
            Tile::Sign { .. } => "📋",
        }
    }

    /// Parse a tile from a grid string character.
    /// Extended format: k(name) for keys, n for NPC with metadata, etc.
    pub fn from_char(c: char) -> Self {
        match c {
            '.' => Tile::Floor,
            '#' => Tile::Wall,
            '@' => Tile::PlayerStart,
            '>' => Tile::Exit,
            '~' => Tile::Water,
            _ => Tile::Floor,
        }
    }
}

// === Position ===

/// Grid position (column, row) — 0-indexed.
pub type Position = (usize, usize);

// === Level Data ===

/// A collectible key in the level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDef {
    pub pos: Position,
    pub name: String,
    pub description: String,
}

/// An NPC in the level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcDef {
    pub pos: Position,
    pub name: String,
    pub dialog: String,
}

/// A gate blocked by a puzzle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDef {
    pub pos: Position,
    pub puzzle_id: String,
}

/// A sign with hint text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignDef {
    pub pos: Position,
    pub text: String,
}

/// Puzzle type definitions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PuzzleDef {
    /// Arrange code lines into correct order.
    Arrange {
        id: String,
        instruction: String,
        pieces: Vec<String>,
        solution: String,
        hint: String,
    },
    /// Fill in the blank with correct keyword.
    FillBlank {
        id: String,
        instruction: String,
        code_template: String,
        blank: String,
        options: Vec<String>,
        answer: String,
        hint: String,
    },
    /// Fix the error in the code.
    FixError {
        id: String,
        instruction: String,
        broken_code: String,
        options: Vec<String>,
        answer: String,
        hint: String,
    },
    /// Match items on left to items on right.
    MatchPairs {
        id: String,
        instruction: String,
        pairs: Vec<(String, String)>,
        hint: String,
    },
    /// Write a short expression.
    ShortAnswer {
        id: String,
        instruction: String,
        code_template: String,
        answer: String,
        hint: String,
    },
}

impl PuzzleDef {
    pub fn id(&self) -> &str {
        match self {
            PuzzleDef::Arrange { id, .. } => id,
            PuzzleDef::FillBlank { id, .. } => id,
            PuzzleDef::FixError { id, .. } => id,
            PuzzleDef::MatchPairs { id, .. } => id,
            PuzzleDef::ShortAnswer { id, .. } => id,
        }
    }

    pub fn hint(&self) -> &str {
        match self {
            PuzzleDef::Arrange { hint, .. } => hint,
            PuzzleDef::FillBlank { hint, .. } => hint,
            PuzzleDef::FixError { hint, .. } => hint,
            PuzzleDef::MatchPairs { hint, .. } => hint,
            PuzzleDef::ShortAnswer { hint, .. } => hint,
        }
    }
}

/// A complete level definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelData {
    pub id: String,
    pub name: String,
    pub concept: String,
    pub width: usize,
    pub height: usize,
    /// Raw grid characters — parsed into Vec<Vec<Tile>> at load time.
    /// '#' = wall, '.' = floor, '@' = player start, '>' = exit, '~' = water
    pub grid: Vec<String>,
    /// Key definitions with positions and descriptions.
    pub keys: Vec<KeyDef>,
    /// NPC definitions.
    pub npcs: Vec<NpcDef>,
    /// Gate definitions.
    pub gates: Vec<GateDef>,
    /// Sign definitions.
    pub signs: Vec<SignDef>,
    /// Puzzles in this level.
    pub puzzles: Vec<PuzzleDef>,
    /// Keys required to open the exit.
    pub required_keys: Vec<String>,
    /// Text shown when level starts.
    pub intro_text: String,
    /// Text shown when level is completed.
    pub completion_text: String,
}

impl LevelData {
    /// Find the player start position from the grid.
    pub fn find_player_start(&self) -> Option<Position> {
        for (row, line) in self.grid.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if ch == '@' {
                    return Some((col, row));
                }
            }
        }
        None
    }

    /// Build the tile grid from raw grid strings + metadata.
    pub fn build_tile_grid(&self) -> Vec<Vec<Tile>> {
        let mut grid: Vec<Vec<Tile>> = self
            .grid
            .iter()
            .map(|row| row.chars().map(Tile::from_char).collect())
            .collect();

        // Place keys
        for key_def in &self.keys {
            let (col, row) = key_def.pos;
            if row < grid.len() && col < grid[row].len() {
                grid[row][col] = Tile::Key {
                    name: key_def.name.clone(),
                    description: key_def.description.clone(),
                };
            }
        }

        // Place NPCs
        for npc_def in &self.npcs {
            let (col, row) = npc_def.pos;
            if row < grid.len() && col < grid[row].len() {
                grid[row][col] = Tile::Npc {
                    name: npc_def.name.clone(),
                    dialog: npc_def.dialog.clone(),
                };
            }
        }

        // Place gates
        for gate_def in &self.gates {
            let (col, row) = gate_def.pos;
            if row < grid.len() && col < grid[row].len() {
                grid[row][col] = Tile::Gate {
                    puzzle_id: gate_def.puzzle_id.clone(),
                };
            }
        }

        // Place signs
        for sign_def in &self.signs {
            let (col, row) = sign_def.pos;
            if row < grid.len() && col < grid[row].len() {
                grid[row][col] = Tile::Sign {
                    text: sign_def.text.clone(),
                };
            }
        }

        grid
    }
}

// === Game State ===

/// NPC dialog state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogState {
    pub npc_name: String,
    pub text: String,
}

/// Active puzzle state.
#[derive(Clone, Debug, PartialEq)]
pub struct PuzzleState {
    pub puzzle: PuzzleDef,
    pub input: String,
    /// For Arrange puzzles: ordered list of piece indices (current user arrangement).
    pub arrange_order: Vec<usize>,
    /// For MatchPairs: which pairs the user has matched (left_idx, right_idx).
    pub matched_pairs: Vec<(usize, usize)>,
    /// For MatchPairs: currently selected left item.
    pub selected_left: Option<usize>,
    /// For MatchPairs: shuffled display order for right-side items.
    /// Maps display index -> canonical pairs index.
    pub right_shuffle: Vec<usize>,
}

/// Score for a completed level.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelScore {
    pub moves: u32,
    pub puzzles_solved: u32,
    pub time_seconds: u32,
    /// Star rating (1-3) based on performance.
    pub stars: u8,
}

/// Result of a player movement attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveResult {
    /// Moved successfully to an empty floor tile.
    Moved,
    /// Moved and collected a key.
    CollectedKey { name: String, description: String },
    /// Moved onto NPC tile — dialog should show.
    NpcDialog { name: String, dialog: String },
    /// Moved onto sign tile — show text.
    SignText { text: String },
    /// Moved onto exit tile — level complete (if conditions met).
    ExitReached,
    /// Moved onto code block adjacent — puzzle should open.
    HitCodeBlock { puzzle_id: String },
    /// Hit a locked gate — open its associated puzzle.
    HitGate { puzzle_id: String },
    /// Blocked by wall, water.
    Blocked,
}

/// The main game state.
#[derive(Clone, Debug)]
pub struct GameState {
    /// Current level index.
    pub current_level: usize,
    /// Player position (col, row).
    pub player_pos: Position,
    /// The parsed tile grid.
    pub tile_grid: Vec<Vec<Tile>>,
    /// Keys collected in this level.
    pub collected_keys: HashSet<String>,
    /// Puzzles solved in this level.
    pub solved_puzzles: HashSet<String>,
    /// NPCs already talked to.
    pub talked_npcs: HashSet<Position>,
    /// Number of moves in current level.
    pub moves_count: u32,
    /// Active dialog (if any).
    pub active_dialog: Option<DialogState>,
    /// Active puzzle (if any).
    pub active_puzzle: Option<PuzzleState>,
    /// Whether level intro is showing.
    pub showing_intro: bool,
    /// Whether level is completed.
    pub level_completed: bool,
}
