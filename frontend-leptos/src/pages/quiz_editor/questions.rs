//! The question list of the quiz editor: one card per question, its
//! options, and the add-question button.

use leptos::prelude::*;

use crate::api::QuizConfigAdmin;
use crate::icons::{Icon, IconName};

use super::helpers::{blank_question, generate_question_id};

/// Renders the question cards for the current config. Called from the
/// editor's reactive closure, so it re-runs whenever `config` changes.
pub(super) fn render_questions(
    config: ReadSignal<Option<QuizConfigAdmin>>,
    set_config: WriteSignal<Option<QuizConfigAdmin>>,
    confirm_delete_q: ReadSignal<Option<String>>,
    set_confirm_delete_q: WriteSignal<Option<String>>,
) -> AnyView {
    let cfg = config.get();
    let Some(c) = cfg else {
        return view! { <div></div> }.into_any();
    };
    let total = c.questions.len();

    view! {
        <div class="quiz-questions-heading">
            <h3 class="quiz-section-heading">
                <Icon icon=IconName::Copy class="icon-sm" />{format!("Questions ({total})")}
            </h3>
        </div>

        <div class="quiz-questions-list">
            {c.questions.iter().enumerate().map(|(index, q)| {
                let sc = set_config;

                let idx = index;
                let is_first = index == 0;
                let is_last = index == total - 1;

                // Local signals prevent focus loss during typing.
                // Synced back to global config on blur (blur fires
                // before any click that triggers structural re-renders).
                let local_q_text = RwSignal::new(q.text.clone());
                let local_explanation = RwSignal::new(q.explanation.clone().unwrap_or_default());
                let local_session = RwSignal::new(q.session_title.clone().unwrap_or_default());

                view! {
                    <div class=move || {
                        let is_enabled = config.get()
                            .and_then(|c| c.questions.get(idx).map(|q| q.enabled))
                            .unwrap_or(true);
                        if is_enabled { "card quiz-question-card".to_string() } else { "card quiz-question-card quiz-question-disabled".to_string() }
                    }>
                        // Question header
                        <div class="quiz-question-header">
                            <span class="quiz-question-number">
                                {format!("Q{}", idx + 1)}
                            </span>
                            // ID (read-only, shown small)
                            {move || {
                                let q_id = config.get()
                                    .and_then(|c| c.questions.get(idx).map(|q| q.id.clone()))
                                    .unwrap_or_default();
                                view! {
                                    <span class="quiz-question-id">{q_id}</span>
                                }
                            }}
                            // Enable/disable toggle
                            {move || {
                                let is_enabled = config.get()
                                    .and_then(|c| c.questions.get(idx).map(|q| q.enabled))
                                    .unwrap_or(true);
                                let sc_toggle = sc;
                                view! {
                                    <button
                                        class=if is_enabled { "quiz-ctrl-btn quiz-toggle-on" } else { "quiz-ctrl-btn quiz-toggle-off" }
                                        on:click=move |_| {
                                            sc_toggle.update(|c| {
                                                if let Some(c) = c
                                                    && let Some(q) = c.questions.get_mut(idx)
                                                {
                                                    q.enabled = !q.enabled;
                                                }
                                            });
                                        }
                                        title=if is_enabled { "Disable question" } else { "Enable question" }
                                    >{if is_enabled { "●" } else { "○" }}</button>
                                }
                            }}
                            // Move & delete controls
                            <div class="quiz-question-controls">
                                <button
                                    class="quiz-ctrl-btn"
                                    disabled=is_first
                                    on:click=move |_| {
                                        sc.update(|c| {
                                            if let Some(c) = c && idx > 0 {
                                                c.questions.swap(idx, idx - 1);
                                            }
                                        });
                                    }
                                    title="Move up"
                                >"↑"</button>
                                <button
                                    class="quiz-ctrl-btn"
                                    disabled=is_last
                                    on:click=move |_| {
                                        sc.update(|c| {
                                            if let Some(c) = c
                                                && idx < c.questions.len() - 1
                                            {
                                                c.questions.swap(idx, idx + 1);
                                            }
                                        });
                                    }
                                    title="Move down"
                                >"↓"</button>
                                {
                                    let q_id = config.get()
                                        .and_then(|c| c.questions.get(idx).map(|q| q.id.clone()))
                                        .unwrap_or_default();
                                    let is_confirming = confirm_delete_q.get().as_deref() == Some(&q_id);
                                    let q_id_clone = q_id.clone();
                                    let sc_del = sc;
                                    let set_cq = set_confirm_delete_q;
                                    view! {
                                        <button
                                            class=if is_confirming { "quiz-ctrl-btn quiz-ctrl-delete btn-confirm-danger" } else { "quiz-ctrl-btn quiz-ctrl-delete" }
                                            disabled=total <= 1
                                            on:click=move |_| {
                                                if !is_confirming {
                                                    set_cq.set(Some(q_id_clone.clone()));
                                                    let set_reset = set_confirm_delete_q;
                                                    gloo_timers::callback::Timeout::new(3000, move || {
                                                        set_reset.set(None);
                                                    }).forget();
                                                } else {
                                                    set_cq.set(None);
                                                    sc_del.update(|c| {
                                                        if let Some(c) = c
                                                            && c.questions.len() > 1
                                                        {
                                                            c.questions.remove(idx);
                                                        }
                                                    });
                                                }
                                            }
                                            title="Delete question"
                                        >{if is_confirming { "⚠?" } else { "✕" }}</button>
                                    }
                                }
                            </div>
                        </div>

                        // Session selector
                        <div class="quiz-field">
                            <label class="quiz-field-label">"Session"</label>
                            <div class="quiz-setting-input-row">
                                <input
                                    type="text"
                                    class="quiz-option-input"
                                    placeholder="e.g., Session 1 (optional)"
                                    prop:value=move || local_session.get()
                                    on:input=move |ev| {
                                        local_session.set(event_target_value(&ev));
                                    }
                                    on:blur=move |_| {
                                        let val = local_session.get();
                                        let sid = if val.is_empty() { None } else { Some(format!("session-{}", idx + 1)) };
                                        let stitle = if val.is_empty() { None } else { Some(val) };
                                        sc.update(|c| {
                                            if let Some(c) = c
                                                && let Some(q) = c.questions.get_mut(idx)
                                            {
                                                q.session_id = sid;
                                                q.session_title = stitle;
                                            }
                                        });
                                    }
                                />
                                <span class="quiz-setting-hint">"Group questions by session/topic"</span>
                            </div>
                        </div>

                        // Question text
                        <div class="quiz-field">
                            <label class="quiz-field-label">"Question"</label>
                            <textarea
                                class="quiz-textarea"
                                rows="2"
                                placeholder="Enter your question..."
                                prop:value=move || local_q_text.get()
                                on:input=move |ev| {
                                    local_q_text.set(event_target_value(&ev));
                                }
                                on:blur=move |_| {
                                    let val = local_q_text.get();
                                    sc.update(|c| {
                                        if let Some(c) = c
                                            && let Some(q) = c.questions.get_mut(idx)
                                        {
                                            q.text = val;
                                        }
                                    });
                                }
                            ></textarea>
                        </div>

                        // Options
                        <div class="quiz-field">
                            <label class="quiz-field-label">"Options"</label>
                            {move || {
                                let opts = config.get()
                                    .and_then(|c| c.questions.get(idx).map(|q| q.options.clone()))
                                    .unwrap_or_default();
                                let correct = config.get()
                                    .and_then(|c| c.questions.get(idx).map(|q| q.correct_index))
                                    .unwrap_or(0);
                                let opts_count = opts.len();

                                opts.iter().enumerate().map(|(oi, opt_text)| {
                                    let sc2 = sc;
                                    let letter = (b'A' + oi as u8) as char;
                                    let local_opt_text = RwSignal::new(opt_text.clone());
                                    let is_correct = oi == correct as usize;
                                    let can_remove = opts_count > 2;

                                    view! {
                                        <div class="quiz-option-row">
                                            <button
                                                class=format!(
                                                    "quiz-option-radio{}",
                                                    if is_correct { " quiz-option-correct" } else { "" }
                                                )
                                                on:click=move |_| {
                                                    sc2.update(|c| {
                                                        if let Some(c) = c
                                                            && let Some(q) = c.questions.get_mut(idx)
                                                        {
                                                            q.correct_index = oi as u8;
                                                        }
                                                    });
                                                }
                                                title="Set as correct answer"
                                            >
                                                {if is_correct { "●" } else { "○" }}
                                            </button>
                                            <span class="quiz-option-letter">{format!("{letter}.")}</span>
                                            <input
                                                type="text"
                                                class="quiz-option-input"
                                                placeholder=format!("Option {letter}")
                                                prop:value=move || local_opt_text.get()
                                                on:input=move |ev| {
                                                    local_opt_text.set(event_target_value(&ev));
                                                }
                                                on:blur=move |_| {
                                                    let val = local_opt_text.get();
                                                    sc2.update(|c| {
                                                        if let Some(c) = c
                                                            && let Some(q) = c.questions.get_mut(idx)
                                                            && let Some(opt) = q.options.get_mut(oi)
                                                        {
                                                            *opt = val;
                                                        }
                                                    });
                                                }
                                            />
                                            <button
                                                class="quiz-ctrl-btn quiz-ctrl-delete"
                                                disabled=!can_remove
                                                on:click=move |_| {
                                                    sc2.update(|c| {
                                                        if let Some(c) = c
                                                            && let Some(q) = c.questions.get_mut(idx)
                                                            && q.options.len() > 2
                                                        {
                                                            q.options.remove(oi);
                                                            if (q.correct_index as usize) >= q.options.len() {
                                                                q.correct_index = (q.options.len() - 1) as u8;
                                                            }
                                                        }
                                                    });
                                                }
                                                title="Remove option"
                                            >"✕"</button>
                                        </div>
                                    }
                                }).collect_view()
                            }}

                            // Add option button
                            {move || {
                                let count = config.get()
                                    .and_then(|c| c.questions.get(idx).map(|q| q.options.len()))
                                    .unwrap_or(0);
                                let can_add = count < 8;
                                view! {
                                    <button
                                        class="btn btn-outline btn-sm quiz-add-option-btn"
                                        disabled=!can_add
                                        on:click=move |_| {
                                            sc.update(|c| {
                                                if let Some(c) = c
                                                    && let Some(q) = c.questions.get_mut(idx)
                                                    && q.options.len() < 8
                                                {
                                                    let letter = (b'A' + q.options.len() as u8) as char;
                                                    q.options.push(format!("Option {letter}"));
                                                }
                                            });
                                        }
                                    >
                                        "+ Option"
                                    </button>
                                }
                            }}
                        </div>

                        // Explanation (optional)
                        <div class="quiz-field">
                            <label class="quiz-field-label">"Explanation (optional)"</label>
                            <textarea
                                class="quiz-textarea quiz-textarea-sm"
                                rows="2"
                                placeholder="Why is this the correct answer?"
                                prop:value=move || local_explanation.get()
                                on:input=move |ev| {
                                    local_explanation.set(event_target_value(&ev));
                                }
                                on:blur=move |_| {
                                    let val = local_explanation.get();
                                    sc.update(|c| {
                                        if let Some(c) = c
                                            && let Some(q) = c.questions.get_mut(idx)
                                        {
                                            q.explanation = if val.is_empty() { None } else { Some(val) };
                                        }
                                    });
                                }
                            ></textarea>
                        </div>
                    </div>
                }
            }).collect_view()}

            // Add question button
            <div class="quiz-add-question">
                <button
                    class="btn btn-outline"
                    on:click=move |_| {
                        set_config.update(|c| {
                            if let Some(c) = c {
                                let id = generate_question_id(&c.questions);
                                c.questions.push(blank_question(id));
                            }
                        });
                    }
                >
                    "+ Add Question"
                </button>
            </div>
        </div>
    }.into_any()
}
