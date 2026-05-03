#[cfg(test)]
mod playtest {
    use crate::pages::adventure::engine;
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

    // === Engine Simulation ===

    #[test]
    fn init_game_state_finds_player_start() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let state = engine::init_game_state(level);
            assert_eq!(
                state.player_pos,
                level.find_player_start().unwrap(),
                "Level {} ({}): init player_pos mismatch",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn init_game_state_shows_intro() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let state = engine::init_game_state(level);
            assert!(
                state.showing_intro,
                "Level {} ({}): intro not showing",
                i + 1,
                level.id
            );
            assert!(
                !state.level_completed,
                "Level {} ({}): should not be completed",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn player_can_move_from_start() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let state = engine::init_game_state(level);
            let dirs = [
                engine::Direction::Up,
                engine::Direction::Down,
                engine::Direction::Left,
                engine::Direction::Right,
            ];
            let can_move = dirs
                .iter()
                .any(|d| !matches!(engine::try_move(&state, *d), MoveResult::Blocked));
            assert!(
                can_move,
                "Level {} ({}): player stuck at start",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn build_tile_grid_dimensions_match() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let grid = level.build_tile_grid();
            assert_eq!(
                grid.len(),
                level.height,
                "Level {} ({}) grid rows",
                i + 1,
                level.id
            );
            for (r, row) in grid.iter().enumerate() {
                assert_eq!(
                    row.len(),
                    level.width,
                    "Level {} ({}) grid row {} cols",
                    i + 1,
                    level.id,
                    r
                );
            }
        }
    }

    #[test]
    fn keys_placed_correctly_in_tile_grid() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let grid = level.build_tile_grid();
            for key in &level.keys {
                let (col, row) = key.pos;
                match &grid[row][col] {
                    Tile::Key { name, .. } => assert_eq!(
                        name,
                        &key.name,
                        "Level {} ({}): key name mismatch at ({},{})",
                        i + 1,
                        level.id,
                        col,
                        row
                    ),
                    other => panic!(
                        "Level {} ({}): expected Key at ({},{}) got {:?}",
                        i + 1,
                        level.id,
                        col,
                        row,
                        other
                    ),
                }
            }
        }
    }

    #[test]
    fn npcs_placed_correctly_in_tile_grid() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let grid = level.build_tile_grid();
            for npc in &level.npcs {
                let (col, row) = npc.pos;
                match &grid[row][col] {
                    Tile::Npc { name, .. } => assert_eq!(
                        name,
                        &npc.name,
                        "Level {} ({}): npc name mismatch at ({},{})",
                        i + 1,
                        level.id,
                        col,
                        row
                    ),
                    other => panic!(
                        "Level {} ({}): expected Npc at ({},{}) got {:?}",
                        i + 1,
                        level.id,
                        col,
                        row,
                        other
                    ),
                }
            }
        }
    }

    #[test]
    fn gates_placed_correctly_in_tile_grid() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let grid = level.build_tile_grid();
            for gate in &level.gates {
                let (col, row) = gate.pos;
                match &grid[row][col] {
                    Tile::Gate { puzzle_id } => assert_eq!(
                        puzzle_id,
                        &gate.puzzle_id,
                        "Level {} ({}): gate puzzle_id mismatch at ({},{})",
                        i + 1,
                        level.id,
                        col,
                        row
                    ),
                    other => panic!(
                        "Level {} ({}): expected Gate at ({},{}) got {:?}",
                        i + 1,
                        level.id,
                        col,
                        row,
                        other
                    ),
                }
            }
        }
    }

    // === BFS Reachability ===

    fn flood_fill(
        start: Position,
        tile_grid: &[Vec<Tile>],
        solved_puzzles: &HashSet<String>,
        collected_keys: &HashSet<String>,
    ) -> HashSet<Position> {
        let mut visited = HashSet::new();
        let mut queue = vec![start];
        visited.insert(start);
        while let Some((col, row)) = queue.pop() {
            let neighbors: Vec<Position> = [
                col.checked_sub(1).map(|c| (c, row)),
                Some((col + 1, row)),
                row.checked_sub(1).map(|r| (col, r)),
                Some((col, row + 1)),
            ]
            .into_iter()
            .flatten()
            .collect();
            for (nc, nr) in neighbors {
                if nr < tile_grid.len() && nc < tile_grid[nr].len() && !visited.contains(&(nc, nr))
                {
                    if tile_grid[nr][nc].walkable(solved_puzzles, collected_keys) {
                        visited.insert((nc, nr));
                        queue.push((nc, nr));
                    }
                }
            }
        }
        visited
    }

    fn find_exit(level: &LevelData) -> Position {
        for (row, line) in level.grid.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if ch == '>' {
                    return (col, row);
                }
            }
        }
        panic!("No exit in level {}", level.id);
    }

    /// BFS reachability that progressively solves gates to reach deeper areas.
    /// Some levels have gates that divide the map — keys/NPCs behind gates
    /// are only reachable after solving the corresponding puzzle.
    fn progressive_reachability(level: &LevelData) -> HashSet<Position> {
        let state = engine::init_game_state(level);
        let mut solved = HashSet::new();
        let keys = state.collected_keys.clone();

        // Iteratively solve gates until no new area is discovered
        let mut reachable = flood_fill(state.player_pos, &state.tile_grid, &solved, &keys);
        loop {
            let mut new_solved = false;
            for gate in &level.gates {
                if solved.contains(&gate.puzzle_id) {
                    continue;
                }
                let (gc, gr) = gate.pos;
                let adj = [
                    gc.checked_sub(1).map(|c| (c, gr)),
                    Some((gc + 1, gr)),
                    gr.checked_sub(1).map(|r| (gc, r)),
                    Some((gc, gr + 1)),
                ];
                let bumpable = adj
                    .iter()
                    .any(|a| a.map(|p| reachable.contains(&p)).unwrap_or(false));
                if bumpable {
                    solved.insert(gate.puzzle_id.clone());
                    new_solved = true;
                }
            }
            if !new_solved {
                break;
            }
            reachable = flood_fill(state.player_pos, &state.tile_grid, &solved, &keys);
        }
        reachable
    }

    #[test]
    fn all_keys_reachable_progressively() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let reachable = progressive_reachability(level);
            for key in &level.keys {
                assert!(
                    reachable.contains(&key.pos),
                    "Level {} ({}): key '{}' at {:?} unreachable even after solving reachable gates",
                    i + 1, level.id, key.name, key.pos
                );
            }
        }
    }

    #[test]
    fn all_npcs_reachable_progressively() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let reachable = progressive_reachability(level);
            for npc in &level.npcs {
                assert!(
                    reachable.contains(&npc.pos),
                    "Level {} ({}): npc '{}' at {:?} unreachable even after solving reachable gates",
                    i + 1, level.id, npc.name, npc.pos
                );
            }
        }
    }

    #[test]
    fn at_least_one_gate_bumpable_from_start() {
        // Each level with gates should have at least one gate reachable from start
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            if level.gates.is_empty() {
                continue;
            }
            let state = engine::init_game_state(level);
            let reachable = flood_fill(
                state.player_pos,
                &state.tile_grid,
                &state.solved_puzzles,
                &state.collected_keys,
            );
            let has_bumpable = level.gates.iter().any(|gate| {
                let (gc, gr) = gate.pos;
                let adj = [
                    gc.checked_sub(1).map(|c| (c, gr)),
                    Some((gc + 1, gr)),
                    gr.checked_sub(1).map(|r| (gc, r)),
                    Some((gc, gr + 1)),
                ];
                adj.iter()
                    .any(|a| a.map(|p| reachable.contains(&p)).unwrap_or(false))
            });
            assert!(
                has_bumpable,
                "Level {} ({}): no gates bumpable from start (player stuck)",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn exit_reachable_with_all_solved() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let mut state = engine::init_game_state(level);
            for k in &level.required_keys {
                state.collected_keys.insert(k.clone());
            }
            for p in &level.puzzles {
                state.solved_puzzles.insert(p.id().to_string());
            }
            let reachable = flood_fill(
                state.player_pos,
                &state.tile_grid,
                &state.solved_puzzles,
                &state.collected_keys,
            );
            let exit = find_exit(level);
            assert!(
                reachable.contains(&exit),
                "Level {} ({}): exit unreachable even with all keys+puzzles",
                i + 1,
                level.id
            );
        }
    }

    // === Puzzle Submission ===

    #[test]
    fn all_puzzles_solvable_with_correct_answers() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let mut state = engine::init_game_state(level);
            state.current_level = i; // fix: set correct level index
            for puzzle in &level.puzzles {
                let pid = puzzle.id().to_string();
                state = engine::open_puzzle_by_id(state.clone(), &pid, &levels);
                assert!(
                    state.active_puzzle.is_some(),
                    "Level {} ({}): failed to open '{}'",
                    i + 1,
                    level.id,
                    pid
                );

                match puzzle {
                    PuzzleDef::Arrange {
                        pieces, solution, ..
                    } => {
                        let sol_lines: Vec<&str> = solution.lines().collect();
                        let order: Vec<usize> = sol_lines
                            .iter()
                            .map(|l| {
                                let trimmed = l.trim();
                                pieces.iter().position(|p| p.trim() == trimmed).unwrap()
                            })
                            .collect();
                        if let Some(ref mut ps) = state.active_puzzle {
                            ps.arrange_order = order;
                        }
                    }
                    PuzzleDef::FillBlank { answer, .. }
                    | PuzzleDef::FixError { answer, .. }
                    | PuzzleDef::ShortAnswer { answer, .. } => {
                        state = engine::update_puzzle_input(state.clone(), answer.clone());
                    }
                    PuzzleDef::MatchPairs { pairs, .. } => {
                        if let Some(ref mut ps) = state.active_puzzle {
                            for idx in 0..pairs.len() {
                                ps.matched_pairs.push((idx, idx));
                            }
                        }
                    }
                }

                let (new_state, correct) = engine::submit_puzzle(state.clone());
                assert!(
                    correct,
                    "Level {} ({}): correct answer rejected for '{}'",
                    i + 1,
                    level.id,
                    pid
                );
                assert!(
                    new_state.solved_puzzles.contains(&pid),
                    "Level {} ({}): '{}' not marked solved",
                    i + 1,
                    level.id,
                    pid
                );
                state = new_state;
            }
        }
    }

    // === Level Completion ===

    #[test]
    fn levels_complete_with_all_conditions_met() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let mut state = engine::init_game_state(level);
            for k in &level.required_keys {
                state.collected_keys.insert(k.clone());
            }
            for p in &level.puzzles {
                state.solved_puzzles.insert(p.id().to_string());
            }
            state.player_pos = find_exit(level);
            assert!(
                engine::check_level_complete(&state, level),
                "Level {} ({}): not complete with all conditions",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn levels_not_complete_without_keys() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            if level.required_keys.is_empty() {
                continue;
            }
            let mut state = engine::init_game_state(level);
            for p in &level.puzzles {
                state.solved_puzzles.insert(p.id().to_string());
            }
            state.player_pos = find_exit(level);
            assert!(
                !engine::check_level_complete(&state, level),
                "Level {} ({}): complete without keys",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn levels_not_complete_without_puzzles() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            if level.gates.is_empty() {
                continue;
            }
            let mut state = engine::init_game_state(level);
            for k in &level.required_keys {
                state.collected_keys.insert(k.clone());
            }
            state.player_pos = find_exit(level);
            assert!(
                !engine::check_level_complete(&state, level),
                "Level {} ({}): complete without puzzles",
                i + 1,
                level.id
            );
        }
    }

    #[test]
    fn levels_not_complete_off_exit() {
        let levels = default_levels();
        for (i, level) in levels.iter().enumerate() {
            let mut state = engine::init_game_state(level);
            for k in &level.required_keys {
                state.collected_keys.insert(k.clone());
            }
            for p in &level.puzzles {
                state.solved_puzzles.insert(p.id().to_string());
            }
            // Player at start, not on exit
            assert!(
                !engine::check_level_complete(&state, level),
                "Level {} ({}): complete off exit",
                i + 1,
                level.id
            );
        }
    }

    // === Engine Mechanics ===

    #[test]
    fn star_rating_boundaries() {
        assert_eq!(engine::calculate_stars(10, 1, 60), 3);
        assert_eq!(engine::calculate_stars(15, 1, 120), 3);
        assert_eq!(engine::calculate_stars(20, 1, 180), 2);
        assert_eq!(engine::calculate_stars(50, 1, 300), 1);
    }

    #[test]
    fn arrange_init_not_sorted() {
        for n in 2..=8 {
            let order = engine::init_arrange_puzzle(n);
            let sorted: Vec<usize> = (0..n).collect();
            assert_ne!(order, sorted, "init_arrange_puzzle({}) returned sorted", n);
        }
    }

    #[test]
    fn match_pairs_shuffle_not_identity() {
        for n in 2..=8 {
            let shuffle = engine::init_right_shuffle(n);
            let identity: Vec<usize> = (0..n).collect();
            assert_ne!(
                shuffle, identity,
                "init_right_shuffle({}) returned identity",
                n
            );
        }
    }

    #[test]
    fn dialog_dismiss_works() {
        let levels = default_levels();
        let mut state = engine::init_game_state(&levels[0]);
        state.active_dialog = Some(DialogState {
            npc_name: "Test".into(),
            text: "Hi".into(),
        });
        let state = engine::dismiss_dialog(state);
        assert!(state.active_dialog.is_none());
    }

    #[test]
    fn puzzle_dismiss_works() {
        let levels = default_levels();
        let level = &levels[0];
        let mut state = engine::init_game_state(level);
        if let Some(puzzle) = level.puzzles.first() {
            state = engine::open_puzzle_by_id(state, puzzle.id(), &levels);
            assert!(state.active_puzzle.is_some());
            let state = engine::dismiss_puzzle(state);
            assert!(state.active_puzzle.is_none());
        }
    }

    // === Full Level 1 Walkthrough ===

    #[test]
    fn level_01_full_walkthrough() {
        let levels = default_levels();
        let level = &levels[0];
        assert_eq!(level.id, "01_hello_world");

        let mut state = engine::init_game_state(level);
        state.showing_intro = false;

        // Collect fn at (3,4)
        for dir in [
            engine::Direction::Down,
            engine::Direction::Down,
            engine::Direction::Down,
            engine::Direction::Right,
            engine::Direction::Right,
        ] {
            let (s, _) = engine::apply_move(state, dir);
            state = s;
        }
        assert!(state.collected_keys.contains("fn"));
        assert_eq!(state.player_pos, (3, 4));

        // Collect let at (8,2)
        for dir in [
            engine::Direction::Right,
            engine::Direction::Right,
            engine::Direction::Right,
            engine::Direction::Up,
            engine::Direction::Up,
            engine::Direction::Up,
            engine::Direction::Right,
            engine::Direction::Right,
            engine::Direction::Down,
        ] {
            let (s, _) = engine::apply_move(state, dir);
            state = s;
        }
        assert!(state.collected_keys.contains("let"));

        // Collect println! at (11,6)
        for dir in [
            engine::Direction::Right,
            engine::Direction::Right,
            engine::Direction::Right,
            engine::Direction::Down,
            engine::Direction::Down,
            engine::Direction::Down,
            engine::Direction::Down,
        ] {
            let (s, _) = engine::apply_move(state, dir);
            state = s;
        }
        assert!(state.collected_keys.contains("println!"));

        // Solve puzzle
        state = engine::open_puzzle_by_id(state, "l01_arrange_hello", &levels);
        assert!(state.active_puzzle.is_some());
        if let Some(ref mut ps) = state.active_puzzle {
            // Pieces: [0]"}", [1]"println!", [2]"fn main() {"
            // Solution: [2, 1, 0]
            ps.arrange_order = vec![2, 1, 0];
        }
        let (mut state, correct) = engine::submit_puzzle(state);
        assert!(correct);
        assert!(state.solved_puzzles.contains("l01_arrange_hello"));

        // Walk to exit (12,8)
        for dir in [
            engine::Direction::Down,
            engine::Direction::Right,
            engine::Direction::Down,
        ] {
            let (s, _) = engine::apply_move(state, dir);
            state = s;
        }
        assert!(
            engine::check_level_complete(&state, level),
            "Level 1 should be complete"
        );
    }
}
