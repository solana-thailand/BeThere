//! Adventure config editor — admin UI to enable/configure Rust Adventures.
//!
//! Allows event organizers to:
//! - Toggle adventure requirement on/off
//! - Set the minimum level required to pass

use leptos::prelude::*;

use crate::api::{self, AdventureConfigData};
use crate::components::{self, ToastType};

/// Level definitions matching the adventure game levels.
/// Used to populate the "required level" dropdown.
const ADVENTURE_LEVELS: &[&str] = &[
    "01 — Hello World",
    "02 — Variables",
    "03 — Data Types",
    "04 — Control Flow",
    "05 — Functions",
    "06 — Ownership",
    "07 — Structs & Enums",
    "08 — Pattern Matching",
    "09 — Error Handling",
    "10 — Traits",
];

#[component]
pub fn AdventureConfigEditor(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    let (config, set_config) = signal(AdventureConfigData::default());
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (dirty, set_dirty) = signal(false);

    // Load config when active event changes
    let load_event_id = active_event_id;
    Effect::new(move |_| {
        let event_id = load_event_id.get();
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::get_admin_adventure_config(event_id.as_deref()).await {
                Ok(cfg) => {
                    set_config.set(cfg);
                    set_dirty.set(false);
                }
                Err(e) => {
                    log::warn!("[adventure-config] failed to load: {e}");
                    set_config.set(AdventureConfigData::default());
                }
            }
            set_loading.set(false);
        });
    });

    // Save handler
    let save_config = move || {
        let cfg = config.get();
        let event_id = active_event_id.get();
        set_saving.set(true);
        leptos::task::spawn_local(async move {
            match api::put_admin_adventure_config(&cfg, event_id.as_deref()).await {
                Ok(_) => {
                    set_dirty.set(false);
                    set_toast.set(Some(components::ToastMessage {
                        text: "Adventure config saved!".to_string(),
                        toast_type: ToastType::Success,
                    }));
                }
                Err(e) => {
                    set_toast.set(Some(components::ToastMessage {
                        text: format!("Failed to save: {e}"),
                        toast_type: ToastType::Error,
                    }));
                }
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="admin-content-inner">
            <div class="admin-section-heading">"🦀 Adventure Configuration"</div>

            <Show
                when=move || !loading.get()
                fallback=move || view! {
                    <div class="page-loading">
                        <div class="spinner"></div>
                        <p>"Loading config..."</p>
                    </div>
                }
            >
                <div class="quiz-editor">
                    // Settings card
                    <div class="quiz-settings-card card">
                        <div class="quiz-section-heading">"Settings"</div>
                        <div class="quiz-settings-grid">
                            // Enable toggle
                            <div class="quiz-setting-item">
                                <label class="quiz-setting-label">
                                    "Require Adventure Completion"
                                </label>
                                <div class="quiz-setting-input-row">
                                    <label class="quiz-toggle-label">
                                        <input
                                            type="checkbox"
                                            class="quiz-toggle-checkbox"
                                            prop:checked=move || config.get().enabled
                                            on:input=move |ev| {
                                                let checked = event_target_checked(&ev);
                                                set_config.update(|c| c.enabled = checked);
                                                set_dirty.set(true);
                                            }
                                        />
                                        <span class="quiz-toggle-switch"></span>
                                        <span class="quiz-toggle-text">
                                            {move || if config.get().enabled { "Enabled" } else { "Disabled" }}
                                        </span>
                                    </label>
                                </div>
                                <p class="quiz-setting-hint">
                                    "When enabled, attendees must complete the Rust Adventure game before they can claim their NFT badge."
                                </p>
                            </div>

                            // Required level
                            <Show when=move || config.get().enabled fallback=|| view! { <div></div> }>
                                <div class="quiz-setting-item">
                                    <label class="quiz-setting-label">
                                        "Minimum Required Level"
                                    </label>
                                    <div class="quiz-setting-input-row">
                                        <select
                                            class="quiz-number-input"
                                            style="width: 100%; padding: 0.5rem 0.75rem;"
                                            on:change=move |ev| {
                                                let val = event_target_value(&ev);
                                                let level = val.parse::<usize>().ok();
                                                set_config.update(|c| c.required_level = level);
                                                set_dirty.set(true);
                                            }
                                        >
                                            <option value="">"All 10 levels (complete all)"</option>
                                            {ADVENTURE_LEVELS.iter().enumerate().map(|(i, name)| {
                                                let val = format!("{}", i);
                                                let name_str = name.to_string();
                                                let selected = move || {
                                                    config.get().required_level == Some(i)
                                                };
                                                view! {
                                                    <option value={val} selected={selected()}>
                                                        {name_str}
                                                    </option>
                                                }
                                            }).collect_view()}
                                        </select>
                                    </div>
                                    <p class="quiz-setting-hint">
                                        "The minimum level an attendee must reach. \"All 10 levels\" means they must complete every level."
                                    </p>
                                </div>
                            </Show>
                        </div>
                    </div>

                    // Save bar
                    <div class="quiz-save-bar">
                        <button
                            class="btn btn-primary"
                            disabled=move || !dirty.get() || saving.get()
                            on:click=move |_| save_config()
                        >
                            {move || if saving.get() { "Saving..." } else { "Save Adventure Config" }}
                        </button>
                        {move || if dirty.get() {
                            view! { <span style="color: var(--warning); font-size: 0.8rem; margin-left: 0.5rem;">"Unsaved changes"</span> }.into_any()
                        } else {
                            view! { <span style="color: var(--text-muted); font-size: 0.8rem; margin-left: 0.5rem;">"Up to date"</span> }.into_any()
                        }}
                    </div>

                    // Preview
                    <div class="quiz-preview card" style="margin-top: 1rem;">
                        <div class="quiz-section-heading">"Preview"</div>
                        <div class="quiz-preview-info">
                            <div class="quiz-preview-stat">
                                <span class="quiz-preview-stat-value">
                                    {move || if config.get().enabled { "✅ Required" } else { "⚪ Optional" }}
                                </span>
                                <span class="quiz-preview-stat-label">"Adventure"</span>
                            </div>
                            <div class="quiz-preview-stat">
                                <span class="quiz-preview-stat-value">
                                    {move || match config.get().required_level {
                                        Some(lvl) => format!("Level {}+", lvl + 1),
                                        None => "All 10".to_string(),
                                    }}
                                </span>
                                <span class="quiz-preview-stat-label">"Required"</span>
                            </div>
                        </div>
                        <p class="quiz-preview-note">
                            "Attendees will "
                            {move || if config.get().enabled {
                                "need to complete the Rust Adventure game before claiming their NFT badge."
                            } else {
                                "go directly to the claim page after passing the quiz (if enabled)."
                            }}
                        </p>
                    </div>
                </div>
            </Show>
        </div>
    }
}
