//! Events management page — list, create, and configure events.
//!
//! Provides the list view, routing between Create/Edit modes, and delegates
//! the form UI to `event_form::EventFormComponent`.

use std::sync::Arc;

use leptos::prelude::*;

use crate::api;
use crate::components;
use crate::utils;

use super::event_form::{
    default_form, form_from_detail, format_date_display, status_badge_class, status_label,
    EventFormComponent,
};

// ===== View State =====

/// Current view state for the events page.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EventsView {
    List,
    Create,
    Edit,
}

// ===== Page Component =====

/// Events management page with list, create, and edit views.
#[component]
pub fn EventsPage(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "active_event_id")] active_event_id: ReadSignal<Option<String>>,
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
    let (search_query, set_search_query) = signal(String::new());
    let search_input_ref: NodeRef<leptos::html::Input> = NodeRef::new();
    let (refresh_counter, set_refresh_counter) = signal(0u32);

    // Ctrl+K keyboard shortcut to focus search
    Effect::new(move |_| {
        let search_ref = search_input_ref;
        let handler = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
            move |ev: web_sys::KeyboardEvent| {
                if (ev.ctrl_key() || ev.meta_key()) && ev.key() == "k" {
                    ev.prevent_default();
                    if let Some(el) = search_ref.get() {
                        el.focus().ok();
                    }
                }
            },
        );
        let window = web_sys::window().expect("no window");
        use wasm_bindgen::JsCast;
        let _ = window.add_event_listener_with_callback(
            "keydown",
            handler.as_ref().unchecked_ref(),
        );
        handler.forget();
    });

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
        set_editing_id.set(None);
        set_current_view.set(EventsView::Create);
    };

    // Form done callback — called after successful save, archive, or cancel
    let on_form_done = Arc::new(move || {
        set_current_view.set(EventsView::List);
        set_editing_id.set(None);
        do_reload();
    });

    // Main view
    view! {
        <div class="admin-events-page">
            // === List View ===
            <Show when=move || current_view.get() == EventsView::List fallback=|| view! { <div></div> }>
                // Header with create button (hidden for staff users)
                <div class="events-header-row">
                    <h2 class="admin-section-heading">"Events Management"</h2>
                    <div class="events-header-actions">
                        <div class="search-wrapper">
                            <input
                                type="text"
                                class="events-search-input"
                                placeholder="Search events..."
                                prop:value=move || search_query.get()
                                on:input=move |ev| set_search_query.set(event_target_value(&ev))
                                node_ref=search_input_ref
                            />
                            <span class="kbd search-kbd-hint">"Ctrl+K"</span>
                        </div>
                        <Show when=move || components::can_manage_events(&user_role.get()) fallback=|| view! { <div></div> }>
                            <button class="btn btn-primary btn-sm" on:click=handle_create>
                                "+ Create Event"
                            </button>
                        </Show>
                    </div>
                </div>

                // Event detail summary card (shows when dropdown selects an event)
                <Show when=move || active_event_id.get().is_some() fallback=|| view! { <div></div> }>
                    {move || {
                        let eid = active_event_id.get();
                        let events_list = events.get();
                        let selected_event = eid.and_then(|id| events_list.iter().find(|e| e.id == id).cloned());
                        match selected_event {
                            None => view! { <div></div> }.into_any(),
                            Some(evt) => {
                                let ename = utils::escape_html(&evt.name);
                                let badge_class = status_badge_class(&evt.status);
                                let status_text = status_label(&evt.status);
                                let start = format_date_display(evt.event_start_ms);
                                let end = format_date_display(evt.event_end_ms);
                                let sheet_preview: String = evt.sheet_id.chars().take(16).collect();
                                let organizers_count = evt.organizer_emails.len();
                                let deposit_text = if evt.deposit_enabled { "Enabled" } else { "Disabled" };
                                let escrow_display = if evt.escrow_address.is_empty() {
                                    "Not set".to_string()
                                } else {
                                    let trunc: String = evt.escrow_address.chars().take(8).collect();
                                    let status_suffix = match evt.escrow_status {
                                        api::EscrowStatus::Initialized => "",
                                        api::EscrowStatus::Deactivated => " (deactive)",
                                        api::EscrowStatus::Closed => " (closed)",
                                        _ => "",
                                    };
                                    format!("{trunc}…{status_suffix}")
                                };
                                let needs_escrow = evt.deposit_enabled && evt.escrow_address.is_empty();
                                let has_escrow = evt.deposit_enabled && !evt.escrow_address.is_empty();
                                let fmt_label = evt.event_format.label();
                                let fmt_badge_class = match evt.event_format {
                                    api::EventFormat::InPerson => "badge badge-info-xs",
                                    api::EventFormat::Online => "badge badge-warning-xs",
                                    api::EventFormat::Hybrid => "badge badge-success-xs",
                                };
                                let (escrow_label, escrow_cls) = if needs_escrow {
                                    ("No Escrow".to_string(), "badge badge-warning-xs".to_string())
                                } else if has_escrow {
                                    match evt.escrow_status {
                                        api::EscrowStatus::Initialized => ("Escrow: Active".to_string(), "badge badge-success-xs".to_string()),
                                        api::EscrowStatus::Deactivated => ("Escrow: Deactivated".to_string(), "badge badge-warning-xs".to_string()),
                                        api::EscrowStatus::Closed => ("Escrow: Closed".to_string(), "badge badge-info-xs".to_string()),
                                        _ => ("Escrow".to_string(), "badge badge-success-xs".to_string()),
                                    }
                                } else {
                                    (String::new(), String::new())
                                };
                                let show_escrow_badge = !escrow_label.is_empty();
                                let edit_id = evt.id.clone();
                                let restore_id = evt.id.clone();
                                let delete_id = evt.id.clone();
                                let can_manage = components::can_manage_events(&user_role.get());
                                let is_draft = evt.status == api::EventStatus::Draft;
                                let is_archived = evt.status == api::EventStatus::Archived;
                                let event_has_escrow_addr = !evt.escrow_address.is_empty();

                                view! {
                                    <div class="event-detail-card">
                                        <div class="event-detail-header">
                                            <div class="flex-row-gap events-flex-wrap-center">
                                                <span class="card-title">{ename.clone()}</span>
                                                <span class=badge_class>{status_text}</span>
                                                <span class=fmt_badge_class>{fmt_label}</span>
                                                {if evt.visibility == api::EventVisibility::Private {
                                                    view! { <span class="badge badge-warning-xs">"🔒 Private"</span> }.into_any()
                                                } else {
                                                    view! { <span></span> }.into_any()
                                                }}
                                                {if show_escrow_badge {
                                                    view! {
                                                        <span class=escrow_cls.clone()>{escrow_label.clone()}</span>
                                                    }.into_any()
                                                } else {
                                                    view! { <span></span> }.into_any()
                                                }}
                                            </div>
                                            {if can_manage {
                                                view! {
                                                    <button class="btn btn-primary btn-sm" on:click=move |_| {
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
                                                    }>
                                                        "Edit Event"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                            {if is_archived {
                                                let rid = restore_id.clone();
                                                view! {
                                                    <button
                                                        class="btn btn-outline btn-sm"
                                                        on:click=move |_| {
                                                            let rid = rid.clone();
                                                            let set_toast = set_toast;
                                                            let reload = do_reload;
                                                            leptos::task::spawn_local(async move {
                                                                match api::restore_event(&rid).await {
                                                                    Ok(data) => {
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Event '{}' restored", data.name),
                                                                            components::ToastType::Success,
                                                                        );
                                                                        reload();
                                                                    }
                                                                    Err(e) => {
                                                                        log::error!("[events-page] restore failed: {e}");
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Failed to restore: {e}"),
                                                                            components::ToastType::Error,
                                                                        );
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "Restore"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                            // Delete button — Draft or Archived, with force for escrow bypass
                                            {if is_draft || is_archived {
                                                let did = delete_id.clone();
                                                let dname = ename.clone();
                                                let force_delete = is_draft || event_has_escrow_addr;
                                                view! {
                                                    <button
                                                        class="btn btn-outline btn-sm btn-danger"
                                                        on:click=move |_| {
                                                            let did = did.clone();
                                                            let set_toast = set_toast;
                                                            let reload = do_reload;
                                                            let escrow_note = if force_delete { "\\n\\nWARNING: Event has an on-chain escrow that will be orphaned." } else { "" };
                                                            let confirm_msg = format!("Permanently delete '{dname}'? This cannot be undone.{escrow_note}");
                                                            if !web_sys::window().unwrap().confirm_with_message(&confirm_msg).unwrap_or(false) {
                                                                return;
                                                            }
                                                            leptos::task::spawn_local(async move {
                                                                match api::hard_delete_event(&did, force_delete).await {
                                                                    Ok(data) => {
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Event '{}' permanently deleted", data.id),
                                                                            components::ToastType::Success,
                                                                        );
                                                                        reload();
                                                                    }
                                                                    Err(e) => {
                                                                        log::error!("[events-page] delete failed: {e}");
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Failed to delete: {e}"),
                                                                            components::ToastType::Error,
                                                                        );
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "Delete"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                            </div>
                                            <div class="event-detail-grid">
                                            <div class="quiz-setting-item">
                                                <span class="quiz-setting-label">"Start"</span>
                                                <span class="setting-value">{start}</span>
                                            </div>
                                            <div class="quiz-setting-item">
                                                <span class="quiz-setting-label">"End"</span>
                                                <span class="setting-value">{end}</span>
                                            </div>
                                            <div class="quiz-setting-item">
                                                <span class="quiz-setting-label">"Sheet ID"</span>
                                                <span class="setting-value-mono">{sheet_preview}"…"</span>
                                            </div>
                                            <div class="quiz-setting-item">
                                                <span class="quiz-setting-label">"Deposit"</span>
                                                <span class="setting-value">{deposit_text}</span>
                                            </div>
                                            <div class="quiz-setting-item">
                                                <span class="quiz-setting-label">"Escrow"</span>
                                                <span class="setting-value-mono">{escrow_display}</span>
                                            </div>
                                            <div class="quiz-setting-item">
                                                <span class="quiz-setting-label">"Organizers"</span>
                                                <span class="setting-value">
                                                    {if organizers_count == 0 { "—".to_string() } else { format!("{organizers_count}") }}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                        }
                    }}
                </Show>

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
                            <div class="events-empty-icon"></div>
                            <h3>"No Events Yet"</h3>
                            <p>"Create your first event to get started with check-in management."</p>
                            <Show when=move || components::can_manage_events(&user_role.get()) fallback=|| view! { <div></div> }>
                                <button class="btn btn-primary u-mt-1rem" on:click=handle_create>
                                    "+ Create Event"
                                </button>
                            </Show>
                        </div>
                    </div>
                </Show>

                // Events list
                <Show when=move || !events.get().is_empty() fallback=|| view! { <div></div> }>
                    {move || {
                        let query = search_query.get().to_lowercase();
                        let events_list = events.get();
                        let filtered: Vec<_> = if query.is_empty() {
                            events_list.iter().collect()
                        } else {
                            events_list.iter().filter(|e| {
                                e.name.to_lowercase().contains(&query)
                                    || e.slug.to_lowercase().contains(&query)
                                    || e.sheet_id.to_lowercase().contains(&query)
                            }).collect()
                        };
                        filtered.iter().map(|event| {
                            let edit_id = event.id.clone();
                            let archive_id = event.id.clone();
                            let event_slug = event.slug.clone();
                            let badge_class = status_badge_class(&event.status);
                            let status_text = status_label(&event.status);
                            let start = format_date_display(event.event_start_ms);
                            let end = format_date_display(event.event_end_ms);
                            let sheet_preview: String = event.sheet_id.chars().take(16).collect();
                            let is_archived = event.status == api::EventStatus::Archived;
                            let is_draft = event.status == api::EventStatus::Draft;
                            let organizers_count = event.organizer_emails.len();
                            let ename = event.name.clone();
                            let can_manage = components::can_manage_events(&user_role.get());
                            let needs_escrow = event.deposit_enabled && event.escrow_address.is_empty();
                            let has_escrow = event.deposit_enabled && !event.escrow_address.is_empty();
                            let event_has_escrow_addr = !event.escrow_address.is_empty();
                            let fmt_label = event.event_format.label();
                            let fmt_badge_class = match event.event_format {
                                api::EventFormat::InPerson => "badge badge-info-xs",
                                api::EventFormat::Online => "badge badge-warning-xs",
                                api::EventFormat::Hybrid => "badge badge-success-xs",
                            };
                            let (escrow_label, escrow_cls) = if needs_escrow {
                                ("No Escrow".to_string(), "badge badge-warning-xs".to_string())
                            } else if has_escrow {
                                match event.escrow_status {
                                    api::EscrowStatus::Initialized => ("Escrow: Active".to_string(), "badge badge-success-xs".to_string()),
                                    api::EscrowStatus::Deactivated => ("Escrow: Deactivated".to_string(), "badge badge-warning-xs".to_string()),
                                    api::EscrowStatus::Closed => ("Escrow: Closed".to_string(), "badge badge-info-xs".to_string()),
                                    _ => ("Escrow".to_string(), "badge badge-success-xs".to_string()),
                                }
                            } else {
                                (String::new(), String::new())
                            };
                            let show_escrow_badge = !escrow_label.is_empty();

                            view! {
                                <div class="card">
                                    <div class="card-header">
                                        <div class="flex-row-gap events-flex-wrap">
                                            <span class="card-title">{ename.clone()}</span>
                                            <span class=badge_class>{status_text}</span>
                                            <span class=fmt_badge_class>{fmt_label}</span>
                                            {if event.visibility == api::EventVisibility::Private {
                                                view! { <span class="badge badge-warning-xs">"🔒 Private"</span> }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                            {if show_escrow_badge {
                                                view! {
                                                    <span class=escrow_cls.clone()>{escrow_label.clone()}</span>
                                                }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                        </div>
                                        {if can_manage { view! {
                                        <div class="flex-row-gap events-flex-gap-sm">
                                            <a
                                                class="btn btn-outline btn-sm"
                                                href=format!("/e/{}", event_slug)
                                                target="_blank"
                                                rel="noopener noreferrer"
                                            >
                                                "Public Link"
                                            </a>
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
                                                "Edit"
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
                                                        "Archive"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                // Archived event — show Restore button
                                                let rid = archive_id.clone();
                                                view! {
                                                    <button
                                                        class="btn btn-outline btn-sm"
                                                        on:click=move |_| {
                                                            let rid = rid.clone();
                                                            let set_toast = set_toast;
                                                            let reload = do_reload;
                                                            leptos::task::spawn_local(async move {
                                                                match api::restore_event(&rid).await {
                                                                    Ok(data) => {
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Event '{}' restored", data.name),
                                                                            components::ToastType::Success,
                                                                        );
                                                                        reload();
                                                                    }
                                                                    Err(e) => {
                                                                        log::error!("[events-page] restore failed: {e}");
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Failed to restore: {e}"),
                                                                            components::ToastType::Error,
                                                                        );
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "Restore"
                                                    </button>
                                                }.into_any()
                                            }}
                                            // Delete button — available for Draft and Archived events
                                            {if is_draft || is_archived {
                                                let did = archive_id.clone();
                                                let dname = ename.clone();
                                                let force_delete = is_draft || event_has_escrow_addr;
                                                view! {
                                                    <button
                                                        class="btn btn-outline btn-sm btn-danger"
                                                        on:click=move |_| {
                                                            let did = did.clone();
                                                            let set_toast = set_toast;
                                                            let reload = do_reload;
                                                            let escrow_note = if force_delete { "\\n\\nWARNING: Event has an on-chain escrow that will be orphaned." } else { "" };
                                                            let confirm_msg = format!("Permanently delete '{dname}'? This cannot be undone.{escrow_note}");
                                                            if !web_sys::window().unwrap().confirm_with_message(&confirm_msg).unwrap_or(false) {
                                                                return;
                                                            }
                                                            leptos::task::spawn_local(async move {
                                                                match api::hard_delete_event(&did, force_delete).await {
                                                                    Ok(data) => {
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Event '{}' permanently deleted", data.id),
                                                                            components::ToastType::Success,
                                                                        );
                                                                        reload();
                                                                    }
                                                                    Err(e) => {
                                                                        log::error!("[events-page] delete failed: {e}");
                                                                        components::show_toast(
                                                                            &set_toast,
                                                                            &format!("Failed to delete: {e}"),
                                                                            components::ToastType::Error,
                                                                        );
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "Delete"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                        </div>
                                        }.into_any() } else { view! { <div></div> }.into_any() }}
                                    </div>
                                    <div class="quiz-settings-grid">
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"Start"</span>
                                            <span class="setting-value">{start}</span>
                                        </div>
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"End"</span>
                                            <span class="setting-value">{end}</span>
                                        </div>
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"Sheet ID"</span>
                                            <span class="setting-value-mono">{sheet_preview}"…"</span>
                                        </div>
                                        <div class="quiz-setting-item">
                                            <span class="quiz-setting-label">"Organizers"</span>
                                            <span class="setting-value">
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

            // Compact event context bar during Edit view
            <Show when=move || current_view.get() == EventsView::Edit && active_event_id.get().is_some() fallback=|| view! { <div></div> }>
                {move || {
                    let eid = active_event_id.get();
                    let events_list = events.get();
                    let selected_event = eid.and_then(|id| events_list.iter().find(|e| e.id == id).cloned());
                    match selected_event {
                        None => view! { <div></div> }.into_any(),
                        Some(evt) => {
                            let ename = utils::escape_html(&evt.name);
                            let badge_class = status_badge_class(&evt.status);
                            let status_text = status_label(&evt.status);
                            let fmt_label = evt.event_format.label();
                            let fmt_badge_class = match evt.event_format {
                                api::EventFormat::InPerson => "badge badge-info-xs",
                                api::EventFormat::Online => "badge badge-warning-xs",
                                api::EventFormat::Hybrid => "badge badge-success-xs",
                            };
                            let needs_escrow = evt.deposit_enabled && evt.escrow_address.is_empty();
                            let has_escrow = evt.deposit_enabled && !evt.escrow_address.is_empty();
                            let (ctx_escrow_label, ctx_escrow_cls) = if needs_escrow {
                                ("No Escrow".to_string(), "badge badge-warning-xs".to_string())
                            } else if has_escrow {
                                match evt.escrow_status {
                                    api::EscrowStatus::Initialized => ("Escrow: Active".to_string(), "badge badge-success-xs".to_string()),
                                    api::EscrowStatus::Deactivated => ("Escrow: Deactivated".to_string(), "badge badge-warning-xs".to_string()),
                                    api::EscrowStatus::Closed => ("Escrow: Closed".to_string(), "badge badge-info-xs".to_string()),
                                    _ => ("Escrow".to_string(), "badge badge-success-xs".to_string()),
                                }
                            } else {
                                (String::new(), String::new())
                            };
                            let ctx_show_escrow = !ctx_escrow_label.is_empty();

                            view! {
                                <div class="event-edit-context-bar">
                                    <button
                                        class="btn btn-outline btn-xs"
                                        on:click=move |_| {
                                            set_current_view.set(EventsView::List);
                                            set_editing_id.set(None);
                                        }
                                    >
                                        "← Back"
                                    </button>
                                    <div class="flex-row-gap events-flex-wrap-center">
                                        <span class="card-title events-ctx-title">{ename}</span>
                                        <span class=badge_class>{status_text}</span>
                                        <span class=fmt_badge_class>{fmt_label}</span>
                                        {if evt.visibility == api::EventVisibility::Private {
                                            view! { <span class="badge badge-warning-xs">"🔒 Private"</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                        {if ctx_show_escrow {
                                            view! { <span class=ctx_escrow_cls>{ctx_escrow_label}</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                    </div>
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </Show>

            // Audit trail (collapsible, loads on demand)
            <Show when=move || current_view.get() == EventsView::Edit && active_event_id.get().is_some() fallback=|| view! { <div></div> }>
                {move || {
                    let eid = active_event_id.get();
                    match eid {
                        None => view! { <div></div> }.into_any(),
                        Some(id) => view! { <crate::pages::audit_panel::AuditPanel event_id=id /> }.into_any(),
                    }
                }}
            </Show>

            // On-chain events panel (collapsible, loads on demand, only if escrow exists)
            <Show when=move || current_view.get() == EventsView::Edit && active_event_id.get().is_some() fallback=|| view! { <div></div> }>
                {move || {
                    let eid = active_event_id.get();
                    let events_list = events.get();
                    let has_escrow = eid.as_ref().and_then(|id| events_list.iter().find(|e| &e.id == id))
                        .map(|e| !e.escrow_address.is_empty())
                        .unwrap_or(false);
                    match (eid, has_escrow) {
                        (Some(id), true) => view! { <crate::pages::onchain_events_panel::OnchainEventsPanel event_id=id /> }.into_any(),
                        _ => view! { <div></div> }.into_any(),
                    }
                }}
            </Show>

            // === Create / Edit Form View ===
            <Show when=move || current_view.get() != EventsView::List fallback=|| view! { <div></div> }>
                <EventFormComponent
                    set_toast=set_toast
                    form=form
                    set_form=set_form
                    editing_id=editing_id
                    is_create=current_view.get() == EventsView::Create
                    events=events
                    on_done=on_form_done.clone()
                />
            </Show>
        </div>
    }
}
