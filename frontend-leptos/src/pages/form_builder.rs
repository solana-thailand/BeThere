//! Form builder component for the admin dashboard (Issue #049 Phase 2).
//!
//! Provides a visual interface for organizers to:
//! - Configure registration form fields (add, edit, remove, reorder)
//! - Set field types (text, textarea, select, multiselect)
//! - Mark fields as profile-enriching (updates developer_profiles)
//! - Preview the form as attendees see it
//! - Save to backend via API

use leptos::prelude::*;

use crate::api::{
    self, FormFieldTypeAdmin, FormFieldConfigAdmin, RegistrationFormConfigAdmin, VALID_PROFILE_KEYS,
};
use crate::components::{self, ToastType};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a blank field with sensible defaults.
fn blank_field(fields: &[FormFieldConfigAdmin]) -> FormFieldConfigAdmin {
    let idx = next_uid(fields);
    FormFieldConfigAdmin {
        uid: idx,
        key: format!("field_{idx}"),
        label: String::new(),
        field_type: FormFieldTypeAdmin::Text,
        options: None,
        required: false,
        profile_field: false,
    }
}

/// Get next unique ID.
fn next_uid(fields: &[FormFieldConfigAdmin]) -> usize {
    fields.iter().map(|f| f.uid).max().unwrap_or(0) + 1
}

/// Generate a CSS class for the field type badge.
fn field_type_badge_class(ft: &FormFieldTypeAdmin) -> &'static str {
    match ft {
        FormFieldTypeAdmin::Text => "badge badge-info",
        FormFieldTypeAdmin::Textarea => "badge badge-info",
        FormFieldTypeAdmin::Select => "badge badge-warning",
        FormFieldTypeAdmin::Multiselect => "badge badge-warning",
    }
}

/// Sanitize a string into a valid field key (lowercase, underscores, no spaces).
fn sanitize_key(input: &str) -> String {
    input
        .to_lowercase()
        .replace(' ', "_")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

// ---------------------------------------------------------------------------
// Form Builder Component
// ---------------------------------------------------------------------------

/// Form builder component for the admin dashboard.
///
/// Loads the current form config for the active event, allows editing,
/// and saves changes via API.
#[component]
pub fn FormBuilder(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    /// Currently selected event ID.
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    // State
    let (config, set_config) = signal(None::<RegistrationFormConfigAdmin>);
    let (original_config, set_original_config) = signal(None::<RegistrationFormConfigAdmin>);
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (preview, set_preview) = signal(false);
    let (confirm_reset, set_confirm_reset) = signal(false);

    // Dirty tracking
    let is_dirty = Memo::new(move |_| {
        let current = config.get();
        let original = original_config.get();
        match (current, original) {
            (Some(c), Some(o)) => serde_json::to_string(&c).ok() != serde_json::to_string(&o).ok(),
            (Some(_), None) => true,
            _ => false,
        }
    });

    // Load form config when event changes
    Effect::new(move |_| {
        set_loading.set(true);
        set_error.set(None);
        set_confirm_reset.set(false);

        let eid = active_event_id.get();
        if eid.is_none() {
            set_loading.set(false);
            return;
        }

        let eid = eid.unwrap();
        leptos::task::spawn_local(async move {
            match api::get_form_config(&eid).await {
                Ok(mut cfg) => {
                    // Assign stable uids to fields loaded from API
                    for (i, field) in cfg.fields.iter_mut().enumerate() {
                        field.uid = i + 1;
                    }
                    set_config.set(Some(cfg.clone()));
                    set_original_config.set(Some(cfg));
                }
                Err(e) => {
                    log::error!("[form-builder] failed to load config: {e}");
                    set_error.set(Some(format!("{e}")));
                }
            }
            set_loading.set(false);
        });
    });

    // Add a new field
    let handle_add_field = move |_: web_sys::MouseEvent| {
        set_config.update(|c| {
            if let Some(cfg) = c {
                cfg.fields.push(blank_field(&cfg.fields));
            }
        });
    };

    // Remove a field by index
    let handle_remove_field = move |idx: usize| {
        set_config.update(|c| {
            if let Some(cfg) = c {
                cfg.fields.remove(idx);
            }
        });
    };

    // Move field up
    let handle_move_up = move |idx: usize| {
        if idx == 0 {
            return;
        }
        set_config.update(|c| {
            if let Some(cfg) = c {
                cfg.fields.swap(idx, idx - 1);
            }
        });
    };

    // Move field down
    let handle_move_down = move |idx: usize| {
        set_config.update(|c| {
            if let Some(cfg) = c {
                if idx + 1 < cfg.fields.len() {
                    cfg.fields.swap(idx, idx + 1);
                }
            }
        });
    };

    // Reset to defaults
    let handle_reset = move |_: web_sys::MouseEvent| {
        let default = RegistrationFormConfigAdmin::default();
        set_config.set(Some(default.clone()));
    };

    // Save
    let do_save = move || {
        let current = config.get();
        let Some(cfg) = current else {
            components::show_toast(&set_toast, "No config to save", ToastType::Warning);
            return;
        };

        // Validate: check for duplicate keys
        let mut keys = std::collections::HashSet::new();
        for field in &cfg.fields {
            if field.key.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    "All fields must have a key",
                    ToastType::Error,
                );
                return;
            }
            if field.label.trim().is_empty() {
                components::show_toast(
                    &set_toast,
                    "All fields must have a label",
                    ToastType::Error,
                );
                return;
            }
            if !keys.insert(field.key.clone()) {
                components::show_toast(
                    &set_toast,
                    &format!("Duplicate field key: '{}'", field.key),
                    ToastType::Error,
                );
                return;
            }
            // Select/Multiselect must have options
            if matches!(
                field.field_type,
                FormFieldTypeAdmin::Select | FormFieldTypeAdmin::Multiselect
            ) && field
                .options
                .as_ref()
                .map_or(true, |o| o.is_empty() || o.iter().any(|opt| opt.trim().is_empty()))
            {
                components::show_toast(
                    &set_toast,
                    &format!("Field '{}' needs non-empty options for Select/Multi-select", field.key),
                    ToastType::Error,
                );
                return;
            }
        }

        let eid = active_event_id.get();
        let Some(eid) = eid else {
            components::show_toast(&set_toast, "No event selected", ToastType::Warning);
            return;
        };

        set_saving.set(true);
        leptos::task::spawn_local(async move {
            match api::put_form_config(&eid, &cfg).await {
                Ok(saved) => {
                    set_original_config.set(Some(saved));
                    components::show_toast(&set_toast, "Form config saved", ToastType::Success);
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to save: {e}"),
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

    // Field count for display
    let field_count = Memo::new(move |_| {
        config.get().map(|c| c.fields.len()).unwrap_or(0)
    });

    let profile_field_count = Memo::new(move |_| {
        config
            .get()
            .map(|c| c.fields.iter().filter(|f| f.profile_field).count())
            .unwrap_or(0)
    });

    view! {
        <div class="admin-section">
            <div class="admin-section-header">
                <h2 class="admin-section-title">
                    "Registration Form Builder"
                </h2>
                <div class="admin-section-actions">
                    <Show when=move || is_dirty.get()>
                        <span class="badge badge-warning">"Unsaved changes"</span>
                    </Show>
                </div>
            </div>

            // No event selected
            <Show when=move || active_event_id.get().is_none() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    <p class="pe-detail-secondary">"Select an event to configure its registration form."</p>
                </div>
            </Show>

            // Loading
            <Show when=move || loading.get() && active_event_id.get().is_some() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    <p class="pe-detail-secondary">"Loading form config..."</p>
                </div>
            </Show>

            // Error
            <Show when=move || error.get().is_some() && !loading.get() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    <p class="pe-detail-secondary">{move || error.get().unwrap_or_default()}</p>
                </div>
            </Show>

            // Main content
            <Show when=move || config.get().is_some() && !loading.get() && active_event_id.get().is_some() fallback=|| view! { <div></div> }>
                <div class="form-builder">
                    // Toolbar
                    <div class="form-builder-toolbar">
                        <div class="form-builder-stats">
                            <span class="quiz-setting-label">"Fields: "</span>
                            <span class="setting-value">{field_count}</span>
                            <span class="quiz-setting-label" style="margin-left: 12px">"Profile-enriching: "</span>
                            <span class="setting-value">{profile_field_count}</span>
                        </div>
                        <div class="form-builder-actions">
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| set_preview.update(|p| *p = !*p)
                            >
                                {move || if preview.get() { "Edit" } else { "Preview" }}
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| set_confirm_reset.set(true)
                            >
                                "Reset to Defaults"
                            </button>
                            <button
                                class="btn btn-primary btn-sm"
                                disabled=move || saving.get() || !is_dirty.get()
                                on:click=handle_save
                            >
                                {move || if saving.get() { "Saving..." } else { "Save" }}
                            </button>
                        </div>
                    </div>

                    // Reset confirmation
                    <Show when=move || confirm_reset.get() fallback=|| view! { <div></div> }>
                        <div class="admin-confirm-bar">
                            <span>"Reset form to default fields?"</span>
                            <button class="btn btn-danger btn-sm" on:click=handle_reset>
                                "Yes, Reset"
                            </button>
                            <button class="btn btn-outline btn-sm" on:click=move |_| set_confirm_reset.set(false)>
                                "Cancel"
                            </button>
                        </div>
                    </Show>

                    // Section label editor
                    <Show when=move || !preview.get() fallback=|| view! { <div></div> }>
                        <div class="form-builder-section-label">
                            <label class="quiz-setting-label">"Section Label"</label>
                            <input
                                class="pe-input"
                                type="text"
                                placeholder="About You (optional — helps us plan better events)"
                                prop:value=move || config.get().map(|c| c.section_label.clone()).unwrap_or_default()
                                on:input=move |ev| {
                                    let val = event_target_value(&ev);
                                    set_config.update(|c| {
                                        if let Some(cfg) = c {
                                            cfg.section_label = val;
                                        }
                                    });
                                }
                            />
                        </div>
                    </Show>

                    // Preview mode
                    <Show when=move || preview.get() fallback=|| view! { <div></div> }>
                        {move || {
                            let cfg = config.get();
                            let Some(cfg) = cfg else { return ().into_any() };
                            let section_label = cfg.section_label.clone();
                            let fields = cfg.fields.clone();
                            view! {
                                <div class="form-builder-preview">
                                    <div class="pe-dev-profile-section">
                                        <label class="pe-label">{section_label}</label>
                                        <For
                                            each=move || fields.clone()
                                            key=|f| f.uid
                                            children=move |field: FormFieldConfigAdmin| {
                                                render_preview_field(field)
                                            }
                                        />
                                    </div>
                                </div>
                            }.into_any()
                        }}
                    </Show>

                    // Edit mode: field list
                    <Show when=move || !preview.get() fallback=|| view! { <div></div> }>
                        <div class="form-builder-fields">
                            {move || {
                                let current_fields = config.get()
                                    .map(|c| c.fields.clone())
                                    .unwrap_or_default();
                                let fields_len = current_fields.len();
                                current_fields.into_iter().enumerate().map(|(idx, field)| {
                                    let field_uid = field.uid;
                                    let field_key = field.key.clone();
                                    let is_first = idx == 0;
                                    let is_last = idx + 1 >= fields_len;
                                    let move_up_idx = idx;
                                    let move_down_idx = idx;
                                    let remove_idx = idx;
                                    view! {
                                        <div class="form-builder-field-card">
                                            <div class="form-builder-field-header">
                                                <div class="form-builder-field-reorder">
                                                    <button
                                                        class="btn btn-xs btn-ghost"
                                                        disabled=is_first
                                                        on:click=move |_| handle_move_up(move_up_idx)
                                                    >
                                                        "↑"
                                                    </button>
                                                    <button
                                                        class="btn btn-xs btn-ghost"
                                                        disabled=is_last
                                                        on:click=move |_| handle_move_down(move_down_idx)
                                                    >
                                                        "↓"
                                                    </button>
                                                </div>
                                                <div class="form-builder-field-info">
                                                    <span class="form-builder-field-key">{field_key.clone()}</span>
                                                    <span class=field_type_badge_class(&field.field_type)>
                                                        {field.field_type.label()}
                                                    </span>
                                                    {if field.profile_field {
                                                        view! { <span class="badge badge-success">"Profile"</span> }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                    {if field.required {
                                                        view! { <span class="badge badge-error">"Required"</span> }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </div>
                                                <button
                                                    class="btn btn-danger btn-xs"
                                                    on:click=move |_| handle_remove_field(remove_idx)
                                                >
                                                    "✕"
                                                </button>
                                            </div>
                                            {render_field_editor(field_uid, config, set_config)}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                            }}

                            // Add field button
                            <button
                                class="form-builder-add-btn"
                                on:click=handle_add_field
                            >
                                "+ Add Field"
                            </button>
                        </div>

                        // Profile field key reference
                        <div class="form-builder-help">
                            <details>
                                <summary class="quiz-setting-label">"Profile Field Keys (developer_profiles columns)"</summary>
                                <div class="form-builder-help-content">
                                    <p class="pe-detail-secondary">
                                        "Fields marked \"Profile\" with these keys will update the developer's profile across events:"
                                    </p>
                                    <div class="form-builder-key-list">
                                        {VALID_PROFILE_KEYS.iter().map(|k| {
                                            view! { <code class="form-builder-key">{*k}</code> }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                    <p class="pe-detail-secondary" style="margin-top: 8px">
                                        "Any other key will be stored in registration_responses but NOT update the profile."
                                    </p>
                                </div>
                            </details>
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Field Editor (inline editing within a field card)
// ---------------------------------------------------------------------------

fn render_field_editor(
    field_uid: usize,
    config: ReadSignal<Option<RegistrationFormConfigAdmin>>,
    set_config: WriteSignal<Option<RegistrationFormConfigAdmin>>,
) -> AnyView {
    view! {
        <div class="form-builder-field-body">
            <div class="form-builder-field-row">
                // Key
                <div class="form-builder-field-col">
                    <label class="quiz-setting-label">"Key"</label>
                    <input
                        class="pe-input"
                        type="text"
                        placeholder="e.g. experience_level"
                        prop:value=move || {
                            config.get()
                                .and_then(|c| c.fields.iter().find(|f| f.uid == field_uid).map(|f| f.key.clone()))
                                .unwrap_or_default()
                        }
                        on:input=move |ev| {
                            let val = sanitize_key(&event_target_value(&ev));
                            set_config.update(|c| {
                                if let Some(cfg) = c {
                                    if let Some(f) = cfg.fields.iter_mut().find(|f| f.uid == field_uid) {
                                        f.key = val;
                                    }
                                }
                            });
                        }
                    />
                </div>
                // Label
                <div class="form_builder-field-col">
                    <label class="quiz-setting-label">"Label"</label>
                    <input
                        class="pe-input"
                        type="text"
                        placeholder="e.g. Experience level"
                        prop:value=move || {
                            config.get()
                                .and_then(|c| c.fields.iter().find(|f| f.uid == field_uid).map(|f| f.label.clone()))
                                .unwrap_or_default()
                        }
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            set_config.update(|c| {
                                if let Some(cfg) = c {
                                    if let Some(f) = cfg.fields.iter_mut().find(|f| f.uid == field_uid) {
                                        f.label = val;
                                    }
                                }
                            });
                        }
                    />
                </div>
            </div>
            <div class="form-builder-field-row">
                // Type
                <div class="form-builder-field-col">
                    <label class="quiz-setting-label">"Type"</label>
                    <select
                        class="pe-input"
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let ft = match val.as_str() {
                                "text" => FormFieldTypeAdmin::Text,
                                "textarea" => FormFieldTypeAdmin::Textarea,
                                "select" => FormFieldTypeAdmin::Select,
                                "multiselect" => FormFieldTypeAdmin::Multiselect,
                                _ => FormFieldTypeAdmin::Text,
                            };
                            set_config.update(|c| {
                                if let Some(cfg) = c {
                                    if let Some(f) = cfg.fields.iter_mut().find(|f| f.uid == field_uid) {
                                        f.field_type = ft;
                                        // Clear options if switching to text/textarea
                                        if matches!(f.field_type, FormFieldTypeAdmin::Text | FormFieldTypeAdmin::Textarea) {
                                            f.options = None;
                                        } else if f.options.is_none() {
                                            f.options = Some(vec!["Option 1".to_string()]);
                                        }
                                    }
                                }
                            });
                        }
                    >
                        {FormFieldTypeAdmin::all().iter().map(|ft| {
                            let val = match ft {
                                FormFieldTypeAdmin::Text => "text",
                                FormFieldTypeAdmin::Textarea => "textarea",
                                FormFieldTypeAdmin::Select => "select",
                                FormFieldTypeAdmin::Multiselect => "multiselect",
                            };
                            let selected = config.get()
                                .and_then(|c| c.fields.iter().find(|f| f.uid == field_uid).map(|f| f.field_type == *ft))
                                .unwrap_or(false);
                            view! {
                                <option value=val selected=selected>{ft.label()}</option>
                            }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>
                // Toggles
                <div class="form-builder-field-col form-builder-toggles">
                    <label class="pe-checkbox-label">
                        <input
                            type="checkbox"
                            class="pe-checkbox"
                            checked=move || {
                                config.get()
                                    .and_then(|c| c.fields.iter().find(|f| f.uid == field_uid).map(|f| f.required))
                                    .unwrap_or(false)
                            }
                            on:change=move |ev| {
                                let val = event_target_checked(&ev);
                                set_config.update(|c| {
                                    if let Some(cfg) = c {
                                        if let Some(f) = cfg.fields.iter_mut().find(|f| f.uid == field_uid) {
                                            f.required = val;
                                        }
                                    }
                                });
                            }
                        />
                        <span>"Required"</span>
                    </label>
                    <label class="pe-checkbox-label">
                        <input
                            type="checkbox"
                            class="pe-checkbox"
                            checked=move || {
                                config.get()
                                    .and_then(|c| c.fields.iter().find(|f| f.uid == field_uid).map(|f| f.profile_field))
                                    .unwrap_or(false)
                            }
                            on:change=move |ev| {
                                let val = event_target_checked(&ev);
                                set_config.update(|c| {
                                    if let Some(cfg) = c {
                                        if let Some(f) = cfg.fields.iter_mut().find(|f| f.uid == field_uid) {
                                            f.profile_field = val;
                                        }
                                    }
                                });
                            }
                        />
                        <span>"Profile field"</span>
                    </label>
                </div>
            </div>
            // Options (for select/multiselect)
            {move || {
                let current = config.get();
                let field = current.as_ref()
                    .and_then(|c| c.fields.iter().find(|f| f.uid == field_uid));
                let Some(field) = field else { return ().into_any() };

                if !matches!(field.field_type, FormFieldTypeAdmin::Select | FormFieldTypeAdmin::Multiselect) {
                    return ().into_any();
                }

                let opts = field.options.clone().unwrap_or_default();
                let opt_uid = field.uid;

                view! {
                    <div class="form-builder-field-options">
                        <label class="quiz-setting-label">"Options (one per line)"</label>
                        <textarea
                            class="pe-input"
                            rows="3"
                            placeholder="Option 1&#10;Option 2&#10;Option 3"
                            prop:value=opts.join("\n")
                            on:input=move |ev| {
                                let text = event_target_value(&ev);
                                let new_opts: Vec<String> = text.lines()
                                    .map(|l| l.trim().to_string())
                                    .filter(|l| !l.is_empty())
                                    .collect();
                                set_config.update(|c| {
                                    if let Some(cfg) = c {
                                        if let Some(f) = cfg.fields.iter_mut().find(|f| f.uid == opt_uid) {
                                            f.options = if new_opts.is_empty() { None } else { Some(new_opts) };
                                        }
                                    }
                                });
                            }
                        ></textarea>
                    </div>
                }.into_any()
            }}
        </div>
    }.into_any()
}

// ---------------------------------------------------------------------------
// Preview Rendering
// ---------------------------------------------------------------------------

fn render_preview_field(field: FormFieldConfigAdmin) -> AnyView {
    let label = field.label.clone();
    let required = field.required;

    match field.field_type {
        FormFieldTypeAdmin::Text => view! {
            <div class="pe-field">
                <label class="pe-label">
                    {label.clone()}
                    {if required { view! { <span class="pe-required">" *"</span> }.into_any() } else { ().into_any() }}
                </label>
                <input type="text" class="pe-input" placeholder=label disabled=true />
            </div>
        }.into_any(),
        FormFieldTypeAdmin::Textarea => view! {
            <div class="pe-field">
                <label class="pe-label">
                    {label.clone()}
                    {if required { view! { <span class="pe-required">" *"</span> }.into_any() } else { ().into_any() }}
                </label>
                <textarea class="pe-input" rows="3" placeholder=label disabled=true></textarea>
            </div>
        }.into_any(),
        FormFieldTypeAdmin::Select => {
            let options = field.options.unwrap_or_default();
            view! {
                <div class="pe-multiselect-group">
                    <span class="pe-multiselect-label">
                        {label}
                        {if required { view! { <span class="pe-required">" *"</span> }.into_any() } else { ().into_any() }}
                    </span>
                    <select class="pe-input" disabled=true>
                        <option value="">"Select..."</option>
                        {options.iter().map(|opt| {
                            let o = opt.clone();
                            view! { <option>{o}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>
            }.into_any()
        }
        FormFieldTypeAdmin::Multiselect => {
            let options = field.options.unwrap_or_default();
            view! {
                <div class="pe-multiselect-group">
                    <span class="pe-multiselect-label">
                        {label}
                        {if required { view! { <span class="pe-required">" *"</span> }.into_any() } else { ().into_any() }}
                    </span>
                    <div class="pe-multiselect-options">
                        {options.iter().map(|opt| {
                            let o = opt.clone();
                            view! {
                                <label class="pe-multiselect-item">
                                    <input type="checkbox" class="pe-checkbox" disabled=true />
                                    <span>{o}</span>
                                </label>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            }.into_any()
        }
    }
}
