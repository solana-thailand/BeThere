//! Events management page — list, create, and configure events.
//!
//! Provides a full UI for listing, creating, editing, and archiving events
//! in the BeThere admin dashboard.

use leptos::prelude::*;

use crate::api;
use crate::components;

// ===== View State =====

/// Current view state for the events page.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EventsView {
    List,
    Create,
    Edit,
}

// ===== Form State =====

/// Form state for creating/editing events.
#[derive(Debug, Clone, Default)]
struct EventForm {
    name: String,
    slug: String,
    tagline: String,
    link: String,
    event_start: String,
    event_end: String,
    sheet_id: String,
    sheet_name: String,
    staff_sheet_name: String,
    quiz_enabled: bool,
    nft_collection_mint: String,
    nft_metadata_uri: String,
    nft_image_url: String,
    nft_name_template: String,
    nft_symbol: String,
    nft_description_template: String,
    claim_base_url: String,
    organizer_emails: String,
    staff_emails: String,
    status: api::EventStatus,
}

// ===== Helpers =====

/// Auto-generate a URL-safe slug from a name.
fn generate_slug(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Parse an ISO date string or epoch ms string to epoch milliseconds.
fn parse_date_to_ms(input: &str) -> Option<i64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Try parsing as epoch ms first
    if let Ok(ms) = trimmed.parse::<i64>() {
        return Some(ms);
    }
    // Try parsing as ISO date string via js_sys
    let ms = js_sys::Date::parse(trimmed);
    if ms.is_nan() {
        None
    } else {
        Some(ms as i64)
    }
}

/// Format epoch milliseconds to a short readable date string.
fn format_date_display(ms: i64) -> String {
    if ms == 0 {
        return "—".to_string();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let iso = date.to_iso_string().as_string().unwrap_or_default();
    // Truncate to YYYY-MM-DDTHH:MM:SS (19 chars)
    iso.chars().take(19).collect()
}

/// Create a default form state with sensible defaults.
fn default_form() -> EventForm {
    EventForm {
        sheet_name: "checkin".to_string(),
        staff_sheet_name: "staff".to_string(),
        ..Default::default()
    }
}

/// Create form state from an EventDetail (for editing).
fn form_from_detail(detail: &api::EventDetail) -> EventForm {
    EventForm {
        name: detail.name.clone(),
        slug: detail.slug.clone(),
        tagline: detail.tagline.clone(),
        link: detail.link.clone(),
        event_start: if detail.event_start_ms > 0 {
            format_date_display(detail.event_start_ms)
        } else {
            String::new()
        },
        event_end: if detail.event_end_ms > 0 {
            format_date_display(detail.event_end_ms)
        } else {
            String::new()
        },
        sheet_id: detail.sheet_id.clone(),
        sheet_name: if detail.sheet_name.is_empty() {
            "checkin".to_string()
        } else {
            detail.sheet_name.clone()
        },
        staff_sheet_name: if detail.staff_sheet_name.is_empty() {
            "staff".to_string()
        } else {
            detail.staff_sheet_name.clone()
        },
        quiz_enabled: detail.quiz_enabled,
        nft_collection_mint: detail.nft_collection_mint.clone(),
        nft_metadata_uri: detail.nft_metadata_uri.clone(),
        nft_image_url: detail.nft_image_url.clone(),
        nft_name_template: detail.nft_name_template.clone(),
        nft_symbol: detail.nft_symbol.clone(),
        nft_description_template: detail.nft_description_template.clone(),
        claim_base_url: detail.claim_base_url.clone(),
        organizer_emails: detail.organizer_emails.join(", "),
        staff_emails: detail.staff_emails.join(", "),
        status: detail.status.clone(),
    }
}

/// Parse comma-separated emails into a Vec.
fn parse_emails(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Get status badge CSS class.
fn status_badge_class(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "badge badge-success",
        api::EventStatus::Draft => "badge badge-warning",
        api::EventStatus::Completed => "badge",
        api::EventStatus::Archived => "badge",
    }
}

/// Get inline style for statuses without dedicated badge classes.
fn status_badge_style(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "",
        api::EventStatus::Draft => "",
        api::EventStatus::Completed => {
            "background:rgba(59,130,246,0.15);color:#3b82f6;border:1px solid rgba(59,130,246,0.3);"
        }
        api::EventStatus::Archived => {
            "background:rgba(107,114,128,0.15);color:#9ca3af;border:1px solid rgba(107,114,128,0.3);"
        }
    }
}

/// Get status display label.
fn status_label(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "🟢 Active",
        api::EventStatus::Draft => "📝 Draft",
        api::EventStatus::Completed => "✅ Completed",
        api::EventStatus::Archived => "📦 Archived",
    }
}

// ===== Component =====

/// Events management page component.
///
/// Provides a UI for listing, creating, editing, and archiving events.
/// Takes a toast signal writer for displaying feedback.
#[component]
pub fn EventsPage(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
) -> impl IntoView {
    // Get user role from ProtectedRoute context for role-based UI
    let user_role = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[events-page] no user_role in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });

    // State
    let (events, set_events) = signal(Vec::<api::EventMeta>::new());
    let (current_view, set_current_view) = signal(EventsView::List);
    let (editing_id, set_editing_id) = signal(None::<String>);
    let (form, set_form) = signal(default_form());
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (slug_manually_edited, set_slug_manually_edited) = signal(false);
    let (refresh_counter, set_refresh_counter) = signal(0u32);

    // Load events on mount and on refresh
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        set_loading.set(true);

        leptos::task::spawn_local(async move {
            match api::list_events().await {
                Ok(data) => {
                    set_events.set(data.events);
                }
                Err(e) => {
                    log::error!("[events-page] failed to load events: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load events: {e}"),
                        components::ToastType::Error,
                    );
                }
            }
            set_loading.set(false);
        });
    });

    // Reload helper
    let do_reload = move || {
        set_refresh_counter.update(|n| *n += 1);
    };

    // Handle create button
    let handle_create = move |_: web_sys::MouseEvent| {
        set_form.set(default_form());
        set_slug_manually_edited.set(false);
        set_current_view.set(EventsView::Create);
    };

    // Handle name input (auto-generate slug if not manually edited)
    let handle_name_input = move |ev| {
        let name = event_target_value(&ev);
        let edited = slug_manually_edited.get();
        set_form.update(|f| {
            f.name = name.clone();
            if !edited {
                f.slug = generate_slug(&name);
            }
        });
    };

    // Handle slug input
    let handle_slug_input = move |ev| {
        set_slug_manually_edited.set(true);
        set_form.update(|f| f.slug = event_target_value(&ev));
    };

    // Handle save (create or update)
    let handle_save = move |_: web_sys::MouseEvent| {
        let current_form = form.get();
        let is_create = current_view.get() == EventsView::Create;

        // Validate required fields
        if current_form.name.trim().is_empty() {
            components::show_toast(&set_toast, "Event name is required", components::ToastType::Error);
            return;
        }
        if current_form.slug.trim().is_empty() {
            components::show_toast(&set_toast, "Event slug is required", components::ToastType::Error);
            return;
        }
        if current_form.sheet_id.trim().is_empty() {
            components::show_toast(&set_toast, "Google Sheet ID is required", components::ToastType::Error);
            return;
        }

        let start_ms = parse_date_to_ms(&current_form.event_start).unwrap_or(0);
        let end_ms = parse_date_to_ms(&current_form.event_end).unwrap_or(0);

        set_saving.set(true);

        if is_create {
            let body = api::CreateEventBody {
                name: current_form.name.trim().to_string(),
                slug: current_form.slug.trim().to_string(),
                tagline: current_form.tagline.trim().to_string(),
                link: current_form.link.trim().to_string(),
                event_start_ms: start_ms,
                event_end_ms: end_ms,
                sheet_id: current_form.sheet_id.trim().to_string(),
                sheet_name: current_form.sheet_name.trim().to_string(),
                staff_sheet_name: current_form.staff_sheet_name.trim().to_string(),
                quiz_enabled: current_form.quiz_enabled,
                nft_collection_mint: current_form.nft_collection_mint.trim().to_string(),
                nft_metadata_uri: current_form.nft_metadata_uri.trim().to_string(),
                nft_image_url: current_form.nft_image_url.trim().to_string(),
                nft_name_template: current_form.nft_name_template.trim().to_string(),
                nft_symbol: current_form.nft_symbol.trim().to_string(),
                nft_description_template: current_form.nft_description_template.trim().to_string(),
                claim_base_url: current_form.claim_base_url.trim().to_string(),
                organizer_emails: parse_emails(&current_form.organizer_emails),
                staff_emails: parse_emails(&current_form.staff_emails),
            };

            leptos::task::spawn_local(async move {
                match api::create_event(&body).await {
                    Ok(data) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Event '{}' created", data.name),
                            components::ToastType::Success,
                        );
                        set_current_view.set(EventsView::List);
                        do_reload();
                    }
                    Err(e) => {
                        log::error!("[events-page] create failed: {e}");
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to create event: {e}"),
                            components::ToastType::Error,
                        );
                    }
                }
                set_saving.set(false);
            });
        } else {
            let eid = editing_id.get().unwrap_or_default();
            let body = api::UpdateEventBody {
                name: Some(current_form.name.trim().to_string()),
                slug: Some(current_form.slug.trim().to_string()),
                tagline: Some(current_form.tagline.trim().to_string()),
                link: Some(current_form.link.trim().to_string()),
                status: Some(current_form.status.clone()),
                event_start_ms: Some(start_ms),
                event_end_ms: Some(end_ms),
                sheet_id: Some(current_form.sheet_id.trim().to_string()),
                sheet_name: Some(current_form.sheet_name.trim().to_string()),
                staff_sheet_name: Some(current_form.staff_sheet_name.trim().to_string()),
                quiz_enabled: Some(current_form.quiz_enabled),
                nft_collection_mint: Some(current_form.nft_collection_mint.trim().to_string()),
                nft_metadata_uri: Some(current_form.nft_metadata_uri.trim().to_string()),
                nft_image_url: Some(current_form.nft_image_url.trim().to_string()),
                nft_name_template: Some(current_form.nft_name_template.trim().to_string()),
                nft_symbol: Some(current_form.nft_symbol.trim().to_string()),
                nft_description_template: Some(current_form.nft_description_template.trim().to_string()),
                claim_base_url: Some(current_form.claim_base_url.trim().to_string()),
                organizer_emails: Some(parse_emails(&current_form.organizer_emails)),
                staff_emails: Some(parse_emails(&current_form.staff_emails)),
            };

            leptos::task::spawn_local(async move {
                match api::update_event(&eid, &body).await {
                    Ok(data) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Event '{}' updated", data.name),
                            components::ToastType::Success,
                        );
                        set_current_view.set(EventsView::List);
                        do_reload();
                    }
                    Err(e) => {
                        log::error!("[events-page] update failed: {e}");
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to update event: {e}"),
                            components::ToastType::Error,
                        );
                    }
                }
                set_saving.set(false);
            });
        }
    };

    // Handle cancel
    let handle_cancel = move |_: web_sys::MouseEvent| {
        set_current_view.set(EventsView::List);
    };

    // Main view
    view! {
        <div class="admin-events-page">
            // === List View ===
            <Show when=move || current_view.get() == EventsView::List fallback=|| view! { <div></div> }>
                // Header with create button (hidden for staff users)
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
                    <h2 class="admin-section-heading" style="margin-bottom:0">"Events Management"</h2>
                    <Show when=move || components::can_manage_events(&user_role.get()) fallback=|| view! { <div></div> }>
                        <button class="btn btn-primary btn-sm" on:click=handle_create>
                            "+ Create Event"
                        </button>
                    </Show>
                </div>

                // Loading state
                <Show when=move || loading.get() && events.get().is_empty() fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading events..."
                    </div>
                </Show>

                // Empty state
                <Show
                    when=move || !loading.get() && events.get().is_empty()
                    fallback=|| view! { <div></div> }
                >
                    <div class="card">
                        <div class="admin-empty-state">
                            <div style="font-size:2rem;margin-bottom:0.5rem">"📅"</div>
                            <h3>"No Events Yet"</h3>
                            <p>"Create your first event to get started with check-in management."</p>
                            <Show when=move || components::can_manage_events(&user_role.get()) fallback=|| view! { <div></div> }>
                                <button class="btn btn-primary" style="margin-top:1rem" on:click=handle_create>
                                    "+ Create Event"
                                </button>
                            </Show>
                        </div>
                    </div>
                </Show>

                // Events list
                <Show when=move || !events.get().is_empty() fallback=|| view! { <div></div> }>
                    {move || {
                        let events_list = events.get();
                        events_list.iter().map(|event| {
                            let edit_id = event.id.clone();
                            let archive_id = event.id.clone();
                            let badge_class = status_badge_class(&event.status);
                            let badge_style = status_badge_style(&event.status);
                            let status_text = status_label(&event.status);
                            let start = format_date_display(event.event_start_ms);
                            let end = format_date_display(event.event_end_ms);
                            let sheet_preview: String = event.sheet_id.chars().take(16).collect();
                            let is_archived = event.status == api::EventStatus::Archived;
                            let organizers_count = event.organizer_emails.len();
                            let ename = event.name.clone();
                            let can_manage = components::can_manage_events(&user_role.get());

                            view! {
                                <div class="card">
                                    <div class="card-header">
                                        <div style="display:flex;align-items:center;gap:0.5rem;flex-wrap:wrap">
                                            <span class="card-title">{ename}</span>
                                            <span class=badge_class style=badge_style>{status_text}</span>
                                        </div>
                                        {if can_manage { view! {
                                        <div style="display:flex;gap:0.5rem">
                                            <button
                                                class="btn btn-outline btn-sm"
                                                on:click=move |_| {
                                                    let edit_id = edit_id.clone();
                                                    let set_form = set_form;
                                                    let set_editing_id = set_editing_id;
                                                    let set_current_view = set_current_view;
                                                    let set_toast = set_toast;
                                                    leptos::task::spawn_local(async move {
                                                        match api::get_event_detail(&edit_id).await {
                                                            Ok(data) => {
                                                                set_form.set(form_from_detail(&data.event));
                                                                set_editing_id.set(Some(edit_id));
                                                                set_current_view.set(EventsView::Edit);
                                                            }
                                                            Err(e) => {
                                                                log::error!("[events-page] load detail failed: {e}");
                                                                components::show_toast(
                                                                    &set_toast,
                                                                    &format!("Failed to load event: {e}"),
                                                                    components::ToastType::Error,
                                                                );
                                                            }
                                                        }
                                                    });
                                                }
                                            >
                                                "✏️ Edit"
                                            </button>
                                            {if !is_archived {
                                                let aid = archive_id.clone();
                                                view! {
                                                    <button
                                                        class="btn btn-outline btn-sm"
                                                        on:click=move |_| {
                                                            let aid = aid.clone();
                                                            let set_toast = set_toast;
                                                            let reload = do_reload;
                                                            leptos::task::spawn_local(async move {
                                                                match api::archive_event(&aid).await {
                                                                    Ok(data) => {
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Event '{}' archived", data.name),
                                                                            components::ToastType::Success,
                                                                        );
                                                                        reload();
                                                                    }
                                                                    Err(e) => {
                                                                        log::error!("[events-page] archive failed: {e}");
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Failed to archive: {e}"),
                                                                            components::ToastType::Error,
                                                                        );
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "📦 Archive"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                        </div>
                                        }.into_any() } else { view! { <div></div> }.into_any() }}
                                    </div>
                                    <div class="quiz-settings-grid">
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"Start"</span>
                                            <span style="font-size:0.85rem">{start}</span>
                                        </div>
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"End"</span>
                                            <span style="font-size:0.85rem">{end}</span>
                                        </div>
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"Sheet ID"</span>
                                            <span style="font-size:0.85rem;font-family:monospace">{sheet_preview}"…"</span>
                                        </div>
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"Organizers"</span>
                                            <span style="font-size:0.85rem">
                                                {if organizers_count == 0 { "—".to_string() } else { format!("{organizers_count}") }}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </Show>
            </Show>

            // === Create / Edit Form View ===
            <Show when=move || current_view.get() != EventsView::List fallback=|| view! { <div></div> }>
                {move || {
                    let is_edit = current_view.get() == EventsView::Edit;
                    let title = if is_edit { "Edit Event" } else { "Create Event" };
                    let save_label = if is_edit { "💾 Update Event" } else { "💾 Create Event" };
                    let is_saving = saving.get();
                    let archive_eid = editing_id.get().unwrap_or_default();

                    view! {
                        <div class="card">
                            <h2 class="admin-section-heading">{title}</h2>

                            // ── Basic Info ──
                            <div style="margin-bottom:1.5rem">
                                <h3 style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;color:var(--text-secondary)">
                                    "📋 Basic Info"
                                </h3>
                                <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Name *"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="Event Name"
                                            prop:value=move || form.get().name
                                            on:input=handle_name_input
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Slug *"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="event-slug"
                                            prop:value=move || form.get().slug
                                            on:input=handle_slug_input
                                        />
                                        <span class="quiz-setting-hint">"Auto-generated from name"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Tagline"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="A short description"
                                            prop:value=move || form.get().tagline
                                            on:input=move |ev| set_form.update(|f| f.tagline = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Link"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="https://example.com"
                                            prop:value=move || form.get().link
                                            on:input=move |ev| set_form.update(|f| f.link = event_target_value(&ev))
                                        />
                                    </div>
                                </div>
                            </div>

                            // ── Schedule ──
                            <div style="margin-bottom:1.5rem">
                                <h3 style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;color:var(--text-secondary)">
                                    "🕐 Schedule"
                                </h3>
                                <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Event Start"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="2025-01-15T09:00:00Z"
                                            prop:value=move || form.get().event_start
                                            on:input=move |ev| set_form.update(|f| f.event_start = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"ISO date or epoch ms"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Event End"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="2025-01-15T18:00:00Z"
                                            prop:value=move || form.get().event_end
                                            on:input=move |ev| set_form.update(|f| f.event_end = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"ISO date or epoch ms"</span>
                                    </div>
                                </div>
                            </div>

                            // ── Google Sheets ──
                            <div style="margin-bottom:1.5rem">
                                <h3 style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;color:var(--text-secondary)">
                                    "📊 Google Sheets"
                                </h3>
                                <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Sheet ID *"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="Google Sheet ID"
                                            prop:value=move || form.get().sheet_id
                                            on:input=move |ev| set_form.update(|f| f.sheet_id = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Sheet Name"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="checkin"
                                            prop:value=move || form.get().sheet_name
                                            on:input=move |ev| set_form.update(|f| f.sheet_name = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Staff Sheet Name"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="staff"
                                            prop:value=move || form.get().staff_sheet_name
                                            on:input=move |ev| set_form.update(|f| f.staff_sheet_name = event_target_value(&ev))
                                        />
                                    </div>
                                </div>
                            </div>

                            // ── NFT Configuration ──
                            <div style="margin-bottom:1.5rem">
                                <h3 style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;color:var(--text-secondary)">
                                    "🎨 NFT Configuration"
                                </h3>
                                <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Collection Mint"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="NFT collection mint address"
                                            prop:value=move || form.get().nft_collection_mint
                                            on:input=move |ev| set_form.update(|f| f.nft_collection_mint = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Metadata URI"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="https://..."
                                            prop:value=move || form.get().nft_metadata_uri
                                            on:input=move |ev| set_form.update(|f| f.nft_metadata_uri = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Image URL"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="https://..."
                                            prop:value=move || form.get().nft_image_url
                                            on:input=move |ev| set_form.update(|f| f.nft_image_url = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Name Template"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="{event_name} #1"
                                            prop:value=move || form.get().nft_name_template
                                            on:input=move |ev| set_form.update(|f| f.nft_name_template = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"Use {event_name} placeholder"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Symbol"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="NFT"
                                            prop:value=move || form.get().nft_symbol
                                            on:input=move |ev| set_form.update(|f| f.nft_symbol = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Description Template"</label>
                                        <textarea
                                            class="quiz-textarea quiz-textarea-sm"
                                            placeholder="NFT description..."
                                            prop:value=move || form.get().nft_description_template
                                            on:input=move |ev| set_form.update(|f| f.nft_description_template = event_target_value(&ev))
                                        ></textarea>
                                    </div>
                                </div>
                            </div>

                            // ── Settings ──
                            <div style="margin-bottom:1.5rem">
                                <h3 style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;color:var(--text-secondary)">
                                    "⚙️ Settings"
                                </h3>
                                <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Claim Base URL"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="https://claim.bethere.com"
                                            prop:value=move || form.get().claim_base_url
                                            on:input=move |ev| set_form.update(|f| f.claim_base_url = event_target_value(&ev))
                                        />
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Quiz Enabled"</label>
                                        <div style="display:flex;align-items:center;gap:0.5rem;padding-top:0.3rem">
                                            <input
                                                type="checkbox"
                                                prop:checked=move || form.get().quiz_enabled
                                                on:change=move |ev| {
                                                    let checked = event_target_checked(&ev);
                                                    set_form.update(|f| f.quiz_enabled = checked);
                                                }
                                            />
                                            <span style="font-size:0.85rem;color:var(--text-secondary)">
                                                {move || if form.get().quiz_enabled { "Yes" } else { "No" }}
                                            </span>
                                        </div>
                                    </div>
                                    // Status selector (edit only)
                                    {if is_edit {
                                        view! {
                                            <div class="quiz-setting-item">
                                                <label class="quiz-field-label">"Status"</label>
                                                <select
                                                    class="quiz-number-input"
                                                    on:change=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        let status = match val.as_str() {
                                                            "active" => api::EventStatus::Active,
                                                            "completed" => api::EventStatus::Completed,
                                                            _ => api::EventStatus::Draft,
                                                        };
                                                        set_form.update(|f| f.status = status);
                                                    }
                                                    prop:value=move || {
                                                        match form.get().status {
                                                            api::EventStatus::Active => "active".to_string(),
                                                            api::EventStatus::Completed => "completed".to_string(),
                                                            api::EventStatus::Draft => "draft".to_string(),
                                                            api::EventStatus::Archived => "archived".to_string(),
                                                        }
                                                    }
                                                >
                                                    <option value="draft">"📝 Draft"</option>
                                                    <option value="active">"🟢 Active"</option>
                                                    <option value="completed">"✅ Completed"</option>
                                                </select>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                </div>
                            </div>

                            // ── People ──
                            <div style="margin-bottom:1.5rem">
                                <h3 style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;color:var(--text-secondary)">
                                    "📧 People"
                                </h3>
                                <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Organizer Emails"</label>
                                        <textarea
                                            class="quiz-textarea quiz-textarea-sm"
                                            placeholder="admin@example.com, organizer@example.com"
                                            prop:value=move || form.get().organizer_emails
                                            on:input=move |ev| set_form.update(|f| f.organizer_emails = event_target_value(&ev))
                                        ></textarea>
                                        <span class="quiz-setting-hint">"Comma-separated"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Staff Emails"</label>
                                        <textarea
                                            class="quiz-textarea quiz-textarea-sm"
                                            placeholder="staff1@example.com, staff2@example.com"
                                            prop:value=move || form.get().staff_emails
                                            on:input=move |ev| set_form.update(|f| f.staff_emails = event_target_value(&ev))
                                        ></textarea>
                                        <span class="quiz-setting-hint">"Comma-separated"</span>
                                    </div>
                                </div>
                            </div>

                            // ── Action Buttons ──
                            <div style="display:flex;gap:0.75rem;padding-top:0.5rem;align-items:center">
                                <button
                                    class="btn btn-primary"
                                    on:click=handle_save
                                    disabled=is_saving
                                >
                                    {if is_saving { "Saving..." } else { save_label }}
                                </button>
                                <button class="btn btn-outline" on:click=handle_cancel>
                                    "Cancel"
                                </button>
                                {if is_edit && !archive_eid.is_empty() {
                                    view! {
                                        <button
                                            class="btn btn-outline"
                                            style="margin-left:auto;color:var(--warning)"
                                            on:click=move |_| {
                                                let aid = archive_eid.clone();
                                                let set_toast = set_toast;
                                                let reload = do_reload;
                                                let set_view = set_current_view;
                                                leptos::task::spawn_local(async move {
                                                    match api::archive_event(&aid).await {
                                                        Ok(data) => {
                                                            components::show_toast(
                                                                &set_toast,
                                                                &format!("Event '{}' archived", data.name),
                                                                components::ToastType::Success,
                                                            );
                                                            set_view.set(EventsView::List);
                                                            reload();
                                                        }
                                                        Err(e) => {
                                                            log::error!("[events-page] archive failed: {e}");
                                                            components::show_toast(
                                                                &set_toast,
                                                                &format!("Failed to archive: {e}"),
                                                                components::ToastType::Error,
                                                            );
                                                        }
                                                    }
                                                });
                                            }
                                        >
                                            "📦 Archive Event"
                                        </button>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                            </div>
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}
