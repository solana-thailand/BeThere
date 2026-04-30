//! Quiz editor component for the admin dashboard (Issue 003).
//!
//! Provides a visual interface for organizers to:
//! - Create and edit quiz questions with multiple-choice options
//! - Set correct answers and explanations
//! - Configure passing score, max attempts, optional timer
//! - Preview quiz as attendees see it
//! - Save to backend via API

use leptos::prelude::*;

use crate::api::{self, QuizConfigAdmin, QuizQuestionAdmin};
use crate::components::{self, ToastType};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a unique question ID (q1, q2, ...) that doesn't collide with existing.
fn generate_question_id(existing: &[QuizQuestionAdmin]) -> String {
    let max_num = existing
        .iter()
        .filter_map(|q| q.id.strip_prefix('q').and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    format!("q{}", max_num + 1)
}

/// Create a blank question with default options.
fn blank_question(id: String) -> QuizQuestionAdmin {
    QuizQuestionAdmin {
        id,
        text: String::new(),
        options: vec![
            "Option A".to_string(),
            "Option B".to_string(),
            "Option C".to_string(),
        ],
        correct_index: 0,
        explanation: None,
    }
}

/// Create a default quiz config with one blank question.
fn default_config() -> QuizConfigAdmin {
    QuizConfigAdmin {
        questions: vec![blank_question("q1".to_string())],
        passing_score_percent: 60,
        max_attempts: 3,
        time_limit_seconds: None,
    }
}

/// Format attempt limit display.
fn format_attempts(n: u8) -> String {
    match n {
        1 => "1 attempt".to_string(),
        n => format!("{n} attempts"),
    }
}

// ---------------------------------------------------------------------------
// Quiz Editor Component
// ---------------------------------------------------------------------------

/// Quiz editor component for the admin dashboard.
///
/// Loads the current quiz config, allows editing, and saves changes.
/// Takes a toast signal writer for displaying feedback.
#[component]
pub fn QuizEditor(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    /// Currently selected event ID, used to scope API calls to a specific event.
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    // State
    let (config, set_config) = signal(None::<QuizConfigAdmin>);
    let (configured, set_configured) = signal(false);
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (preview, set_preview) = signal(false);
    let (error, set_error) = signal(None::<String>);

    // Load quiz config on mount
    Effect::new(move |_| {
        set_loading.set(true);
        set_error.set(None);

        let set_configured = set_configured;
        let set_config = set_config;
        let set_loading = set_loading;
        let set_error = set_error;
        let set_toast = set_toast;

        let eid = active_event_id.get();
        leptos::task::spawn_local(async move {
            match api::get_admin_quiz(eid.as_deref()).await {
                Ok(data) => {
                    set_configured.set(data.configured);
                    if data.configured {
                        set_config.set(Some(QuizConfigAdmin {
                            questions: data.questions,
                            passing_score_percent: data.passing_score_percent,
                            max_attempts: data.max_attempts,
                            time_limit_seconds: data.time_limit_seconds,
                        }));
                    }
                }
                Err(e) => {
                    log::error!("[quiz-editor] failed to load quiz: {e}");
                    set_error.set(Some(format!("{e}")));
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load quiz: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_loading.set(false);
        });
    });

    // Reload helper closure (used by retry button)
    let reload = move || {
        set_loading.set(true);
        set_error.set(None);
        let set_configured = set_configured;
        let set_config = set_config;
        let set_loading = set_loading;
        let set_error = set_error;

        let eid = active_event_id.get();
        leptos::task::spawn_local(async move {
            match api::get_admin_quiz(eid.as_deref()).await {
                Ok(data) => {
                    set_configured.set(data.configured);
                    if data.configured {
                        set_config.set(Some(QuizConfigAdmin {
                            questions: data.questions,
                            passing_score_percent: data.passing_score_percent,
                            max_attempts: data.max_attempts,
                            time_limit_seconds: data.time_limit_seconds,
                        }));
                    }
                }
                Err(e) => {
                    set_error.set(Some(format!("{e}")));
                }
            }
            set_loading.set(false);
        });
    };

    // Handle create quiz (from empty state)
    let handle_create = move |_: web_sys::MouseEvent| {
        set_config.set(Some(default_config()));
        set_configured.set(true);
    };

    // Validate and save quiz
    let do_save = move || {
        let current = config.get();
        let Some(cfg) = current else {
            components::show_toast(&set_toast, "No quiz to save", ToastType::Warning);
            return;
        };

        // Validate: min 1 question
        if cfg.questions.is_empty() {
            components::show_toast(
                &set_toast,
                "Quiz must have at least 1 question",
                ToastType::Error,
            );
            return;
        }

        // Validate: each question
        for q in &cfg.questions {
            if q.text.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    &format!("Question '{}' has empty text", q.id),
                    ToastType::Error,
                );
                return;
            }
            if q.options.len() < 2 {
                components::show_toast(
                    &set_toast,
                    &format!("Question '{}' needs at least 2 options", q.id),
                    ToastType::Error,
                );
                return;
            }
            if (q.correct_index as usize) >= q.options.len() {
                components::show_toast(
                    &set_toast,
                    &format!("Question '{}' has invalid correct answer", q.id),
                    ToastType::Error,
                );
                return;
            }
            // Check for empty options
            for (oi, opt) in q.options.iter().enumerate() {
                if opt.trim().is_empty() {
                    components::show_toast(
                        &set_toast,
                        &format!("Question '{}' option {} is empty", q.id, oi + 1),
                        ToastType::Error,
                    );
                    return;
                }
            }
        }

        // Validate: passing score 1-100
        if cfg.passing_score_percent == 0 || cfg.passing_score_percent > 100 {
            components::show_toast(
                &set_toast,
                "Passing score must be between 1 and 100",
                ToastType::Error,
            );
            return;
        }

        // Validate: max attempts >= 1
        if cfg.max_attempts == 0 {
            components::show_toast(
                &set_toast,
                "Max attempts must be at least 1",
                ToastType::Error,
            );
            return;
        }

        // Check for unique IDs
        let mut seen_ids = std::collections::HashSet::new();
        for q in &cfg.questions {
            if !seen_ids.insert(&q.id) {
                components::show_toast(
                    &set_toast,
                    &format!("Duplicate question ID: '{}'", q.id),
                    ToastType::Error,
                );
                return;
            }
        }

        // All valid — save
        set_saving.set(true);
        let cfg_clone = cfg.clone();

        leptos::task::spawn_local(async move {
            let eid = active_event_id.get();
            match api::put_admin_quiz(&cfg_clone, eid.as_deref()).await {
                Ok(result) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Quiz saved: {} questions", result.questions_count),
                        ToastType::Success,
                    );
                    set_configured.set(true);
                }
                Err(e) => {
                    log::error!("[quiz-editor] save failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Save failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_saving.set(false);
        });
    };

    let handle_save = move |_: web_sys::MouseEvent| {
        do_save();
    };

    let handle_toggle_preview = move |_: web_sys::MouseEvent| {
        set_preview.update(|p| *p = !*p);
    };

    // Computed flags
    let show_loading = move || loading.get();
    let show_error = move || !loading.get() && error.get().is_some();
    let show_empty = move || !loading.get() && !configured.get() && error.get().is_none();
    let show_content = move || !loading.get() && configured.get() && config.get().is_some();

    view! {
        <div class="quiz-editor">
            // Loading state
            <Show when=show_loading fallback=|| view! { <div></div> }>
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    "Loading quiz..."
                </div>
            </Show>

            // Error state
            <Show when=show_error fallback=|| view! { <div></div> }>
                {move || {
                    let err = error.get().unwrap_or_default();
                    view! {
                        <div class="card quiz-error-card">
                            <div class="quiz-state-icon">"⚠️"</div>
                            <h3>"Failed to Load Quiz"</h3>
                            <p class="quiz-state-desc">{err}</p>
                            <button class="btn btn-outline btn-sm" on:click=move |_| reload()>"Retry"</button>
                        </div>
                    }.into_any()
                }}
            </Show>

            // Empty state (not configured)
            <Show when=show_empty fallback=|| view! { <div></div> }>
                <div class="card quiz-empty-card">
                    <div class="quiz-state-icon quiz-state-icon-lg">"📝"</div>
                    <h3>"No Quiz Configured"</h3>
                    <p class="quiz-state-desc">
                        "Create a quiz that attendees must pass before claiming their NFT.
                        Add questions about the event content to verify engagement."
                    </p>
                    <button class="btn btn-primary" on:click=handle_create>
                        "Create Quiz"
                    </button>
                </div>
            </Show>

            // Editor content
            <Show when=show_content fallback=|| view! { <div></div> }>
                // Header with actions
                <div class="quiz-editor-header">
                    <div class="quiz-editor-title-row">
                        <h2 class="quiz-editor-title">"Quiz Editor"</h2>
                        <div class="quiz-editor-actions">
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=handle_toggle_preview
                            >
                                {move || if preview.get() { "✏️ Edit" } else { "👁 Preview" }}
                            </button>
                            <button
                                class="btn btn-primary btn-sm"
                                on:click=handle_save
                                disabled=move || saving.get()
                            >
                                {move || if saving.get() { "Saving..." } else { "💾 Save" }}
                            </button>
                        </div>
                    </div>
                    // Configured indicator
                    {move || {
                        let is_configured = configured.get();
                        let badge_class = if is_configured { "badge badge-success" } else { "badge badge-warning" };
                        let badge_text = if is_configured { "Active" } else { "Draft" };
                        view! {
                            <span class=format!("quiz-status-badge {badge_class}")>{badge_text}</span>
                        }
                    }}
                </div>

                // Preview mode
                <Show when=move || preview.get() fallback=|| view! { <div></div> }>
                    {move || {
                        let cfg = config.get();
                        match cfg {
                            Some(c) => render_preview(&c),
                            None => view! { <div></div> }.into_any(),
                        }
                    }}
                </Show>

                // Edit mode
                <Show when=move || !preview.get() && config.get().is_some() fallback=|| view! { <div></div> }>
                    // Settings card
                    {move || {
                        let cfg = config.get();
                        let Some(_c) = cfg else { return view! { <div></div> }.into_any() };


                        view! {
                            <div class="card quiz-settings-card">
                                <h3 class="quiz-section-heading">"⚙️ Settings"</h3>
                                <div class="quiz-settings-grid">
                                    // Passing score
                                    <div class="quiz-setting-item">
                                        <label class="quiz-setting-label">"Passing Score"</label>
                                        <div class="quiz-setting-input-row">
                                            <input
                                                type="range"
                                                min="10"
                                                max="100"
                                                step="5"
                                                prop:value=move || config.get().map(|c| c.passing_score_percent.to_string()).unwrap_or_default()
                                                on:input=move |ev| {
                                                    if let Ok(val) = event_target_value(&ev).parse::<u8>() {
                                                        set_config.update(|c| {
                                                            if let Some(c) = c { c.passing_score_percent = val; }
                                                        });
                                                    }
                                                }
                                                class="quiz-range-input"
                                            />
                                            <span class="quiz-range-value">
                                                {move || format!("{}%", config.get().map(|c| c.passing_score_percent).unwrap_or(60))}
                                            </span>
                                        </div>
                                    </div>

                                    // Max attempts
                                    <div class="quiz-setting-item">
                                        <label class="quiz-setting-label">"Max Attempts"</label>
                                        <input
                                            type="number"
                                            min="1"
                                            max="10"
                                            prop:value=move || config.get().map(|c| c.max_attempts.to_string()).unwrap_or_default()
                                            on:input=move |ev| {
                                                if let Ok(val) = event_target_value(&ev).parse::<u8>() {
                                                    set_config.update(|c| {
                                                        if let Some(c) = c {
                                                            c.max_attempts = val.clamp(1, 10);
                                                        }
                                                    });
                                                }
                                            }
                                            class="quiz-number-input"
                                        />
                                        <span class="quiz-setting-hint">
                                            {move || format_attempts(config.get().map(|c| c.max_attempts).unwrap_or(3))}
                                        </span>
                                    </div>

                                    // Time limit (optional)
                                    <div class="quiz-setting-item">
                                        <label class="quiz-setting-label">"Time Limit"</label>
                                        <div class="quiz-setting-input-row">
                                            <input
                                                type="number"
                                                min="0"
                                                max="3600"
                                                step="30"
                                                placeholder="No limit"
                                                prop:value=move || config.get()
                                                    .and_then(|c| c.time_limit_seconds.map(|t| t.to_string()))
                                                    .unwrap_or_default()
                                                on:input=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    let parsed = val.parse::<u16>().ok().filter(|&v| v > 0);
                                                    set_config.update(|c| {
                                                        if let Some(c) = c { c.time_limit_seconds = parsed; }
                                                    });
                                                }
                                                class="quiz-number-input"
                                            />
                                            <span class="quiz-setting-hint">"seconds (optional)"</span>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }}

                    // Questions list
                    {move || {
                        let cfg = config.get();
                        let Some(c) = cfg else { return view! { <div></div> }.into_any() };
                        let total = c.questions.len();

                        view! {
                            <div class="quiz-questions-heading">
                                <h3 class="quiz-section-heading">
                                    {format!("📋 Questions ({total})")}
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

                                    view! {
                                        <div class="card quiz-question-card">
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
                                                    <button
                                                        class="quiz-ctrl-btn quiz-ctrl-delete"
                                                        disabled=total <= 1
                                                        on:click=move |_| {
                                                            sc.update(|c| {
                                                                if let Some(c) = c
                                                                    && c.questions.len() > 1
                                                                {
                                                                    c.questions.remove(idx);
                                                                }
                                                            });
                                                        }
                                                        title="Delete question"
                                                    >"✕"</button>
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
                    }}

                    // Bottom save bar
                    <div class="quiz-save-bar">
                        <button
                            class="btn btn-primary"
                            on:click=handle_save
                            disabled=move || saving.get()
                        >
                            {move || if saving.get() { "Saving..." } else { "💾 Save Quiz" }}
                        </button>
                    </div>
                </Show>
            </Show>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Preview renderer
// ---------------------------------------------------------------------------

/// Render the quiz in preview mode (as attendees see it).
fn render_preview(config: &QuizConfigAdmin) -> AnyView {
    let questions = config.questions.clone();
    let passing = config.passing_score_percent;
    let max_attempts = config.max_attempts;
    let time_limit = config.time_limit_seconds;
    let total = questions.len();

    let time_display = time_limit.map(|t| {
        if t >= 60 {
            format!("{}m {}s", t / 60, t % 60)
        } else {
            format!("{t}s")
        }
    });

    view! {
        <div class="quiz-preview">
            // Info bar
            <div class="card quiz-preview-info">
                <div class="quiz-preview-stat">
                    <span class="quiz-preview-stat-value">{total}</span>
                    <span class="quiz-preview-stat-label">"Questions"</span>
                </div>
                <div class="quiz-preview-stat">
                    <span class="quiz-preview-stat-value">{format!("{passing}%")}</span>
                    <span class="quiz-preview-stat-label">"To Pass"</span>
                </div>
                <div class="quiz-preview-stat">
                    <span class="quiz-preview-stat-value">{max_attempts}</span>
                    <span class="quiz-preview-stat-label">{format_attempts(max_attempts)}</span>
                </div>
                {match time_display {
                    Some(td) => view! {
                        <div class="quiz-preview-stat">
                            <span class="quiz-preview-stat-value">{td}</span>
                            <span class="quiz-preview-stat-label">"Time Limit"</span>
                        </div>
                    }.into_any(),
                    None => view! { <div></div> }.into_any(),
                }}
            </div>

            // Questions
            <div class="quiz-preview-questions">
                {questions.iter().enumerate().map(|(qi, q)| {
                    let question_text = q.text.clone();
                    let options = q.options.clone();

                    view! {
                        <div class="card quiz-preview-question">
                            <div class="quiz-preview-question-text">
                                {format!("{}. {question_text}", qi + 1)}
                            </div>
                            <div class="quiz-preview-options">
                                {options.iter().enumerate().map(|(oi, opt)| {
                                    let letter = (b'A' + oi as u8) as char;
                                    view! {
                                        <div class="quiz-preview-option">
                                            <span class="quiz-preview-option-letter">
                                                {format!("{letter}.")}
                                            </span>
                                            <span>{opt.clone()}</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>

            <div class="quiz-preview-note">
                "This is how attendees will see the quiz. Correct answers are hidden."
            </div>
        </div>
    }
    .into_any()
}
