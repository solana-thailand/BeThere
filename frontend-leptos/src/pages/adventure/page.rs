//! Adventure game page — Rust Adventures.
//!
//! Route: `/adventure`
//! A tile-based puzzle game that teaches Rust programming.
//! Players navigate a grid, collect keyword keys, solve code puzzles.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use std::collections::HashSet;

use serde_json;

use super::{default_levels, engine, types::*};
use crate::api::{self, AdventureLevelScore};

/// localStorage key for casual (no-token) progress.
const LS_COMPLETED_KEY: &str = "adventure_completed_levels";

// ============================================================
// Main Adventure Component
// ============================================================

#[component]
pub fn Adventure() -> impl IntoView {
    let levels = default_levels();
    let first_level = levels.first().cloned().unwrap_or_else(test_level_fallback);

    // Game state signal
    let initial_state = engine::init_game_state(&first_level);
    let (game, set_game) = signal(initial_state);

    // Levels data
    let (levels_signal, set_levels) = signal(levels);
    let _ = set_levels;

    // UI state
    let (show_level_select, set_show_level_select) = signal(false);
    let (notification, set_notification) = signal::<Option<String>>(None);
    let (puzzle_feedback, set_puzzle_feedback) = signal::<Option<bool>>(None);
    let (gate_animating, set_gate_animating) = signal::<Option<String>>(None); // puzzle_id when animating
    let (elapsed_seconds, set_elapsed_seconds) = signal(0u32);
    let (completed_levels, set_completed_levels) = signal::<HashSet<usize>>(HashSet::new());
    let grid_container_ref = NodeRef::<leptos::html::Div>::new();

    // Query params for claim flow integration
    let query = use_query_map();
    let _claim_token = move || query.get().get("token").map(|s| s.to_string());
    let has_token = _claim_token().is_some();
    let _required_level = move || {
        query
            .get()
            .get("level")
            .and_then(|l| l.parse::<usize>().ok())
    };

    // Restore progress: API if token present, localStorage otherwise
    let restore_token = _claim_token();
    let restore_levels = levels_signal.clone();
    let restore_set_game = set_game.clone();
    let restore_set_completed = set_completed_levels.clone();
    Effect::new(move |_| {
        if let Some(ref token) = restore_token {
            // Restore from API (claim flow)
            let token = token.clone();
        let levels = restore_levels.get();
        let set_g = restore_set_game.clone();
        let set_completed = restore_set_completed.clone();
        leptos::task::spawn_local(async move {
            match api::get_adventure_status(&token).await {
                Ok(status_data) => {
                    if let Some(progress) = status_data.progress {
                        if !progress.levels_completed.is_empty() {
                            // Find which level indices are completed
                            let completed_indices: HashSet<usize> = levels.iter()
                                .enumerate()
                                .filter(|(_, l)| progress.levels_completed.contains(&l.id))
                                .map(|(i, _)| i)
                                .collect();
                            set_completed.set(completed_indices.clone());

                            // Find the first uncompleted level
                            let next_level_idx = levels.iter().position(|l| {
                                !progress.levels_completed.contains(&l.id)
                            }).unwrap_or(0);
                            log::info!(
                                "[adventure] restored progress: {}/{} levels done, loading level {}",
                                progress.levels_completed.len(),
                                levels.len(),
                                next_level_idx + 1
                            );
                            if let Some(level) = levels.get(next_level_idx) {
                                let mut state = engine::init_game_state(level);
                                state.current_level = next_level_idx;
                                state.showing_intro = false; // skip intro for returning players
                                set_g.set(state);
                            }
                        }
                    }
                }
                Err(e) => {
                        log::warn!("[adventure] failed to restore progress: {e}");
                        // Continue with default — first level with intro
                    }
                }
            });
            } else {
                // Restore from localStorage (casual play)
                let stored = gloo::utils::window()
                    .local_storage()
                    .ok()
                    .flatten()
                    .and_then(|ls| ls.get(LS_COMPLETED_KEY).ok().flatten());
                if let Some(json) = stored {
                    if let Ok(indices) = serde_json::from_str::<HashSet<usize>>(&json) {
                        log::info!("[adventure] restored {} completed levels from localStorage", indices.len());
                        let levels = restore_levels.get();
                        let next_idx = (0..levels.len()).find(|i| !indices.contains(i)).unwrap_or(0);
                        if let Some(level) = levels.get(next_idx) {
                            let mut state = engine::init_game_state(level);
                            state.current_level = next_idx;
                            state.showing_intro = false;
                            restore_set_game.set(state);
                        }
                        restore_set_completed.set(indices);
                    }
                }
            }
        });

    // Timer — increments every second while level is active
    let game_for_timer = game.clone();
    let _timer = set_interval(
        move || {
            let g = game_for_timer.get();
            if !g.showing_intro && !g.level_completed {
                set_elapsed_seconds.update(|t| *t += 1);
            }
        },
        std::time::Duration::from_secs(1),
    );

    // Reactive scroll — fires whenever player position changes
    let scroll_game = game.clone();
    let scroll_grid_ref = grid_container_ref.clone();
    Effect::new(move |_| {
        let g = scroll_game.get();
        // Trigger on player position change
        let _pos = g.player_pos;
        // Defer scroll to next frame so DOM has updated
        let grid_ref = scroll_grid_ref.clone();
        request_animation_frame(move || {
            let Some(el) = grid_ref.get() else { return };
            let (col, row) = _pos;
            let tile_size = 48.0_f64;
            let container_width = el.client_width() as f64;
            let container_height = el.client_height() as f64;
            let levels = levels_signal.get();
            let g = scroll_game.get();
            let grid_width = levels.get(g.current_level).map(|l| l.width).unwrap_or(12) as f64;
            let grid_height = levels.get(g.current_level).map(|l| l.height).unwrap_or(8) as f64;
            let total_w = grid_width * tile_size;
            let total_h = grid_height * tile_size;
            if total_w <= container_width && total_h <= container_height {
                return; // grid fits in container, no scroll needed
            }
            let player_x = col as f64 * tile_size + tile_size / 2.0;
            let player_y = row as f64 * tile_size + tile_size / 2.0;
            let scroll_x = (player_x - container_width / 2.0).max(0.0);
            let scroll_y = (player_y - container_height / 2.0).max(0.0);
            el.set_scroll_left(scroll_x as i32);
            el.set_scroll_top(scroll_y as i32);
        });
    });

    // Auto-save on level completion
    let (save_status, set_save_status) = signal::<Option<String>>(None); // None=in progress, Some(msg)=done
    let claim_token_for_save = _claim_token();
    let auto_save_game = game.clone();
    let auto_save_levels = levels_signal.clone();
    let auto_save_elapsed = elapsed_seconds.clone();
    let auto_save_set_status = set_save_status.clone();
    let auto_save_set_completed = set_completed_levels.clone();
    let auto_save_completed_read = completed_levels.clone();
    Effect::new(move |_| {
        let g = auto_save_game.get();
        if !g.level_completed {
            return;
        }
        let levels = auto_save_levels.get();
        let Some(level) = levels.get(g.current_level) else {
            return;
        };
        let level_idx = g.current_level;

        // Mark level as completed in local state
        auto_save_set_completed.update(|set| {
            set.insert(level_idx);
        });

        if let Some(ref token) = claim_token_for_save {
            // Save to API (claim flow)
            let level_id = level.id.clone();
            let elapsed = auto_save_elapsed.get();
            let stars = engine::calculate_stars(g.moves_count, g.solved_puzzles.len() as u32, elapsed);
            let score = AdventureLevelScore {
                moves: g.moves_count,
                puzzles_solved: g.solved_puzzles.len() as u32,
                time_seconds: elapsed,
                stars,
            };
            let token_clone = token.clone();
            let level_id_clone = level_id.clone();
            auto_save_set_status.set(Some("saving".to_string()));
            leptos::task::spawn_local(async move {
                match api::save_adventure_progress(&token_clone, &level_id_clone, &score).await {
                    Ok(_progress) => {
                        log::info!("[adventure] saved progress for level {level_id_clone}");
                        auto_save_set_status.set(Some("saved".to_string()));
                    }
                    Err(e) => {
                        log::warn!("[adventure] failed to save progress: {e}");
                        auto_save_set_status.set(Some(format!("error: {e}")));
                    }
                }
            });
        } else {
            // Save to localStorage (casual play)
            let completed = auto_save_completed_read.get();
            if let Ok(json) = serde_json::to_string(&completed) {
                if let Some(ls) = gloo::utils::window().local_storage().ok().flatten() {
                    if let Err(_) = ls.set(LS_COMPLETED_KEY, &json) {
                        log::warn!("[adventure] failed to save to localStorage");
                    } else {
                        log::info!("[adventure] saved {} completed levels to localStorage", completed.len());
                    }
                }
            }
        }
    });

    // Dismiss intro on first interaction
    let dismiss_intro = move || {
        let g = game.get();
        if g.showing_intro {
            set_game.update(|g| g.showing_intro = false);
            set_elapsed_seconds.set(0);
        }
    };

    // Load a level by index
    let load_level = move |level_idx: usize| {
        let levels = levels_signal.get();
        if let Some(level) = levels.get(level_idx) {
            let mut state = engine::init_game_state(level);
            state.current_level = level_idx;
            set_game.set(state);
            set_elapsed_seconds.set(0);
            set_show_level_select.set(false);
            set_puzzle_feedback.set(None);
            set_gate_animating.set(None);
            auto_save_set_status.set(None);
        }
    };

    // Auto-dismiss notification after 3s
    let auto_dismiss_notification = {
        let set_notif = set_notification.clone();
        move || {
            let set_notif = set_notif.clone();
            set_timeout(
                move || set_notif.set(None),
                std::time::Duration::from_secs(3),
            );
        }
    };

    // Handle keyboard input — global listener
    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        let g = game.get();

        // If level select is showing, Escape closes it
        if show_level_select.get() {
            match ev.key().as_str() {
                "Escape" => {
                    set_show_level_select.set(false);
                    return;
                }
                _ => return,
            }
        }

        // If dialog is active, any key dismisses it
        if g.active_dialog.is_some() {
            set_game.update(|g| *g = engine::dismiss_dialog(g.clone()));
            return;
        }

        // If intro is showing, any key dismisses
        if g.showing_intro {
            dismiss_intro();
            return;
        }

        // If puzzle is active, don't process movement
        if g.active_puzzle.is_some() {
            return;
        }

        let direction = match ev.key().as_str() {
            "ArrowUp" | "w" | "k" => Some(engine::Direction::Up),
            "ArrowDown" | "s" | "j" => Some(engine::Direction::Down),
            "ArrowLeft" | "a" | "h" => Some(engine::Direction::Left),
            "ArrowRight" | "d" | "l" => Some(engine::Direction::Right),
            _ => None,
        };

        if let Some(dir) = direction {
            ev.prevent_default();
            let current = game.get();
            let (new_state, result) = engine::apply_move(current, dir);

            match &result {
                MoveResult::CollectedKey { name, description } => {
                    set_notification.set(Some(format!("🔑 Collected: {name} — {description}")));
                    auto_dismiss_notification();
                }
                MoveResult::ExitReached => {
                    let levels = levels_signal.get();
                    if let Some(level) = levels.get(new_state.current_level)
                        && engine::check_level_complete(&new_state, level)
                    {
                        let completed_idx = new_state.current_level;
                        set_game.update(|g| {
                            g.player_pos = new_state.player_pos;
                            g.moves_count = new_state.moves_count;
                            g.level_completed = true;
                        });
                        set_completed_levels.update(|c| {
                            c.insert(completed_idx);
                        });
                        set_notification.set(Some("🎉 Level Complete!".to_string()));
                        return;
                    }
                }
                MoveResult::HitCodeBlock { puzzle_id } | MoveResult::HitGate { puzzle_id } => {
                    let levels = levels_signal.get();
                    set_game.update(|g| {
                        *g = engine::open_puzzle_by_id(g.clone(), puzzle_id, &levels);
                    });
                    set_notification.set(Some("🔒 Gate locked! Solve the puzzle to open it.".to_string()));
                    auto_dismiss_notification();
                    return;
                }
                MoveResult::Blocked => {}
                _ => {}
            }

            set_game.set(new_state);
        }
    };

    // Global keyboard listener
    let _ = window_event_listener(leptos::ev::keydown, handle_keydown);

    // D-pad handler for mobile
    let dpad_move = move |dir: engine::Direction| {
        dismiss_intro();
        let g = game.get();
        if g.active_dialog.is_some() {
            set_game.update(|g| *g = engine::dismiss_dialog(g.clone()));
            return;
        }
        if g.active_puzzle.is_some() || g.showing_intro || g.level_completed {
            return;
        }
        let (new_state, result) = engine::apply_move(g, dir);
        match &result {
            MoveResult::CollectedKey { name, description } => {
                set_notification.set(Some(format!("🔑 Collected: {name} — {description}")));
                auto_dismiss_notification();
            }
            MoveResult::ExitReached => {
                let levels = levels_signal.get();
                if let Some(level) = levels.get(new_state.current_level)
                    && engine::check_level_complete(&new_state, level)
                {
                    let completed_idx = new_state.current_level;
                    set_game.update(|g| {
                        g.player_pos = new_state.player_pos;
                        g.moves_count = new_state.moves_count;
                        g.level_completed = true;
                    });
                    set_completed_levels.update(|c| {
                        c.insert(completed_idx);
                    });
                    set_notification.set(Some("🎉 Level Complete!".to_string()));
                    return;
                }
            }
            MoveResult::HitCodeBlock { puzzle_id } | MoveResult::HitGate { puzzle_id } => {
                let levels = levels_signal.get();
                set_game.update(|g| {
                    *g = engine::open_puzzle_by_id(g.clone(), puzzle_id, &levels);
                });
                set_notification.set(Some("🔒 Gate locked! Solve the puzzle to open it.".to_string()));
                auto_dismiss_notification();
                return;
            }
            _ => {}
        }
        set_game.set(new_state);
    };

    // Compute tile grid for rendering
    // Check if all conditions are met for exit to be "unlocked"
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

    let first_level_ref = levels_signal.get();
    let first_level_width = first_level_ref
        .first()
        .map(|l| l.width)
        .unwrap_or(12);

    // Format elapsed time as MM:SS
    let format_time = move || {
        let s = elapsed_seconds.get();
        format!("{:02}:{:02}", s / 60, s % 60)
    };

    view! {
        <Title text="Rust Adventures — Learn Rust by Playing" />
        <div class="adventure-page">
            // No-token warning banner
            <Show when=move || !has_token fallback=|| view! { <div></div> }>
                <div class="adventure-banner adventure-banner-info">
                    <span class="adventure-banner-icon">"💡"</span>
                    <span>"Playing in demo mode — progress saves to your browser. "
                        <b>"Claim an NFT badge?"</b>
                        " Ask staff to scan your ticket first!"
                    </span>
                </div>
            </Show>

            // Header
            <header class="adventure-header">
                <div class="adventure-brand">
                    <span class="adventure-logo">"🦀"</span>
                    <h1 class="adventure-title">"Rust Adventures"</h1>
                </div>
                <div class="adventure-header-right">
                    <span class="adventure-timer">
                        {format_time}
                    </span>
                    <span class="adventure-moves">
                        "Moves: " {move || game.get().moves_count}
                    </span>
                    <button
                        class="btn btn-outline btn-sm"
                        on:click=move |_| set_show_level_select.set(!show_level_select.get())
                    >
                        "Levels"
                    </button>
                </div>
            </header>

            // Current level name
            {move || {
                let g = game.get();
                let levels = levels_signal.get();
                let level_name = levels.get(g.current_level)
                    .map(|l| format!("Level {} — {}", g.current_level + 1, l.name))
                    .unwrap_or_default();
                view! {
                    <div class="adventure-level-name">{level_name}</div>
                }.into_any()
            }}

            // Keys collected bar
            <div class="adventure-keys-bar">
                <span class="keys-label">"Keys: "</span>
                {move || {
                    let keys: Vec<String> = game.get().collected_keys.iter().cloned().collect();
                    if keys.is_empty() {
                        vec![view! { <span class="key-empty">"none yet"</span> }.into_any()]
                    } else {
                        keys.into_iter().map(|k| view! {
                            <span class="key-badge">{k}</span>
                        }.into_any()).collect()
                    }
                }}
            </div>

            // Required keys hint
            {move || {
                let g = game.get();
                let levels = levels_signal.get();
                if let Some(level) = levels.get(g.current_level) {
                    let missing: Vec<String> = level.required_keys.iter()
                        .filter(|k| !g.collected_keys.contains(*k))
                        .cloned()
                        .collect();
                    if !missing.is_empty() {
                        view! {
                            <div class="adventure-hint-bar">
                                "Need: " {missing.join(", ")}
                            </div>
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            // Notification toast
            {move || {
                notification.get().map(|msg| view! {
                    <div class="adventure-notification">
                        {msg}
                    </div>
                }.into_any())
            }}

            // === Level Select Overlay ===
            {move || {
                if show_level_select.get() {
                    let levels = levels_signal.get();
                    let current = game.get().current_level;
                    let completed = completed_levels.get();
                    let levels_vec: Vec<(usize, String, String, bool, bool)> = levels.iter()
                        .enumerate()
                        .map(|(i, l)| (i, l.name.clone(), l.concept.clone(), i == current, completed.contains(&i)))
                        .collect();
                    view! {
                        <div class="adventure-overlay" on:click=move |_| set_show_level_select.set(false)>
                            <div class="adventure-overlay-card adventure-level-select-card" on:click=move |ev| ev.stop_propagation()>
                                <h2>"🗺️ Select Level"</h2>
                                <div class="level-select-list">
                                    {levels_vec.into_iter().map(|(idx, name, concept, is_current, is_completed)| {
                                        let load_idx = idx;
                                        let item_class = if is_current {
                                            "level-select-item level-select-active"
                                        } else if is_completed {
                                            "level-select-item level-select-completed"
                                        } else {
                                            "level-select-item"
                                        };
                                        view! {
                                            <button
                                                class={item_class}
                                                on:click=move |_| load_level(load_idx)
                                            >
                                                <span class="level-select-num">{format!("{}", idx + 1)}</span>
                                                <span class="level-select-info">
                                                    <span class="level-select-name">{name}</span>
                                                    <span class="level-select-concept">{concept}</span>
                                                </span>
                                                {if is_completed {
                                                    view! { <span class="level-select-check">"✓"</span> }.into_any()
                                                } else {
                                                    view! { <span></span> }.into_any()
                                                }}
                                            </button>
                                        }
                                    }).collect_view()}
                                </div>
                                <button class="btn btn-outline" on:click=move |_| set_show_level_select.set(false)>
                                    "Close"
                                </button>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            // === Intro Overlay ===
            {move || {
                let g = game.get();
                if g.showing_intro {
                    let levels = levels_signal.get();
                    let level_info = levels.get(g.current_level)
                        .map(|l| (l.intro_text.clone(), l.name.clone()));
                    let (intro, level_name) = level_info.unwrap_or_default();
                    view! {
                        <div class="adventure-overlay" on:click=move |_| dismiss_intro()>
                            <div class="adventure-overlay-card adventure-intro-card">
                                <div class="adventure-intro-level">"Level " {g.current_level + 1}</div>
                                <h2>{level_name}</h2>
                                <p>{intro}</p>
                                <div class="adventure-intro-controls">
                                    <div class="control-key-pair">
                                        <span class="control-keys">"↑ ← ↓ → / WASD / hjkl"</span>
                                        <span class="control-desc">"Move"</span>
                                    </div>
                                    <div class="control-key-pair">
                                        <span class="control-keys">"Walk into tiles"</span>
                                        <span class="control-desc">"Interact"</span>
                                    </div>
                                </div>
                                <p class="adventure-overlay-hint">"Press any key or tap to start"</p>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            // === Level Complete Overlay ===
            {move || {
                let g = game.get();
                if g.level_completed {
                    let levels = levels_signal.get();
                    let level_info = levels.get(g.current_level)
                        .map(|l| (l.completion_text.clone(), l.id.clone()));
                    let (completion, _level_id) = level_info.unwrap_or_default();
                    let has_next = g.current_level + 1 < levels.len();
                    let next_idx = g.current_level + 1;
                    let time_str = format_time();
                    let elapsed = elapsed_seconds.get();
                    let stars = engine::calculate_stars(g.moves_count, g.solved_puzzles.len() as u32, elapsed);
                    let star_display = match stars {
                        3 => "⭐⭐⭐".to_string(),
                        2 => "⭐⭐☆".to_string(),
                        _ => "⭐☆☆".to_string(),
                    };
                    let save_msg = save_status.get();
                    let saving_indicator = match save_msg.as_deref() {
                        Some("saving") => "💾 Saving...".to_string(),
                        Some("saved") => "✅ Progress saved!".to_string(),
                        Some(msg) if msg.starts_with("error") => "⚠️ Save failed (offline mode)".to_string(),
                        _ => String::new(),
                    };
                    view! {
                        <div class="adventure-overlay adventure-overlay-success">
                            <div class="adventure-overlay-card adventure-overlay-card-success">
                                <div class="adventure-success-icon">"🎉"</div>
                                <h2>"Level Complete!"</h2>
                                <div class="adventure-stars">{star_display}</div>
                                <p>{completion}</p>
                                <div class="adventure-stats">
                                    <div class="stat-item">
                                        <span class="stat-label">"Moves"</span>
                                        <span class="stat-value">{g.moves_count}</span>
                                    </div>
                                    <div class="stat-item">
                                        <span class="stat-label">"Time"</span>
                                        <span class="stat-value">{time_str}</span>
                                    </div>
                                    <div class="stat-item">
                                        <span class="stat-label">"Keys"</span>
                                        <span class="stat-value">{g.collected_keys.len()}</span>
                                    </div>
                                </div>
                                {if saving_indicator.is_empty() {
                                    view! { <div></div> }.into_any()
                                } else {
                                    view! {
                                        <div class="adventure-save-status">{saving_indicator}</div>
                                    }.into_any()
                                }}
                                <div class="adventure-success-actions">
                                    {if has_next {
                                        view! {
                                            <button class="btn btn-primary" on:click=move |_| load_level(next_idx)>
                                                "Next Level →"
                                            </button>
                                        }.into_any()
                                    } else {
                                        let claim_link = _claim_token().map(|t| format!("/claim/{t}"));
                                        view! {
                                            <div class="adventure-all-complete">
                                                "🏆 All levels complete! You're a Rust adventurer!"
                                            </div>
                                            {if let Some(link) = claim_link {
                                                view! {
                                                    <a class="btn btn-primary adventure-claim-btn" href={link}>
                                                        "🎁 Claim your NFT Badge"
                                                    </a>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <button class="btn btn-outline" on:click=move |_| {
                                                        // Reset all progress and play from level 1
                                                        set_completed_levels.set(HashSet::new());
                                                        if let Some(ls) = gloo::utils::window().local_storage().ok().flatten() {
                                                            let _ = ls.remove_item(LS_COMPLETED_KEY);
                                                        }
                                                        if let Some(level) = levels_signal.get().first() {
                                                            let mut state = engine::init_game_state(level);
                                                            state.current_level = 0;
                                                            set_game.set(state);
                                                        }
                                                    }>
                                                        "🔄 Play Again"
                                                    </button>
                                                }.into_any()
                                            }}
                                        }.into_any()
                                    }}
                                    <button class="btn btn-outline" on:click=move |_| {
                                        set_game.update(|g| g.level_completed = false);
                                        set_show_level_select.set(true);
                                    }>
                                        "Level Select"
                                    </button>
                                </div>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            // === Dialog Overlay (NPC / Sign) ===
            {move || {
                game.get().active_dialog.as_ref().map(|dialog| view! {
                    <div class="adventure-overlay" on:click=move |_| {
                        set_game.update(|g| *g = engine::dismiss_dialog(g.clone()));
                    }>
                        <div class="adventure-dialog-card">
                            <div class="dialog-speaker">{dialog.npc_name.clone()}</div>
                            <div class="dialog-text">{dialog.text.clone()}</div>
                            <p class="adventure-overlay-hint">"Press any key or tap to dismiss"</p>
                        </div>
                    </div>
                }.into_any())
            }}

            // === Puzzle Overlay ===
            {move || {
                game.get().active_puzzle.as_ref().map(|puzzle_state| {
                    let puzzle = &puzzle_state.puzzle;
                    let pid = puzzle.id().to_string();
                    let hint_text = puzzle.hint().to_string();

                    let instruction = match puzzle {
                        PuzzleDef::Arrange { instruction, pieces, .. } => {
                            let inst = instruction.clone();
                            let pcs: Vec<String> = pieces.clone();
                            let order = puzzle_state.arrange_order.clone();
                            view! {
                                <div>
                                    <p class="puzzle-instruction">{inst}</p>
                                    <p class="puzzle-hint-small">"Use ↑↓ buttons or drag to reorder"</p>
                                    <div class="puzzle-pieces puzzle-pieces-interactive">
                                        {order.iter().enumerate().map(|(display_idx, &piece_idx)| {
                                            let piece_text = pcs.get(piece_idx)
                                                .cloned()
                                                .unwrap_or_default();
                                            let up_idx = display_idx;
                                            let down_idx = display_idx;
                                            view! {
                                                <div class="puzzle-piece-row">
                                                    <button
                                                        class="puzzle-move-btn"
                                                        on:click=move |_| {
                                                            set_game.update(|g| {
                                                                engine::arrange_piece_up(g, up_idx);
                                                            });
                                                        }
                                                        disabled={display_idx == 0}
                                                    >
                                                        "↑"
                                                    </button>
                                                    <button
                                                        class="puzzle-move-btn"
                                                        on:click=move |_| {
                                                            set_game.update(|g| {
                                                                engine::arrange_piece_down(g, down_idx);
                                                            });
                                                        }
                                                        disabled={display_idx >= order.len() - 1}
                                                    >
                                                        "↓"
                                                    </button>
                                                    <div class="puzzle-piece">
                                                        <span class="puzzle-piece-num">{format!("{}.", display_idx + 1)}</span>
                                                        {piece_text}
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            }.into_any()
                        }
                        PuzzleDef::FillBlank { instruction, code_template, options, answer: _, .. } => {
                            let inst = instruction.clone();
                            let tmpl = code_template.clone();
                            let opts: Vec<String> = options.clone();
                            let current_input = puzzle_state.input.clone();
                            view! {
                                <div>
                                    <p class="puzzle-instruction">{inst}</p>
                                    <pre class="puzzle-code">{tmpl}</pre>
                                    <div class="puzzle-options">
                                        {opts.into_iter().map(|opt| {
                                            let opt_val = opt.clone();
                                            let is_selected = current_input == opt_val;
                                            let sel_class = if is_selected { "puzzle-opt puzzle-opt-selected" } else { "puzzle-opt" };
                                            view! {
                                                <button
                                                    class={sel_class}
                                                    on:click=move |_| {
                                                        set_game.update(|g| {
                                                            *g = engine::update_puzzle_input(g.clone(), opt_val.clone());
                                                        });
                                                    }
                                                >
                                                    {opt}
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            }.into_any()
                        }
                        PuzzleDef::FixError { instruction, broken_code, options, .. } => {
                            let inst = instruction.clone();
                            let code = broken_code.clone();
                            let opts: Vec<String> = options.clone();
                            let current_input = puzzle_state.input.clone();
                            view! {
                                <div>
                                    <p class="puzzle-instruction">{inst}</p>
                                    <pre class="puzzle-code puzzle-code-broken">{code}</pre>
                                    <div class="puzzle-options">
                                        {opts.into_iter().map(|opt| {
                                            let opt_val = opt.clone();
                                            let is_selected = current_input == opt_val;
                                            let sel_class = if is_selected { "puzzle-opt puzzle-opt-selected" } else { "puzzle-opt" };
                                            view! {
                                                <button
                                                    class={sel_class}
                                                    on:click=move |_| {
                                                        set_game.update(|g| {
                                                            *g = engine::update_puzzle_input(g.clone(), opt_val.clone());
                                                        });
                                                    }
                                                >
                                                    {opt}
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            }.into_any()
                        }
                        PuzzleDef::ShortAnswer { instruction, code_template, .. } => {
                            let inst = instruction.clone();
                            let tmpl = code_template.clone();
                            view! {
                                <div>
                                    <p class="puzzle-instruction">{inst}</p>
                                    <pre class="puzzle-code">{tmpl}</pre>
                                    <input
                                        class="puzzle-input"
                                        type="text"
                                        placeholder="Type your answer..."
                                        on:input=move |ev| {
                                            let val = event_target_value(&ev);
                                            set_game.update(|g| {
                                                *g = engine::update_puzzle_input(g.clone(), val);
                                            });
                                        }
                                    />
                                </div>
                            }.into_any()
                        }
                        PuzzleDef::MatchPairs { instruction, pairs, .. } => {
                            let inst = instruction.clone();
                            let pairs_data: Vec<(String, String)> = pairs.clone();
                            let matched = puzzle_state.matched_pairs.clone();
                            let selected = puzzle_state.selected_left;
                            let right_shuffle = puzzle_state.right_shuffle.clone();
                            // Left items in canonical order, right items shuffled
                            let left_items: Vec<String> = pairs_data.iter().map(|(l, _)| l.clone()).collect();
                            let right_items_shuffled: Vec<(usize, String)> = right_shuffle.iter()
                                .map(|&canonical_idx| {
                                    let text = pairs_data.get(canonical_idx)
                                        .map(|(_, r)| r.clone())
                                        .unwrap_or_default();
                                    (canonical_idx, text)
                                })
                                .collect();
                            view! {
                                <div>
                                    <p class="puzzle-instruction">{inst}</p>
                                    <div class="puzzle-match-area">
                                        <div class="match-row match-row-left">
                                            <span class="match-label">"Code"</span>
                                            {left_items.iter().enumerate().map(|(idx, item)| {
                                                let is_matched = matched.iter().any(|(l, _)| *l == idx);
                                                let is_selected = selected == Some(idx);
                                                let cls = if is_matched {
                                                    "match-item match-item-matched"
                                                } else if is_selected {
                                                    "match-item match-item-selected"
                                                } else {
                                                    "match-item"
                                                };
                                                let left_idx = idx;
                                                view! {
                                                    <button
                                                        class={cls}
                                                        on:click=move |_| {
                                                            set_game.update(|g| {
                                                                engine::select_match_left(g, left_idx);
                                                            });
                                                        }
                                                        disabled={is_matched}
                                                    >
                                                        {item.clone()}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                        <div class="match-arrows">
                                            {(0..pairs_data.len()).map(|_| {
                                                view! { <span class="match-arrow">"↕"</span> }
                                            }).collect_view()}
                                        </div>
                                        <div class="match-row match-row-right">
                                            <span class="match-label">"Type"</span>
                                            {right_items_shuffled.iter().enumerate().map(|(display_idx, (canonical_idx, item))| {
                                                let is_matched = matched.iter().any(|(_, r)| *r == *canonical_idx);
                                                let cls = if is_matched {
                                                    "match-item match-item-matched"
                                                } else {
                                                    "match-item"
                                                };
                                                let right_display_idx = display_idx;
                                                let pairs_count = pairs_data.len();
                                                view! {
                                                    <button
                                                        class={cls}
                                                        on:click=move |_| {
                                                            let result = {
                                                                let g = game.get();
                                                                let mut g_clone = g;
                                                                engine::try_match_pair(&mut g_clone, right_display_idx)
                                                            };
                                                            if let Some(correct) = result {
                                                                set_game.update(|g| {
                                                                    engine::try_match_pair(g, right_display_idx);
                                                                });
                                                                if correct {
                                                                    let g = game.get();
                                                                    if let Some(ps) = &g.active_puzzle {
                                                                        if ps.matched_pairs.len() == pairs_count {
                                                                            set_notification.set(Some("🧩 All pairs matched!".to_string()));
                                                                        }
                                                                    }
                                                                } else {
                                                                    set_puzzle_feedback.set(Some(false));
                                                                }
                                                            }
                                                        }
                                                        disabled={is_matched}
                                                    >
                                                        {item.clone()}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        }
                    };

                    view! {
                        <div class="adventure-overlay adventure-overlay-puzzle">
                            <div class="adventure-puzzle-card">
                                <h3>"🧩 Code Puzzle"</h3>
                                {instruction}

                                // Feedback
                                {move || {
                                    puzzle_feedback.get().map(|correct| {
                                        if correct {
                                            view! {
                                                <div class="puzzle-feedback puzzle-correct">
                                                    "✅ Correct!"
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div class="puzzle-feedback puzzle-wrong">
                                                    "❌ Not quite. Try again!"
                                                </div>
                                            }.into_any()
                                        }
                                    })
                                }}

                                <div class="puzzle-actions">
                                    <button class="btn btn-primary" on:click=move |_| {
                                        let (new_state, correct) = engine::submit_puzzle(game.get());
                                        if correct {
                                            // Gate animation
                                            if new_state.active_puzzle.is_some() {
                                                // shouldn't have active puzzle after correct
                                            } else {
                                                // The puzzle was just solved — find the gate
                                                let levels = levels_signal.get();
                                                if let Some(level) = levels.get(new_state.current_level) {
                                                    for gate in &level.gates {
                                                        if gate.puzzle_id == pid {
                                                            set_gate_animating.set(Some(pid.clone()));
                                                            // Clear animation after delay
                                                            let _anim_id = pid.clone();
                                                            set_timeout(move || {
                                                                set_gate_animating.set(None);
                                                            }, std::time::Duration::from_millis(600));
                                                        }
                                                    }
                                                }
                                            }
                                            set_game.set(new_state);
                                            set_puzzle_feedback.set(None);
                                            set_notification.set(Some("🧩 Puzzle solved! Gate opened.".to_string()));
                                            auto_dismiss_notification();
                                        } else {
                                            set_game.set(new_state);
                                            set_puzzle_feedback.set(Some(false));
                                        }
                                    }>"Submit"</button>
                                    <button class="btn btn-outline" on:click=move |_| {
                                        set_game.update(|g| *g = engine::dismiss_puzzle(g.clone()));
                                        set_puzzle_feedback.set(None);
                                    }>"Cancel"</button>
                                </div>
                                <p class="puzzle-hint">
                                    "💡 Hint: " {hint_text}
                                </p>
                            </div>
                        </div>
                    }.into_any()
                })
            }}

            // === Game Grid ===
            <div class="adventure-grid-container" node_ref=grid_container_ref>
                <div class="adventure-grid" style={move || {
                    let g = game.get();
                    let levels = levels_signal.get();
                    let width = levels.get(g.current_level).map(|l| l.width).unwrap_or(first_level_width);
                    format!("grid-template-columns: repeat({}, var(--tile-size))", width)
                }}>
                    {move || {
                        let g = game.get();
                        let levels = levels_signal.get();
                        let grid = &g.tile_grid;
                        let player_pos = g.player_pos;
                        let collected = &g.collected_keys;
                        let solved = &g.solved_puzzles;
                        let animating = gate_animating.get();

                        let mut tiles_out = Vec::new();
                        for (row_idx, row) in grid.iter().enumerate() {
                            for (col_idx, tile) in row.iter().enumerate() {
                                let is_player = (col_idx, row_idx) == player_pos;
                                let tile_class = match tile {
                                    Tile::Floor | Tile::PlayerStart => "tile-floor",
                                    Tile::Wall => "tile-wall",
                                    Tile::Exit => {
                                        if check_exit_unlocked(&g, &levels) {
                                            "tile-exit tile-exit-unlocked"
                                        } else {
                                            "tile-exit"
                                        }
                                    }
                                    Tile::Key { name, .. } => {
                                        if collected.contains(name) { "tile-floor" } else { "tile-key" }
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
                                        Tile::Exit => "▶".to_string(),
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
                    }}
                </div>
            </div>

            // D-pad for mobile
            <div class="adventure-dpad">
                <div class="dpad-row">
                    <button class="dpad-btn dpad-up" on:click=move |_| dpad_move(engine::Direction::Up)>
                        "▲"
                    </button>
                </div>
                <div class="dpad-row">
                    <button class="dpad-btn dpad-left" on:click=move |_| dpad_move(engine::Direction::Left)>
                        "◀"
                    </button>
                    <div class="dpad-center"></div>
                    <button class="dpad-btn dpad-right" on:click=move |_| dpad_move(engine::Direction::Right)>
                        "▶"
                    </button>
                </div>
                <div class="dpad-row">
                    <button class="dpad-btn dpad-down" on:click=move |_| dpad_move(engine::Direction::Down)>
                        "▼"
                    </button>
                </div>
            </div>

            // Controls hint
            <div class="adventure-controls-hint">
                <span>"Arrow keys / WASD / hjkl to move"</span>
            </div>

            // Back link
            <div class="adventure-footer">
                <a href="/" class="adventure-back">"← Back to BeThere"</a>
            </div>
        </div>
    }
}

fn test_level_fallback() -> LevelData {
    LevelData {
        id: "fallback".to_string(),
        name: "Fallback".to_string(),
        concept: "Fallback".to_string(),
        width: 8,
        height: 6,
        grid: vec![
            "########".to_string(),
            "#@.....#".to_string(),
            "#......#".to_string(),
            "#......#".to_string(),
            "#.....>#".to_string(),
            "########".to_string(),
        ],
        keys: vec![],
        npcs: vec![],
        gates: vec![],
        signs: vec![],
        puzzles: vec![],
        required_keys: vec![],
        intro_text: "Empty level.".to_string(),
        completion_text: "Done!".to_string(),
    }
}
