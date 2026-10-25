//! The `QuizEditor` admin component.

use leptos::prelude::*;

use crate::api::{self, QuizConfigAdmin, QuizQuestionAdmin};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

use super::helpers::{blank_question, default_config, export_json_download, format_attempts};
use super::preview::render_preview;
use super::questions::render_questions;

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
    let (original_config, set_original_config) = signal(None::<QuizConfigAdmin>);
    let (configured, set_configured) = signal(false);
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (preview, set_preview) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (confirm_delete_q, set_confirm_delete_q) = signal(None::<String>);
    let (show_import, set_show_import) = signal(false);
    let (import_json, set_import_json) = signal(String::new());
    let (import_mode, set_import_mode) = signal(false); // false = merge, true = replace
    let (confirm_clear_all, set_confirm_clear_all) = signal(false);
    let (confirm_delete_quiz, set_confirm_delete_quiz) = signal(false);

    // Dirty tracking: compare serialized current config vs original
    let is_dirty = Memo::new(move |_| {
        let current = config.get();
        let original = original_config.get();
        match (current, original) {
            (Some(c), Some(o)) => serde_json::to_string(&c).ok() != serde_json::to_string(&o).ok(),
            (Some(_), None) => true,
            _ => false,
        }
    });

    // Load quiz config on mount
    Effect::new(move |_| {
        set_loading.set(true);
        set_error.set(None);

        // Skip when no event selected
        if active_event_id.get().is_none() {
            set_loading.set(false);
            return;
        }

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
                        let cfg = QuizConfigAdmin {
                            questions: data.questions,
                            passing_score_percent: data.passing_score_percent,
                            max_attempts: data.max_attempts,
                            time_limit_seconds: data.time_limit_seconds,
                        };
                        set_config.set(Some(cfg.clone()));
                        set_original_config.set(Some(cfg));
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
                        let cfg = QuizConfigAdmin {
                            questions: data.questions,
                            passing_score_percent: data.passing_score_percent,
                            max_attempts: data.max_attempts,
                            time_limit_seconds: data.time_limit_seconds,
                        };
                        set_config.set(Some(cfg.clone()));
                        set_original_config.set(Some(cfg));
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
        let cfg = default_config();
        set_original_config.set(Some(cfg.clone()));
        set_config.set(Some(cfg));
        set_configured.set(true);
    };

    // Delete entire quiz (admin only)
    let handle_delete_quiz = move || {
        let eid = active_event_id.get();
        set_saving.set(true);
        leptos::task::spawn_local(async move {
            match api::delete_admin_quiz(eid.as_deref()).await {
                Ok(()) => {
                    set_config.set(None);
                    set_original_config.set(None);
                    set_configured.set(false);
                    components::show_toast(
                        &set_toast,
                        "Quiz deleted successfully",
                        ToastType::Success,
                    );
                }
                Err(e) => {
                    log::error!("[quiz-editor] delete quiz failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Delete failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_saving.set(false);
        });
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
                    // Update original to the saved state — resets dirty flag
                    set_original_config.set(Some(cfg_clone));
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

    // Import JSON handler
    let handle_import = move |_: web_sys::MouseEvent| {
        let raw = import_json.get();
        if raw.trim().is_empty() {
            components::show_toast(&set_toast, "Paste JSON to import", ToastType::Warning);
            return;
        }

        // Try parsing as array of questions first, then as full QuizConfigAdmin
        let imported_questions: Vec<QuizQuestionAdmin> =
            if let Ok(arr) = serde_json::from_str::<Vec<QuizQuestionAdmin>>(&raw) {
                arr
            } else if let Ok(full) = serde_json::from_str::<QuizConfigAdmin>(&raw) {
                full.questions
            } else {
                components::show_toast(
                    &set_toast,
                    "Invalid JSON — expected array of questions or full config",
                    ToastType::Error,
                );
                return;
            };

        // Validate imported questions
        for q in &imported_questions {
            if q.text.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    &format!("Imported question '{}' has empty text", q.id),
                    ToastType::Error,
                );
                return;
            }
            if q.options.len() < 2 {
                components::show_toast(
                    &set_toast,
                    &format!("Imported question '{}' needs at least 2 options", q.id),
                    ToastType::Error,
                );
                return;
            }
            if (q.correct_index as usize) >= q.options.len() {
                components::show_toast(
                    &set_toast,
                    &format!("Imported question '{}' has invalid correct_index", q.id),
                    ToastType::Error,
                );
                return;
            }
        }

        let count = imported_questions.len();

        set_config.update(|c| {
            if let Some(c) = c {
                // Remove any blank placeholder questions (empty text) that were auto-generated
                c.questions.retain(|q| !q.text.trim().is_empty());

                if import_mode.get() {
                    // Replace mode: overwrite questions, keep settings
                    c.questions = imported_questions;
                } else {
                    // Merge mode: append, always assign new sequential IDs to guarantee uniqueness
                    let max_num = c
                        .questions
                        .iter()
                        .filter_map(|id| {
                            id.id.strip_prefix('q').and_then(|n| n.parse::<u32>().ok())
                        })
                        .max()
                        .unwrap_or(0);

                    for (i, mut q) in imported_questions.into_iter().enumerate() {
                        let new_num = max_num + i as u32 + 1;
                        q.id = format!("q{new_num}");
                        c.questions.push(q);
                    }
                }
            }
        });

        let total = config.get().map(|c| c.questions.len()).unwrap_or(0);
        let mode_label = if import_mode.get() {
            "Replaced with"
        } else {
            "Imported"
        };
        components::show_toast(
            &set_toast,
            &format!("{mode_label} {count} questions. Total: {total}"),
            ToastType::Success,
        );
        set_show_import.set(false);
        set_import_json.set(String::new());
    };

    // Computed flags
    let has_event = move || active_event_id.get().is_some();
    let show_loading = move || loading.get() && has_event();
    let show_error = move || !loading.get() && error.get().is_some() && has_event();
    let show_empty =
        move || !loading.get() && !configured.get() && error.get().is_none() && has_event();
    let show_content = move || !loading.get() && configured.get() && config.get().is_some();

    view! {
        <div class="quiz-editor">
            // No event selected
            <Show when=move || !has_event() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    "Select an event to configure its quiz."
                </div>
            </Show>

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
                            <div class="quiz-state-icon"><Icon icon=IconName::Warning class="icon-lg icon-warning" /></div>
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
                    <div class="quiz-state-icon quiz-state-icon-lg"><Icon icon=IconName::Copy class="icon-2xl icon-info" /></div>
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
                                {move || if preview.get() {
                                    view! { <Icon icon=IconName::Palette class="icon-sm" />" Edit" }.into_any()
                                } else {
                                    view! { <Icon icon=IconName::Search class="icon-sm" />" Preview" }.into_any()
                                }}
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_: web_sys::MouseEvent| { set_show_import.set(true); }
                            >
                                "Import"
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_: web_sys::MouseEvent| {
                                    let cfg = config.get();
                                    if let Some(c) = cfg {
                                        let json_str = serde_json::to_string_pretty(&c).unwrap_or_default();
                                        export_json_download(&json_str, "quiz-config.json");
                                    }
                                }
                            >
                                "Export"
                            </button>
                            {
                                let is_confirming_del = confirm_delete_quiz;
                                let set_cdq = set_confirm_delete_quiz;
                                let do_delete = handle_delete_quiz;
                                view! {
                                    <button
                                        class=move || if is_confirming_del.get() { "btn btn-danger btn-sm" } else { "btn btn-outline btn-sm" }
                                        disabled=move || saving.get()
                                        on:click=move |_| {
                                            if !is_confirming_del.get() {
                                                set_cdq.set(true);
                                                let reset = set_confirm_delete_quiz;
                                                gloo_timers::callback::Timeout::new(3000, move || {
                                                    reset.set(false);
                                                }).forget();
                                            } else {
                                                set_cdq.set(false);
                                                do_delete();
                                            }
                                        }
                                    >{move || if is_confirming_del.get() { "⚠ Confirm Delete Quiz" } else { "Delete Quiz" }}</button>
                                }
                            }
                            {
                                let sc = set_config;
                                let is_confirming = confirm_clear_all;
                                let set_ca = set_confirm_clear_all;
                                view! {
                                    <button
                                        class=move || if is_confirming.get() { "btn btn-danger btn-sm" } else { "btn btn-outline btn-sm" }
                                        on:click=move |_| {
                                            if !is_confirming.get() {
                                                set_ca.set(true);
                                                let reset = set_confirm_clear_all;
                                                gloo_timers::callback::Timeout::new(3000, move || {
                                                    reset.set(false);
                                                }).forget();
                                            } else {
                                                set_ca.set(false);
                                                sc.update(|c| {
                                                    if let Some(c) = c {
                                                        c.questions = vec![blank_question("q1".to_string())];
                                                    }
                                                });
                                            }
                                        }
                                    >{move || if is_confirming.get() { "⚠ Confirm Clear All" } else { "Clear All" }}</button>
                                }
                            }
                            <button
                                class="btn btn-primary btn-sm"
                                on:click=handle_save
                                disabled=move || saving.get() || !is_dirty.get()
                            >
                                {move || if saving.get() {
                                    "Saving...".into_any()
                                } else {
                                    view! { <Icon icon=IconName::Save class="icon-sm" />" Save" }.into_any()
                                }}
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
                                <h3 class="quiz-section-heading">"Settings"</h3>
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
                    {move || render_questions(config, set_config, confirm_delete_q, set_confirm_delete_q)}

                    // Bottom save bar
                    <div class="quiz-save-bar">
                        <button
                            class="btn btn-primary"
                            on:click=handle_save
                            disabled=move || saving.get() || !is_dirty.get()
                        >
                            {move || if saving.get() { "Saving...".into_any() } else { view! { <Icon icon=IconName::Save class="icon-sm" />" Save Quiz" }.into_any() }}
                        </button>
                        {move || if is_dirty.get() {
                            view! { <span class="hint-dirty">"Unsaved changes"</span> }.into_any()
                        } else {
                            view! { <span class="hint-clean">"Up to date"</span> }.into_any()
                        }}
                    </div>

                    // Import JSON modal
                    <Show when=move || show_import.get() fallback=|| view! { <div></div> }>
                        <div class="quiz-modal-overlay" on:click=move |_| { set_show_import.set(false); }>
                            <div class="quiz-modal" role="dialog" aria-modal="true" aria-label="Import Questions" on:click=move |ev: web_sys::MouseEvent| { ev.stop_propagation(); }>
                                <div class="quiz-modal-header">
                                    <h3 class="quiz-modal-title">"Import Questions"</h3>
                                    <button class="quiz-ctrl-btn" aria-label="Close" on:click=move |_| { set_show_import.set(false); }>"✕"</button>
                                </div>
                                <div class="quiz-modal-body">
                                    <p class="quiz-modal-desc">
                                        "Paste JSON — either an array of questions or a full quiz config."
                                    </p>
                                    <div class="quiz-import-mode">
                                        <label class="quiz-setting-label">"Mode"</label>
                                        <div class="quiz-import-mode-btns">
                                            <button
                                                class=move || if !import_mode.get() { "btn btn-sm btn-primary" } else { "btn btn-sm btn-outline" }
                                                on:click=move |_| { set_import_mode.set(false); }
                                            >"Merge (append)"</button>
                                            <button
                                                class=move || if import_mode.get() { "btn btn-sm btn-primary" } else { "btn btn-sm btn-outline" }
                                                on:click=move |_| { set_import_mode.set(true); }
                                            >"Replace all"</button>
                                        </div>
                                    </div>
                                    <textarea
                                        class="quiz-textarea quiz-import-textarea"
                                        rows="12"
                                        placeholder=r#"[
  {
    "id": "q1",
    "text": "What is...?",
    "options": ["A", "B", "C"],
    "correct_index": 0,
    "explanation": "Because...",
    "enabled": true
  }
]"#
                                        prop:value=move || import_json.get()
                                        on:input=move |ev| {
                                            set_import_json.set(event_target_value(&ev));
                                        }
                                    ></textarea>
                                </div>
                                <div class="quiz-modal-footer">
                                    <button class="btn btn-outline btn-sm" on:click=move |_| { set_show_import.set(false); }>"Cancel"</button>
                                    <button class="btn btn-primary btn-sm" on:click=handle_import>"Import"</button>
                                </div>
                            </div>
                        </div>
                    </Show>
                </Show>
            </Show>
        </div>
    }
}
