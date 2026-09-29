//! Plan 028 F7: the adventure page borrows its state instead of cloning it.
//!
//! `game.get()` clones the whole `GameState` (tile grid and key sets) and
//! `levels_signal.get()` clones every level. The view closures did that on
//! every move, about 20 clones per step. They now borrow with `read()` or
//! `with()`. `get()` stays only where the engine takes the state by value.

// The page plus the view modules split out of it (.issues/052).
const PAGE: &str = concat!(
    include_str!("../src/pages/adventure/page.rs"),
    include_str!("../src/pages/adventure/puzzle_view.rs"),
    include_str!("../src/pages/adventure/grid_view.rs")
);

#[test]
fn levels_are_never_cloned() {
    assert!(!PAGE.contains("levels_signal.get()"));
}

#[test]
fn game_is_cloned_only_to_hand_the_engine_an_owned_state() {
    let lines: Vec<&str> = PAGE.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains("game.get()") {
            continue;
        }
        let window = lines[i..(i + 10).min(lines.len())].join("\n");
        assert!(
            [
                "engine::apply_move(",
                "engine::submit_puzzle(",
                "engine::try_match_pair("
            ]
            .iter()
            .any(|f| window.contains(f)),
            "line {}: borrow with read()/with() instead of cloning",
            i + 1
        );
    }
}
