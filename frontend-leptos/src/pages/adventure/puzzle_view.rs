//! Code-puzzle overlay of the adventure page.
//!
//! A plain fn, not a component: it runs inside the page's reactive closure
//! over `game.active_puzzle`, exactly where the markup used to live.

use leptos::prelude::*;

use super::{engine, types::*};
use crate::icons::{Icon, IconName};

/// The page signals the puzzle overlay reads and writes.
#[derive(Clone, Copy)]
pub(super) struct PuzzleCtx {
    pub game: ReadSignal<GameState>,
    pub set_game: WriteSignal<GameState>,
    pub levels_signal: ReadSignal<Vec<LevelData>>,
    pub puzzle_feedback: ReadSignal<Option<bool>>,
    pub set_puzzle_feedback: WriteSignal<Option<bool>>,
    pub set_notification: WriteSignal<Option<String>>,
    pub set_gate_animating: WriteSignal<Option<String>>,
}

/// Clear the notification toast after 3 s.
pub(super) fn dismiss_notification_later(set_notification: WriteSignal<Option<String>>) {
    set_timeout(
        move || set_notification.set(None),
        std::time::Duration::from_secs(3),
    );
}

pub(super) fn puzzle_overlay(puzzle_state: &PuzzleState, ctx: PuzzleCtx) -> AnyView {
    let PuzzleCtx {
        game,
        set_game,
        levels_signal,
        puzzle_feedback,
        set_puzzle_feedback,
        set_notification,
        set_gate_animating,
    } = ctx;
    let puzzle = &puzzle_state.puzzle;
    let pid = puzzle.id().to_string();
    let hint_text = puzzle.hint().to_string();

    let instruction = match puzzle {
        PuzzleDef::Arrange {
            instruction,
            pieces,
            ..
        } => {
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
        PuzzleDef::FillBlank {
            instruction,
            code_template,
            options,
            ..
        } => {
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
        PuzzleDef::FixError {
            instruction,
            broken_code,
            options,
            ..
        } => {
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
        PuzzleDef::ShortAnswer {
            instruction,
            code_template,
            ..
        } => {
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
            }
            .into_any()
        }
        PuzzleDef::MatchPairs {
            instruction, pairs, ..
        } => {
            let inst = instruction.clone();
            let pairs_data: Vec<(String, String)> = pairs.clone();
            let matched = puzzle_state.matched_pairs.clone();
            let selected = puzzle_state.selected_left;
            let right_shuffle = puzzle_state.right_shuffle.clone();
            // Left items in canonical order, right items shuffled
            let left_items: Vec<String> = pairs_data.iter().map(|(l, _)| l.clone()).collect();
            let right_items_shuffled: Vec<(usize, String)> = right_shuffle
                .iter()
                .map(|&canonical_idx| {
                    let text = pairs_data
                        .get(canonical_idx)
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
                                                    if game.with(|g| g.active_puzzle.as_ref().is_some_and(|ps| ps.matched_pairs.len() == pairs_count)) {
                                                            set_notification.set(Some("All pairs matched!".to_string()));
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
            <div class="adventure-puzzle-card" role="dialog" aria-modal="true" aria-label="Code Puzzle">
                <h3>"🧩 Code Puzzle"</h3>
                {instruction}

                // Feedback
                {move || {
                    puzzle_feedback.get().map(|correct| {
                        if correct {
                            view! {
                                <div class="puzzle-feedback puzzle-correct">
                                    <Icon icon=IconName::Check class="icon-sm icon-success" />" Correct!"
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="puzzle-feedback puzzle-wrong">
                                    <Icon icon=IconName::Cross class="icon-sm icon-danger" />" Not quite. Try again!"
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
                                let levels = levels_signal.read();
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
                            set_notification.set(Some("Puzzle solved! Gate opened.".to_string()));
                            dismiss_notification_later(set_notification);
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
}
