use crate::pages::adventure::levels::*;
use crate::pages::adventure::types::*;
use std::collections::HashSet;

// === Grid & Structural Validation ===

#[test]
fn all_10_levels_exist() {
    let levels = default_levels();
    assert_eq!(levels.len(), 10, "Expected exactly 10 levels");
}

#[test]
fn each_level_has_valid_grid_dimensions() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        assert_eq!(
            level.grid.len(),
            level.height,
            "Level {} ({}): grid row count {} != height {}",
            i + 1,
            level.id,
            level.grid.len(),
            level.height
        );
        for (row_idx, row) in level.grid.iter().enumerate() {
            assert_eq!(
                row.len(),
                level.width,
                "Level {} ({}): row {} has {} cols, expected {}",
                i + 1,
                level.id,
                row_idx,
                row.len(),
                level.width
            );
        }
    }
}

#[test]
fn each_level_has_player_start() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        assert!(
            level.find_player_start().is_some(),
            "Level {} ({}) has no player start '@'",
            i + 1,
            level.id
        );
    }
}

#[test]
fn each_level_has_exit() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        let has_exit = level.grid.iter().any(|row| row.contains('>'));
        assert!(has_exit, "Level {} ({}) has no exit '>'", i + 1, level.id);
    }
}

#[test]
fn keys_within_grid_bounds() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for (ki, key) in level.keys.iter().enumerate() {
            let (col, row) = key.pos;
            assert!(
                row < level.height && col < level.width,
                "Level {} ({}): key[{}] '{}' at ({},{}) out of bounds ({},{})",
                i + 1,
                level.id,
                ki,
                key.name,
                col,
                row,
                level.width,
                level.height
            );
        }
    }
}

#[test]
fn npcs_within_grid_bounds() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for (ni, npc) in level.npcs.iter().enumerate() {
            let (col, row) = npc.pos;
            assert!(
                row < level.height && col < level.width,
                "Level {} ({}): npc[{}] '{}' at ({},{}) out of bounds ({},{})",
                i + 1,
                level.id,
                ni,
                npc.name,
                col,
                row,
                level.width,
                level.height
            );
        }
    }
}

#[test]
fn gates_within_grid_bounds() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for (gi, gate) in level.gates.iter().enumerate() {
            let (col, row) = gate.pos;
            assert!(
                row < level.height && col < level.width,
                "Level {} ({}): gate[{}] '{}' at ({},{}) out of bounds ({},{})",
                i + 1,
                level.id,
                gi,
                gate.puzzle_id,
                col,
                row,
                level.width,
                level.height
            );
        }
    }
}

#[test]
fn signs_within_grid_bounds() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for (si, sign) in level.signs.iter().enumerate() {
            let (col, row) = sign.pos;
            assert!(
                row < level.height && col < level.width,
                "Level {} ({}): sign[{}] at ({},{}) out of bounds ({},{})",
                i + 1,
                level.id,
                si,
                col,
                row,
                level.width,
                level.height
            );
        }
    }
}

#[test]
fn required_keys_exist_as_key_defs() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        let key_names: HashSet<&str> = level.keys.iter().map(|k| k.name.as_str()).collect();
        for rk in &level.required_keys {
            assert!(
                key_names.contains(rk.as_str()),
                "Level {} ({}): required key '{}' not found in keys list",
                i + 1,
                level.id,
                rk
            );
        }
    }
}

#[test]
fn gate_puzzle_ids_reference_existing_puzzles() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        let puzzle_ids: HashSet<&str> = level.puzzles.iter().map(|p| p.id()).collect();
        for gate in &level.gates {
            assert!(
                puzzle_ids.contains(gate.puzzle_id.as_str()),
                "Level {} ({}): gate references puzzle '{}' which doesn't exist",
                i + 1,
                level.id,
                gate.puzzle_id
            );
        }
    }
}

#[test]
fn keys_not_on_wall_tiles() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for (ki, key) in level.keys.iter().enumerate() {
            let (col, row) = key.pos;
            let ch = level.grid[row].chars().nth(col).unwrap();
            assert!(
                ch != '#',
                "Level {} ({}): key[{}] '{}' placed on wall at ({},{})",
                i + 1,
                level.id,
                ki,
                key.name,
                col,
                row
            );
        }
    }
}

#[test]
fn npcs_not_on_wall_tiles() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for (ni, npc) in level.npcs.iter().enumerate() {
            let (col, row) = npc.pos;
            let ch = level.grid[row].chars().nth(col).unwrap();
            assert!(
                ch != '#',
                "Level {} ({}): npc[{}] '{}' placed on wall at ({},{})",
                i + 1,
                level.id,
                ni,
                npc.name,
                col,
                row
            );
        }
    }
}

#[test]
fn no_duplicate_puzzle_ids_per_level() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        let mut seen = HashSet::new();
        for puzzle in &level.puzzles {
            let id = puzzle.id();
            assert!(
                seen.insert(id),
                "Level {} ({}): duplicate puzzle id '{}'",
                i + 1,
                level.id,
                id
            );
        }
    }
}

#[test]
fn no_duplicate_key_names_per_level() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        let mut seen = HashSet::new();
        for key in &level.keys {
            assert!(
                seen.insert(key.name.as_str()),
                "Level {} ({}): duplicate key name '{}'",
                i + 1,
                level.id,
                key.name
            );
        }
    }
}

// === Puzzle Solution Verification ===

#[test]
fn arrange_puzzle_solutions_are_valid() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for puzzle in &level.puzzles {
            if let PuzzleDef::Arrange {
                pieces, solution, ..
            } = puzzle
            {
                let solution_lines: Vec<&str> = solution.lines().collect();
                assert_eq!(
                    solution_lines.len(),
                    pieces.len(),
                    "Level {} ({}): arrange '{}' — solution has {} lines but {} pieces",
                    i + 1,
                    level.id,
                    puzzle.id(),
                    solution_lines.len(),
                    pieces.len()
                );

                // Every solution line must come from a piece
                for line in &solution_lines {
                    let trimmed = line.trim();
                    let found = pieces.iter().any(|p| p.trim() == trimmed);
                    assert!(
                        found,
                        "Level {} ({}): arrange '{}' — solution line '{}' not found in pieces",
                        i + 1,
                        level.id,
                        puzzle.id(),
                        trimmed
                    );
                }
            }
        }
    }
}

#[test]
fn fill_blank_answers_among_options() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for puzzle in &level.puzzles {
            if let PuzzleDef::FillBlank {
                answer, options, ..
            } = puzzle
            {
                assert!(
                    options.contains(answer),
                    "Level {} ({}): fill_blank '{}' — answer '{}' not in options",
                    i + 1,
                    level.id,
                    puzzle.id(),
                    answer
                );
            }
        }
    }
}

#[test]
fn fix_error_answers_among_options() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for puzzle in &level.puzzles {
            if let PuzzleDef::FixError {
                answer, options, ..
            } = puzzle
            {
                assert!(
                    options.contains(answer),
                    "Level {} ({}): fix_error '{}' — answer '{}' not in options",
                    i + 1,
                    level.id,
                    puzzle.id(),
                    answer
                );
            }
        }
    }
}

#[test]
fn short_answer_puzzles_have_answers() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for puzzle in &level.puzzles {
            if let PuzzleDef::ShortAnswer { answer, .. } = puzzle {
                assert!(
                    !answer.trim().is_empty(),
                    "Level {} ({}): short_answer '{}' has empty answer",
                    i + 1,
                    level.id,
                    puzzle.id()
                );
            }
        }
    }
}

#[test]
fn match_pairs_have_valid_pairs() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for puzzle in &level.puzzles {
            if let PuzzleDef::MatchPairs { pairs, .. } = puzzle {
                assert!(
                    pairs.len() >= 2,
                    "Level {} ({}): match_pairs '{}' has only {} pairs (need >= 2)",
                    i + 1,
                    level.id,
                    puzzle.id(),
                    pairs.len()
                );
            }
        }
    }
}

#[test]
fn puzzle_hints_not_empty() {
    let levels = default_levels();
    for (i, level) in levels.iter().enumerate() {
        for puzzle in &level.puzzles {
            assert!(
                !puzzle.hint().is_empty(),
                "Level {} ({}): puzzle '{}' has empty hint",
                i + 1,
                level.id,
                puzzle.id()
            );
        }
    }
}
