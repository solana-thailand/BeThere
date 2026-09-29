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

use super::grid_view::tile_views;
use super::puzzle_view::{PuzzleCtx, dismiss_notification_later, puzzle_overlay};
use super::{default_levels, engine, levels::fallback_level, types::*};
use crate::api::{self, AdventureLevelScore};
use crate::icons::{Icon, IconName};

/// localStorage key for casual (no-token) progress.
const LS_COMPLETED_KEY: &str = "adventure_completed_levels";

// ============================================================
// Main Adventure Component
// ============================================================

#[component]
pub fn Adventure() -> impl IntoView {
    let levels = default_levels();
    let first_level = levels.first().cloned().unwrap_or_else(fallback_level);

    // Game state signal
    let initial_state = engine::init_game_state(&first_level);
    let (game, set_game) = signal(initial_state);

    // Levels data
    let (levels_signal, set_levels) = signal(levels);
    let _ = set_levels;

    // UI state
    let (adventure_passed, set_adventure_passed) = signal(false);
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
    let event_id_param = move || query.get().get("event_id").map(|s| s.to_string());

    // Required level from API config (fetched when event_id is present in casual mode)
    let (required_level_from_api, set_required_level_from_api) = signal::<Option<usize>>(None);
    let (event_slug, set_event_slug) = signal::<Option<String>>(None);
    // Claim token returned by quest-complete check-in (casual mode)
    let (quest_claim_token, set_quest_claim_token) = signal::<Option<String>>(None);

    // Restore progress: API if token present, localStorage otherwise
    let restore_token = _claim_token();
    let restore_event_id = event_id_param();
    let restore_levels = levels_signal;
    let restore_set_game = set_game;
    let restore_set_completed = set_completed_levels;
    let restore_set_required_level = set_required_level_from_api;
    let restore_set_adventure_passed = set_adventure_passed;
    let restore_set_event_slug = set_event_slug;
    let restore_completed_read = completed_levels;
    Effect::new(move |_| {
        if let Some(ref token) = restore_token {
            // Restore from API (claim flow)
            let token = token.clone();
            let levels = restore_levels.get();
            let set_g = restore_set_game;
            let set_completed = restore_set_completed;
            let eid_for_status = restore_event_id.clone();
            leptos::task::spawn_local(async move {
                match api::get_adventure_status(&token, eid_for_status.as_deref()).await {
                    Ok(status_data) => {
                        if let Some(progress) = status_data.progress
                            && !progress.levels_completed.is_empty()
                        {
                            // Find which level indices are completed
                            let completed_indices: HashSet<usize> = levels
                                .iter()
                                .enumerate()
                                .filter(|(_, l)| progress.levels_completed.contains(&l.id))
                                .map(|(i, _)| i)
                                .collect();
                            set_completed.set(completed_indices.clone());

                            // Find the first uncompleted level
                            let next_level_idx = levels
                                .iter()
                                .position(|l| !progress.levels_completed.contains(&l.id))
                                .unwrap_or(0);
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
                    Err(e) => {
                        log::warn!("[adventure] failed to restore progress: {e}");
                        // Continue with default — first level with intro
                    }
                }
            });
        } else {
            // Restore from localStorage (casual play)
            let stored = gloo_utils::window()
                .local_storage()
                .ok()
                .flatten()
                .and_then(|ls| ls.get(LS_COMPLETED_KEY).ok().flatten());
            if let Some(json) = stored
                && let Ok(indices) = serde_json::from_str::<HashSet<usize>>(&json)
            {
                log::info!(
                    "[adventure] restored {} completed levels from localStorage",
                    indices.len()
                );
                let levels = restore_levels.get();
                let next_idx = (0..levels.len())
                    .find(|i| !indices.contains(i))
                    .unwrap_or(0);
                if let Some(level) = levels.get(next_idx) {
                    let mut state = engine::init_game_state(level);
                    state.current_level = next_idx;
                    state.showing_intro = false;
                    restore_set_game.set(state);
                }
                restore_set_completed.set(indices);
            }

            // Fetch adventure config from API to determine required_level
            if let Some(ref eid) = restore_event_id {
                let eid = eid.clone();
                let set_rl = restore_set_required_level;
                let set_ap = restore_set_adventure_passed;
                let set_slug = restore_set_event_slug;
                let completed_read = restore_completed_read;
                leptos::task::spawn_local(async move {
                    match api::get_public_adventure_config(&eid).await {
                        Ok(config) => {
                            // Store slug for navigation links
                            if !config.event_slug.is_empty() {
                                set_slug.set(Some(config.event_slug.clone()));
                            }
                            if config.enabled {
                                log::info!(
                                    "[adventure] config: enabled={}, required_level={:?}",
                                    config.enabled,
                                    config.required_level
                                );
                                set_rl.set(config.required_level);
                                // Check if already passed from restored progress
                                if let Some(req_lvl) = config.required_level {
                                    let completed = completed_read.get();
                                    // required_level is 0-based in config
                                    // e.g. required_level=2 means levels 0,1,2 must be completed
                                    let all_done = (0..=req_lvl).all(|i| completed.contains(&i));
                                    if all_done {
                                        log::info!(
                                            "[adventure] casual mode: already passed from restored progress"
                                        );
                                        set_ap.set(true);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[adventure] failed to fetch config: {e}");
                        }
                    }
                });
            }
        }
    });

    // Timer — increments every second while level is active. Cleared on
    // unmount: an uncleared interval kept reading `game` after navigation,
    // and `.get()` on a disposed signal panics (a wasm trap every second).
    // `.with` also avoids cloning the whole GameState per tick.
    let game_for_timer = game;
    if let Ok(timer) = set_interval_with_handle(
        move || {
            let running = game_for_timer
                .try_with(|g| !g.showing_intro && !g.level_completed)
                .unwrap_or(false);
            if running {
                set_elapsed_seconds.update(|t| *t += 1);
            }
        },
        std::time::Duration::from_secs(1),
    ) {
        on_cleanup(move || timer.clear());
    }

    // Reactive scroll — fires whenever player position changes
    let scroll_game = game;
    let scroll_grid_ref = grid_container_ref;
    Effect::new(move |_| {
        let _pos = scroll_game.with(|g| g.player_pos);
        // Trigger on player position change

        // Defer scroll to next frame so DOM has updated
        let grid_ref = scroll_grid_ref;
        request_animation_frame(move || {
            let Some(el) = grid_ref.get() else { return };
            let (col, row) = _pos;
            let tile_size = 48.0_f64;
            let container_width = el.client_width() as f64;
            let container_height = el.client_height() as f64;
            let levels = levels_signal.read();
            let current_level = scroll_game.with(|g| g.current_level);
            let grid_width = levels.get(current_level).map(|l| l.width).unwrap_or(12) as f64;
            let grid_height = levels.get(current_level).map(|l| l.height).unwrap_or(8) as f64;
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
    let auto_save_game = game;
    let auto_save_levels = levels_signal;
    let auto_save_elapsed = elapsed_seconds;
    let auto_save_set_status = set_save_status;
    let auto_save_set_completed = set_completed_levels;
    let auto_save_set_adventure_passed = set_adventure_passed;
    let auto_save_completed_read = completed_levels;
    let auto_save_required_level = required_level_from_api;
    Effect::new(move |_| {
        let g = auto_save_game.read();
        if !g.level_completed {
            return;
        }
        let levels = auto_save_levels.read();
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
            let stars =
                engine::calculate_stars(g.moves_count, g.solved_puzzles.len() as u32, elapsed);
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
                    Ok(progress) => {
                        log::info!("[adventure] saved progress for level {level_id_clone}");
                        if progress.passed {
                            log::info!("[adventure] adventure passed!");
                            auto_save_set_adventure_passed.set(true);
                        }
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
            let mut completed = auto_save_completed_read.get();
            completed.insert(level_idx);
            if let Ok(json) = serde_json::to_string(&completed)
                && let Some(ls) = gloo_utils::window().local_storage().ok().flatten()
            {
                if ls.set(LS_COMPLETED_KEY, &json).is_err() {
                    log::warn!("[adventure] failed to save to localStorage");
                } else {
                    log::info!(
                        "[adventure] saved {} completed levels to localStorage",
                        completed.len()
                    );
                }
            }

            // Check if adventure is passed in casual mode
            // required_level is 0-based in config: n means levels 0..=n must be completed
            let req_lvl = auto_save_required_level.get();
            if let Some(req_lvl) = req_lvl {
                let all_required_done = (0..=req_lvl).all(|i| completed.contains(&i));
                if all_required_done {
                    log::info!(
                        "[adventure] casual mode: quest passed (required level index {req_lvl} completed)"
                    );
                    auto_save_set_adventure_passed.set(true);
                }
            }
        }
    });

    // Trigger virtual check-in when adventure is passed in casual mode
    let quest_event_id = event_id_param();
    let quest_adventure_passed = adventure_passed;
    let _quest_event_slug = event_slug;
    let (quest_checkin_done, set_quest_checkin_done) = signal(false);
    Effect::new(move |_| {
        if quest_adventure_passed.get()
            && !quest_checkin_done.get()
            && !has_token
            && let Some(ref eid) = quest_event_id
        {
            let eid = eid.clone();
            let set_done = set_quest_checkin_done;
            let set_slug = set_event_slug;
            let set_ct = set_quest_claim_token;
            leptos::task::spawn_local(async move {
                match api::quest_complete_checkin(&eid).await {
                    Ok(data) => {
                        log::info!("[adventure] virtual check-in done: {}", data.status);
                        if !data.event_slug.is_empty() {
                            set_slug.set(Some(data.event_slug));
                        }
                        if let Some(ct) = data.claim_token
                            && !ct.is_empty()
                        {
                            log::info!("[adventure] got claim token from check-in");
                            set_ct.set(Some(ct));
                        }
                        set_done.set(true);
                    }
                    Err(e) => {
                        log::warn!("[adventure] virtual check-in failed: {e}");
                        set_done.set(true); // Don't retry
                    }
                }
            });
        }
    });

    // Dismiss intro on first interaction
    let dismiss_intro = move || {
        if game.with(|g| g.showing_intro) {
            set_game.update(|g| g.showing_intro = false);
            set_elapsed_seconds.set(0);
        }
    };

    // Load a level by index
    let load_level = move |level_idx: usize| {
        let levels = levels_signal.read();
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
    let auto_dismiss_notification = move || dismiss_notification_later(set_notification);

    // One move, shared by keyboard, d-pad and swipe. Every input path must
    // stop at a finished level: the auto-save Effect tracks `game`, so a move
    // after completion would re-run it and post another save.
    let step = move |dir: engine::Direction| {
        let g = game.get();
        if g.active_puzzle.is_some() || g.showing_intro || g.level_completed {
            return;
        }
        let (new_state, result) = engine::apply_move(g, dir);
        match &result {
            MoveResult::CollectedKey { name, description } => {
                set_notification.set(Some(format!("Collected: {name} — {description}")));
                auto_dismiss_notification();
            }
            MoveResult::ExitReached => {
                let levels = levels_signal.read();
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
                    set_notification.set(Some("Level Complete!".to_string()));
                    return;
                }
            }
            MoveResult::HitCodeBlock { puzzle_id } | MoveResult::HitGate { puzzle_id } => {
                let levels = levels_signal.read();
                set_game.update(|g| {
                    *g = engine::open_puzzle_by_id(g.clone(), puzzle_id, &levels);
                });
                set_notification.set(Some(
                    "Gate locked! Solve the puzzle to open it.".to_string(),
                ));
                auto_dismiss_notification();
                return;
            }
            _ => {}
        }
        set_game.set(new_state);
    };

    // Handle keyboard input — global listener
    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        let (has_dialog, showing_intro, has_puzzle) = game.with(|g| {
            (
                g.active_dialog.is_some(),
                g.showing_intro,
                g.active_puzzle.is_some(),
            )
        });

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
        if has_dialog {
            set_game.update(|g| *g = engine::dismiss_dialog(g.clone()));
            return;
        }

        // If intro is showing, any key dismisses
        if showing_intro {
            dismiss_intro();
            return;
        }

        // If puzzle is active, don't process movement
        if has_puzzle {
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
            step(dir);
        }
    };

    // Global keyboard listener, removed on unmount (dropping the handle does not).
    let keydown = window_event_listener(leptos::ev::keydown, handle_keydown);
    on_cleanup(move || keydown.remove());

    // D-pad handler for mobile
    let dpad_move = move |dir: engine::Direction| {
        dismiss_intro();
        if game.with(|g| g.active_dialog.is_some()) {
            set_game.update(|g| *g = engine::dismiss_dialog(g.clone()));
            return;
        }
        step(dir);
    };

    // Swipe support for mobile
    let (touch_start, set_touch_start) = signal(Option::<(f64, f64)>::None);

    let touch_start_handler = move |ev: web_sys::TouchEvent| {
        if let Some(touch) = ev.touches().get(0) {
            set_touch_start.set(Some((touch.client_x() as f64, touch.client_y() as f64)));
        }
    };

    let swipe_dpad_move = dpad_move;
    let touch_end_handler = move |ev: web_sys::TouchEvent| {
        if let Some((sx, sy)) = touch_start.get()
            && let Some(touch) = ev.changed_touches().get(0)
        {
            let ex = touch.client_x() as f64;
            let ey = touch.client_y() as f64;
            let dx = ex - sx;
            let dy = ey - sy;
            let min_swipe = 30.0;
            if dx.abs() < min_swipe && dy.abs() < min_swipe {
                return; // too short, ignore
            }
            let dir = if dx.abs() > dy.abs() {
                if dx > 0.0 {
                    engine::Direction::Right
                } else {
                    engine::Direction::Left
                }
            } else if dy > 0.0 {
                engine::Direction::Down
            } else {
                engine::Direction::Up
            };
            swipe_dpad_move(dir);
        }
        set_touch_start.set(None);
    };

    let first_level_width =
        levels_signal.with_untracked(|l| l.first().map(|l| l.width).unwrap_or(12));

    let puzzle_ctx = PuzzleCtx {
        game,
        set_game,
        levels_signal,
        puzzle_feedback,
        set_puzzle_feedback,
        set_notification,
        set_gate_animating,
    };

    // Format elapsed time as MM:SS
    let format_time = move || {
        let s = elapsed_seconds.get();
        format!("{:02}:{:02}", s / 60, s % 60)
    };

    view! {
        <Title text="Rust Adventures — Learn Rust by Playing" />
        <div class="adventure-page">
            // No-token banner: show quest mode info when event_id present
            <Show when=move || !has_token fallback=|| view! { <div></div> }>
                <div class="adventure-banner adventure-banner-info">
                    <span class="adventure-banner-icon">"💡"</span>
                    <span>{move || {
                        let eid = event_id_param();
                        let req_lvl = required_level_from_api.get();
                        if eid.is_some() && let Some(req_lvl) = req_lvl {
                            let lvl = req_lvl + 1; // 0-based to 1-based for display
                            format!("Quest Mode — complete Level {lvl} to pass the quest! Progress saves to your browser.")
                        } else if eid.is_some() {
                            "Quest Mode — loading requirements...".to_string()
                        } else {
                            "Playing in demo mode — progress saves to your browser.".to_string()
                        }
                    }}</span>
                </div>
            </Show>

            // Persistent quest-complete banner — shown whenever quest is passed so the user
            // can always navigate to claim/event even if they keep playing higher levels.
            <Show when=move || adventure_passed.get() && event_id_param().is_some()>
                <div class="adventure-banner adventure-banner-success">
                    <span class="adventure-banner-icon">"🎉"</span>
                    <span>"Quest complete! "</span>
                    {move || {
                        // Prefer: URL token > quest-complete token > event page slug
                        let claim_link = _claim_token().map(|t| format!("/claim/{t}"))
                            .or_else(|| quest_claim_token.get().map(|t| format!("/claim/{t}")));
                        let slug_link = event_slug.get().map(|s| format!("/e/{s}"));
                        if let Some(link) = claim_link {
                            view! {
                                <a class="adventure-banner-link" href={link}>
                                    "Claim your NFT Badge →"
                                </a>
                            }.into_any()
                        } else if let Some(link) = slug_link {
                            view! {
                                <a class="adventure-banner-link" href={link}>
                                    "Go to Event Page →"
                                </a>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
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
                        "Moves: " {move || game.with(|g| g.moves_count)}
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
                let g = game.read();
                let levels = levels_signal.read();
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
                    let keys: Vec<String> = game.with(|g| g.collected_keys.iter().cloned().collect());
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
                let g = game.read();
                let levels = levels_signal.read();
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
                    let levels = levels_signal.read();
                    let current = game.with(|g| g.current_level);
                    let completed = completed_levels.get();
                    let levels_vec: Vec<(usize, String, String, bool, bool)> = levels.iter()
                        .enumerate()
                        .map(|(i, l)| (i, l.name.clone(), l.concept.clone(), i == current, completed.contains(&i)))
                        .collect();
                    view! {
                        <div class="adventure-overlay" on:click=move |_| set_show_level_select.set(false)>
                            <div class="adventure-overlay-card adventure-level-select-card" role="dialog" aria-modal="true" aria-label="Select Level" on:click=move |ev| ev.stop_propagation()>
                                <h2><Icon icon=IconName::Map class="icon-sm" />" Select Level"</h2>
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
                let g = game.read();
                if g.showing_intro {
                    let levels = levels_signal.read();
                    let level_info = levels.get(g.current_level)
                        .map(|l| (l.intro_text.clone(), l.name.clone()));
                    let (intro, level_name) = level_info.unwrap_or_default();
                    let dialog_label = level_name.clone();
                    view! {
                        <div class="adventure-overlay" on:click=move |_| dismiss_intro()>
                            <div class="adventure-overlay-card adventure-intro-card" role="dialog" aria-modal="true" aria-label=dialog_label>
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
                let g = game.read();
                if g.level_completed {
                    let levels = levels_signal.read();
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
                    let save_status_view = match save_msg.as_deref() {
                        Some("saving") => Some(view! {
                            <div class="adventure-save-status"><Icon icon=IconName::Save class="icon-sm" />" Saving..."</div>
                        }.into_any()),
                        Some("saved") => Some(view! {
                            <div class="adventure-save-status"><Icon icon=IconName::Check class="icon-sm icon-success" />" Progress saved!"</div>
                        }.into_any()),
                        Some(msg) if msg.starts_with("error") => Some(view! {
                            <div class="adventure-save-status"><Icon icon=IconName::Warning class="icon-sm icon-danger" />" Save failed (offline mode)"</div>
                        }.into_any()),
                        _ => None,
                    };
                    view! {
                        <div class="adventure-overlay adventure-overlay-success">
                            <div class="adventure-overlay-card adventure-overlay-card-success" role="dialog" aria-modal="true" aria-label="Level Complete">
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
                                {if let Some(sv) = save_status_view {
                                    sv
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                                <div class="adventure-success-actions">
                                    {if !has_next {
                                        // All levels complete
                                        let claim_link = _claim_token().map(|t| format!("/claim/{t}"));
                                        view! {
                                            <div class="adventure-all-complete">
                                                "🏆 All levels complete! You're a Rust adventurer!"
                                            </div>
                                            {if let Some(link) = claim_link {
                                                view! {
                                                    <a class="btn adventure-claim-btn" href={link}>
                                                        "🎁 Claim your NFT Badge"
                                                    </a>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <button class="btn btn-outline" on:click=move |_| {
                                                        // Reset all progress and play from level 1
                                                        set_completed_levels.set(HashSet::new());
                                                        if let Some(ls) = gloo_utils::window().local_storage().ok().flatten() {
                                                            let _ = ls.remove_item(LS_COMPLETED_KEY);
                                                        }
                                                        if let Some(level) = levels_signal.read().first() {
                                                            let mut state = engine::init_game_state(level);
                                                            state.current_level = 0;
                                                            set_game.set(state);
                                                        }
                                                    }>
                                                        <Icon icon=IconName::Refresh class="icon-sm" />" Play Again"
                                                    </button>
                                                }.into_any()
                                            }}
                                        }.into_any()
                                    } else if adventure_passed.get() {
                                        // Required levels passed — can claim now
                                        let claim_link = _claim_token().map(|t| format!("/claim/{t}"))
                                            .or_else(|| quest_claim_token.get().map(|t| format!("/claim/{t}")));
                                        let slug_link = event_slug.get().map(|s| format!("/e/{s}"));
                                        view! {
                                            <div class="adventure-all-complete">
                                                "🎉 Quest complete! You've passed the adventure!"
                                            </div>
                                            {if let Some(link) = claim_link.clone() {
                                                view! {
                                                    <a class="btn adventure-claim-btn" href={link}>
                                                        "🎁 Claim your NFT Badge"
                                                    </a>
                                                }.into_any()
                                            } else if let Some(link) = slug_link {
                                                view! {
                                                    <a class="btn adventure-claim-btn" href={link}>
                                                        "→ Go to Event Page"
                                                    </a>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                            <button class="btn btn-outline" on:click=move |_| load_level(next_idx)>
                                                "Continue Playing →"
                                            </button>
                                        }.into_any()
                                    } else {
                                        // More levels to go
                                        view! {
                                            <button class="btn btn-primary" on:click=move |_| load_level(next_idx)>
                                                "Next Level →"
                                            </button>
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
                game.read().active_dialog.as_ref().map(|dialog| view! {
                    <div class="adventure-overlay" on:click=move |_| {
                        set_game.update(|g| *g = engine::dismiss_dialog(g.clone()));
                    }>
                        <div class="adventure-dialog-card" role="dialog" aria-modal="true" aria-label=dialog.npc_name.clone()>
                            <div class="dialog-speaker">{dialog.npc_name.clone()}</div>
                            <div class="dialog-text">{dialog.text.clone()}</div>
                            <p class="adventure-overlay-hint">"Press any key or tap to dismiss"</p>
                        </div>
                    </div>
                }.into_any())
            }}

            // === Puzzle Overlay ===
            {move || game.read().active_puzzle.as_ref().map(|ps| puzzle_overlay(ps, puzzle_ctx))}

            // === Game Grid ===
            <div class="adventure-grid-container" node_ref=grid_container_ref on:touchstart=touch_start_handler on:touchend=touch_end_handler>
                <div class="adventure-grid" style={move || {
                    let g = game.read();
                    let levels = levels_signal.read();
                    let width = levels.get(g.current_level).map(|l| l.width).unwrap_or(first_level_width);
                    format!("grid-template-columns: repeat({}, var(--tile-size))", width)
                }}>
                    {move || tile_views(&game.read(), &levels_signal.read(), gate_animating.get())}
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
                <span>"Arrow keys / WASD / swipe to move"</span>
            </div>

            // Back link
            <div class="adventure-footer">
                <a href="/" class="adventure-back">"← Back to BeThere"</a>
            </div>
        </div>
    }
}
