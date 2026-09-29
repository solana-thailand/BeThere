//! Tile grid of the adventure page.

use leptos::prelude::*;

use super::types::*;

/// Every condition for the exit to be "unlocked" is met.
fn check_exit_unlocked(game: &GameState, levels: &[LevelData]) -> bool {
    if let Some(level) = levels.get(game.current_level) {
        let keys_ok = level
            .required_keys
            .iter()
            .all(|k| game.collected_keys.contains(k));
        let gates_ok = level
            .gates
            .iter()
            .all(|g| game.solved_puzzles.contains(&g.puzzle_id));
        keys_ok && gates_ok
    } else {
        false
    }
}

/// One `<div class="tile …">` per grid cell, player drawn on top.
pub(super) fn tile_views(
    g: &GameState,
    levels: &[LevelData],
    animating: Option<String>,
) -> impl IntoView + use<> {
    let grid = &g.tile_grid;
    let player_pos = g.player_pos;
    let collected = &g.collected_keys;
    let solved = &g.solved_puzzles;

    let mut tiles_out = Vec::new();
    for (row_idx, row) in grid.iter().enumerate() {
        for (col_idx, tile) in row.iter().enumerate() {
            let is_player = (col_idx, row_idx) == player_pos;
            let tile_class = match tile {
                Tile::Floor | Tile::PlayerStart => "tile-floor",
                Tile::Wall => "tile-wall",
                Tile::Exit => {
                    if check_exit_unlocked(g, levels) {
                        "tile-exit tile-exit-unlocked"
                    } else {
                        "tile-exit"
                    }
                }
                Tile::Key { name, .. } => {
                    if collected.contains(name) {
                        "tile-floor"
                    } else {
                        "tile-key"
                    }
                }
                Tile::Npc { .. } => "tile-npc",
                Tile::Gate { puzzle_id } => {
                    if solved.contains(puzzle_id) {
                        "tile-gate-open"
                    } else if animating.as_deref() == Some(puzzle_id.as_str()) {
                        "tile-gate tile-gate-animating"
                    } else {
                        "tile-gate"
                    }
                }
                Tile::CodeBlock { .. } => "tile-code",
                Tile::Water => "tile-water",
                Tile::Sign { .. } => "tile-sign",
            };

            let display = if is_player {
                "🦀".to_string()
            } else {
                match tile {
                    Tile::Floor | Tile::PlayerStart => String::new(),
                    Tile::Wall => String::new(),
                    Tile::Exit => "🚪".to_string(),
                    Tile::Key { name, .. } if !collected.contains(name) => name.clone(),
                    Tile::Gate { puzzle_id } if !solved.contains(puzzle_id) => "🔒".to_string(),
                    _ => tile.display_char().to_string(),
                }
            };

            let class = if is_player {
                format!("tile {tile_class} tile-player")
            } else {
                format!("tile {tile_class}")
            };
            tiles_out.push(view! {
                <div class={class}>{display}</div>
            });
        }
    }
    tiles_out.collect_view()
}
