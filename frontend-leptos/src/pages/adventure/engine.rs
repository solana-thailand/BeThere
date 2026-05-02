//! Game engine — movement, collision detection, key collection, puzzle mechanics.

use super::types::*;
use std::collections::HashSet;

/// Direction the player can move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// Calculate the new position after moving one tile in this direction.
    pub fn apply(&self, pos: Position) -> Option<Position> {
        let (col, row) = pos;
        match self {
            Direction::Up => row.checked_sub(1).map(|r| (col, r)),
            Direction::Down => Some((col, row + 1)),
            Direction::Left => col.checked_sub(1).map(|c| (c, row)),
            Direction::Right => Some((col + 1, row)),
        }
    }
}

/// Try to move the player in the given direction.
///
/// Returns a `MoveResult` describing what happened. Does NOT mutate state —
/// the caller (Leptos signal update) applies the changes.
pub fn try_move(game: &GameState, direction: Direction) -> MoveResult {
    let new_pos = match direction.apply(game.player_pos) {
        Some(pos) => pos,
        None => return MoveResult::Blocked,
    };

    // Bounds check
    let (new_col, new_row) = new_pos;
    if new_row >= game.tile_grid.len() || new_col >= game.tile_grid[new_row].len() {
        return MoveResult::Blocked;
    }

    let target_tile = &game.tile_grid[new_row][new_col];

    // Check if tile is walkable
    if !target_tile.walkable(&game.solved_puzzles, &game.collected_keys) {
        // Special case: if it's a code block, show puzzle hint
        if let Tile::CodeBlock { puzzle_id } = target_tile {
            return MoveResult::HitCodeBlock {
                puzzle_id: puzzle_id.clone(),
            };
        }
        // Special case: if it's a locked gate, open its puzzle
        if let Tile::Gate { puzzle_id } = target_tile {
            if !game.solved_puzzles.contains(puzzle_id) {
                return MoveResult::HitGate {
                    puzzle_id: puzzle_id.clone(),
                };
            }
        }
        return MoveResult::Blocked;
    }

    // Determine what happens at the target tile
    match target_tile {
        Tile::Floor | Tile::PlayerStart => MoveResult::Moved,
        Tile::Key { name, description } => {
            if game.collected_keys.contains(name) {
                MoveResult::Moved // already collected, treat as floor
            } else {
                MoveResult::CollectedKey {
                    name: name.clone(),
                    description: description.clone(),
                }
            }
        }
        Tile::Npc { name, dialog } => {
            // Only show dialog if not already talked to this NPC
            if game.talked_npcs.contains(&new_pos) {
                MoveResult::Moved
            } else {
                MoveResult::NpcDialog {
                    name: name.clone(),
                    dialog: dialog.clone(),
                }
            }
        }
        Tile::Sign { text } => MoveResult::SignText { text: text.clone() },
        Tile::Exit => {
            // Check if all required keys are collected
            // (The level data's required_keys are checked against collected_keys)
            MoveResult::ExitReached
        }
        // Gate already opened (puzzle solved) — treat as floor
        Tile::Gate { .. } => MoveResult::Moved,
        Tile::CodeBlock { .. } => MoveResult::Blocked,
        Tile::Wall | Tile::Water => MoveResult::Blocked,
    }
}

/// Apply a move result to the game state, returning the updated state.
///
/// This is a pure function — takes current state, returns new state.
/// The caller wraps this in Leptos signal updates.
pub fn apply_move(mut game: GameState, direction: Direction) -> (GameState, MoveResult) {
    let result = try_move(&game, direction);

    match &result {
        MoveResult::Moved
        | MoveResult::NpcDialog { .. }
        | MoveResult::SignText { .. }
        | MoveResult::ExitReached => {
            if let Some(new_pos) = direction.apply(game.player_pos) {
                game.player_pos = new_pos;
                game.moves_count += 1;
            }
        }
        MoveResult::CollectedKey {
            name,
            description: _,
        } => {
            if let Some(new_pos) = direction.apply(game.player_pos) {
                game.player_pos = new_pos;
                game.collected_keys.insert(name.clone());
                game.moves_count += 1;
            }
        }
        MoveResult::HitCodeBlock { .. } | MoveResult::HitGate { .. } => {
            // Don't move, just signal the puzzle
        }
        MoveResult::Blocked => {
            // Don't move
        }
    }

    // Handle NPC dialog
    if let MoveResult::NpcDialog { .. } = &result {
        let (col, row) = game.player_pos;
        if let Tile::Npc { name, dialog } = &game.tile_grid[row][col] {
            game.talked_npcs.insert(game.player_pos);
            game.active_dialog = Some(DialogState {
                npc_name: name.clone(),
                text: dialog.clone(),
            });
        }
    }

    // Handle sign
    if let MoveResult::SignText { text } = &result {
        game.active_dialog = Some(DialogState {
            npc_name: "📋 Sign".to_string(),
            text: text.clone(),
        });
    }

    (game, result)
}

/// Check if the level is complete.
///
/// Level is complete when:
/// 1. Player is on the exit tile
/// 2. All required keys are collected
/// 3. All gates have been opened (puzzles solved)
pub fn check_level_complete(game: &GameState, level: &LevelData) -> bool {
    // Must be on exit
    let (col, row) = game.player_pos;
    if !matches!(
        game.tile_grid.get(row).and_then(|r| r.get(col)),
        Some(Tile::Exit)
    ) {
        return false;
    }

    // Must have all required keys
    for key_name in &level.required_keys {
        if !game.collected_keys.contains(key_name) {
            return false;
        }
    }

    // Must have solved all gate puzzles
    for gate in &level.gates {
        if !game.solved_puzzles.contains(&gate.puzzle_id) {
            return false;
        }
    }

    true
}

/// Initialize game state for a level.
pub fn init_game_state(level: &LevelData) -> GameState {
    let player_pos = level.find_player_start().unwrap_or((1, 1));
    let tile_grid = level.build_tile_grid();

    GameState {
        current_level: 0, // caller sets this
        player_pos,
        tile_grid,
        collected_keys: HashSet::new(),
        solved_puzzles: HashSet::new(),
        talked_npcs: HashSet::new(),
        moves_count: 0,
        active_dialog: None,
        active_puzzle: None,
        showing_intro: true,
        level_completed: false,
    }
}

/// Dismiss the active dialog.
pub fn dismiss_dialog(mut game: GameState) -> GameState {
    game.active_dialog = None;
    game
}

/// Open a puzzle by its ID.
pub fn open_puzzle(mut game: GameState, levels: &[LevelData]) -> GameState {
    // Find the puzzle in the current level
    if let Some(level) = levels.get(game.current_level) {
        // Check if there's a puzzle adjacent to the player or on the tile they just hit
        // For now, find unsolved puzzles in the level
        for puzzle in &level.puzzles {
            let puzzle_id = puzzle.id().to_string();
            if !game.solved_puzzles.contains(&puzzle_id) {
                let arrange_order = match puzzle {
                    PuzzleDef::Arrange { pieces, .. } => init_arrange_puzzle(pieces.len()),
                    _ => Vec::new(),
                };
                let right_shuffle = match puzzle {
                    PuzzleDef::MatchPairs { pairs, .. } => init_right_shuffle(pairs.len()),
                    _ => Vec::new(),
                };
                game.active_puzzle = Some(PuzzleState {
                    puzzle: puzzle.clone(),
                    input: String::new(),
                    arrange_order,
                    matched_pairs: Vec::new(),
                    selected_left: None,
                    right_shuffle,
                });
                return game;
            }
        }
    }
    game
}

/// Open a specific puzzle by ID.
pub fn open_puzzle_by_id(mut game: GameState, puzzle_id: &str, levels: &[LevelData]) -> GameState {
    if let Some(level) = levels.get(game.current_level) {
        for puzzle in &level.puzzles {
            if puzzle.id() == puzzle_id && !game.solved_puzzles.contains(puzzle_id) {
                let arrange_order = match puzzle {
                    PuzzleDef::Arrange { pieces, .. } => init_arrange_puzzle(pieces.len()),
                    _ => Vec::new(),
                };
                let right_shuffle = match puzzle {
                    PuzzleDef::MatchPairs { pairs, .. } => init_right_shuffle(pairs.len()),
                    _ => Vec::new(),
                };
                game.active_puzzle = Some(PuzzleState {
                    puzzle: puzzle.clone(),
                    input: String::new(),
                    arrange_order,
                    matched_pairs: Vec::new(),
                    selected_left: None,
                    right_shuffle,
                });
                return game;
            }
        }
    }
    game
}

/// Check a puzzle answer.
pub fn check_puzzle_answer(game: &GameState) -> bool {
    if let Some(puzzle_state) = &game.active_puzzle {
        match &puzzle_state.puzzle {
            PuzzleDef::Arrange {
                pieces, solution, ..
            } => {
                let ordered: Vec<&str> = puzzle_state
                    .arrange_order
                    .iter()
                    .map(|&i| pieces.get(i).map(|s| s.as_str()).unwrap_or(""))
                    .collect();
                let user_answer = ordered.join("\n");
                let normalize =
                    |s: &str| s.lines().map(|l| l.trim()).collect::<Vec<_>>().join("\n");
                normalize(&user_answer) == normalize(solution)
            }
            PuzzleDef::FillBlank { answer, .. } => puzzle_state.input.trim() == answer,
            PuzzleDef::FixError { answer, .. } => puzzle_state.input.trim() == answer,
            PuzzleDef::ShortAnswer { answer, .. } => puzzle_state.input.trim() == answer,
            PuzzleDef::MatchPairs { pairs, .. } => {
                // All pairs must be matched and correct (left_idx == right_idx)
                puzzle_state.matched_pairs.len() == pairs.len()
                    && puzzle_state.matched_pairs.iter().all(|(l, r)| l == r)
            }
        }
    } else {
        false
    }
}

/// Submit puzzle answer — returns updated state and whether correct.
pub fn submit_puzzle(mut game: GameState) -> (GameState, bool) {
    let correct = check_puzzle_answer(&game);
    if correct {
        if let Some(puzzle_state) = &game.active_puzzle {
            let puzzle_id = puzzle_state.puzzle.id().to_string();
            game.solved_puzzles.insert(puzzle_id);
        }
        game.active_puzzle = None;
    }
    (game, correct)
}

/// Update puzzle input text.
pub fn update_puzzle_input(mut game: GameState, input: String) -> GameState {
    if let Some(puzzle_state) = &mut game.active_puzzle {
        puzzle_state.input = input;
    }
    game
}

/// Dismiss the puzzle (close without solving).
pub fn dismiss_puzzle(mut game: GameState) -> GameState {
    game.active_puzzle = None;
    game
}

// === Arrange Puzzle Functions ===

/// Initialize arrange puzzle: shuffle piece indices.
pub fn init_arrange_puzzle(piece_count: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..piece_count).collect();
    // Deterministic shuffle: reverse, then ensure it's not already sorted
    indices.reverse();
    if is_sorted(&indices) {
        if indices.len() > 1 {
            indices.swap(0, 1);
        }
    }
    indices
}

/// Move an arrange piece from one position to another.
pub fn move_arrange_piece(game: &mut GameState, from_idx: usize, to_idx: usize) {
    if let Some(puzzle_state) = &mut game.active_puzzle {
        if from_idx < puzzle_state.arrange_order.len() && to_idx < puzzle_state.arrange_order.len()
        {
            let item = puzzle_state.arrange_order.remove(from_idx);
            puzzle_state.arrange_order.insert(to_idx, item);
        }
    }
}

/// Move arrange piece up one position.
pub fn arrange_piece_up(game: &mut GameState, idx: usize) {
    if idx > 0 {
        move_arrange_piece(game, idx, idx - 1);
    }
}

/// Move arrange piece down one position.
pub fn arrange_piece_down(game: &mut GameState, idx: usize) {
    if let Some(puzzle_state) = &game.active_puzzle {
        if idx + 1 < puzzle_state.arrange_order.len() {
            move_arrange_piece(game, idx, idx + 1);
        }
    }
}

/// Check if arrange order matches solution (indices in ascending order).
fn is_sorted(indices: &[usize]) -> bool {
    indices.windows(2).all(|w| w[0] < w[1])
}

// === Match Pairs Functions ===

/// Select a left item for match pairs.
pub fn select_match_left(game: &mut GameState, left_idx: usize) {
    if let Some(puzzle_state) = &mut game.active_puzzle {
        puzzle_state.selected_left = Some(left_idx);
    }
}

/// Try to match a right item with the selected left item.
///
/// `right_display_idx` is the display position (0-based) in the shuffled right column.
/// Returns `Some(true)` if correct, `Some(false)` if wrong, `None` if invalid.
pub fn try_match_pair(game: &mut GameState, right_display_idx: usize) -> Option<bool> {
    if let Some(puzzle_state) = &mut game.active_puzzle {
        // Map display index to canonical pairs index
        let right_idx = puzzle_state
            .right_shuffle
            .get(right_display_idx)
            .copied()
            .unwrap_or(right_display_idx);

        if let Some(left_idx) = puzzle_state.selected_left {
            puzzle_state.selected_left = None;
            // Already matched this left item
            if puzzle_state
                .matched_pairs
                .iter()
                .any(|(l, _)| *l == left_idx)
            {
                return None;
            }
            // Already matched this right item
            if puzzle_state
                .matched_pairs
                .iter()
                .any(|(_, r)| *r == right_idx)
            {
                return None;
            }
            // Correct if left_idx == right_idx (canonical pairing)
            if left_idx == right_idx {
                puzzle_state.matched_pairs.push((left_idx, right_idx));
                return Some(true);
            } else {
                return Some(false);
            }
        }
    }
    None
}

/// Initialize shuffled display order for right-side items in MatchPairs.
/// Returns a permutation of `0..count` that is not identity.
pub fn init_right_shuffle(count: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..count).collect();
    // Deterministic shuffle: reverse the order
    indices.reverse();
    // If still sorted (e.g., 0 or 1 elements), swap first two
    if is_sorted(&indices) && indices.len() > 1 {
        indices.swap(0, 1);
    }
    indices
}

/// Calculate star rating (1-3) based on performance.
///
/// - 3 stars: moves <= optimal * 1.5 and time <= 120s
/// - 2 stars: moves <= optimal * 2.5 and time <= 240s
/// - 1 star: completed (always at least 1)
pub fn calculate_stars(moves: u32, puzzles_solved: u32, time_seconds: u32) -> u8 {
    let optimal_moves = puzzles_solved * 10 + 5;
    if moves <= optimal_moves * 3 / 2 && time_seconds <= 120 {
        3
    } else if moves <= optimal_moves * 5 / 2 && time_seconds <= 240 {
        2
    } else {
        1
    }
}
