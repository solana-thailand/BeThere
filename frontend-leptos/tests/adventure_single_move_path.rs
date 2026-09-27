//! Keyboard, d-pad and swipe share one move closure. The keyboard copy had
//! lost the `level_completed` guard, so arrow keys kept moving the player on
//! a finished level and re-ran the auto-save Effect (another save POST).

const PAGE: &str = include_str!("../src/pages/adventure/page.rs");

#[test]
fn player_moves_are_applied_in_one_place() {
    assert_eq!(PAGE.matches("engine::apply_move(").count(), 1);
}

#[test]
fn the_shared_move_stops_at_a_finished_level() {
    let start = PAGE
        .find("let step = move |dir: engine::Direction|")
        .unwrap();
    let head = &PAGE[start..PAGE[start..].find("engine::apply_move(").unwrap() + start];
    assert!(
        head.contains("g.level_completed"),
        "guard missing before apply_move"
    );
}

#[test]
fn every_input_path_uses_the_shared_move() {
    for handler in ["let handle_keydown", "let dpad_move"] {
        let start = PAGE.find(handler).unwrap();
        let body = &PAGE[start..start + PAGE[start..].find("\n    };").unwrap()];
        assert!(body.contains("step(dir)"), "{handler} bypasses step()");
    }
}
