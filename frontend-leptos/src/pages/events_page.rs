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
pub struct EventForm {
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
    merkle_tree: String,
    claim_base_url: String,
    organizer_emails: String,
    staff_emails: String,
    status: api::EventStatus,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: String,
    pub deposit_amount_thb: String,
    promptpay_id: String,
    pub escrow_address: String,
    pub organizer_wallet: String,
    pub on_chain_event_id: String,
    pub refund_deadline_hours: String,
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
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let seconds = date.get_seconds();
    let tz_offset_min = date.get_timezone_offset(); // minutes behind UTC (positive = west)
    let tz_sign = if tz_offset_min >= 0.0 { '-' } else { '+' };
    let tz_abs = tz_offset_min.abs() as i32;
    let tz_h = tz_abs / 60;
    let tz_m = tz_abs % 60;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC{}{:02}:{:02}",
        year, month, day, hours, minutes, seconds, tz_sign, tz_h, tz_m
    )
}

/// Format epoch milliseconds to datetime-local input format (YYYY-MM-DDTHH:MM).
fn format_datetime_local(ms: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        year, month, day, hours, minutes
    )
}

/// Create a default form state with sensible defaults.
fn default_form() -> EventForm {
    EventForm {
        sheet_name: "checkin".to_string(),
        staff_sheet_name: "staff".to_string(),
        deposit_enabled: false,
        deposit_amount_usdc: String::new(),
        deposit_amount_thb: String::new(),
        promptpay_id: String::new(),
        escrow_address: String::new(),
        organizer_wallet: String::new(),
        on_chain_event_id: String::new(),
        refund_deadline_hours: String::new(),
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
            format_datetime_local(detail.event_start_ms)
        } else {
            String::new()
        },
        event_end: if detail.event_end_ms > 0 {
            format_datetime_local(detail.event_end_ms)
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
        merkle_tree: detail.merkle_tree.clone(),
        claim_base_url: detail.claim_base_url.clone(),
        organizer_emails: detail.organizer_emails.join(", "),
        staff_emails: detail.staff_emails.join(", "),
        status: detail.status.clone(),
        deposit_enabled: detail.deposit_enabled,
        deposit_amount_usdc: if detail.deposit_amount_usdc > 0 { format!("{:.6}", detail.deposit_amount_usdc as f64 / 1_000_000.0).trim_end_matches('0').trim_end_matches('.').to_string() } else { String::new() },
        deposit_amount_thb: if detail.deposit_amount_thb > 0 { detail.deposit_amount_thb.to_string() } else { String::new() },
        promptpay_id: detail.promptpay_id.clone(),
        escrow_address: detail.escrow_address.clone(),
        organizer_wallet: detail.organizer_wallet.clone(),
        on_chain_event_id: if detail.on_chain_event_id > 0 { detail.on_chain_event_id.to_string() } else { String::new() },
        refund_deadline_hours: if detail.refund_deadline_hours > 0 { detail.refund_deadline_hours.to_string() } else { String::new() },
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
        api::EventStatus::Completed => "badge badge-completed",
        api::EventStatus::Archived => "badge badge-archived",
    }
}

/// Get status display label.
fn status_label(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "Active",
        api::EventStatus::Draft => "Draft",
        api::EventStatus::Completed => "Completed",
        api::EventStatus::Archived => "Archived",
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

    // Section collapse signals (true = expanded)
    let (sec_basic_open, set_sec_basic_open) = signal(true);
    let (sec_schedule_open, set_sec_schedule_open) = signal(true);
    let (sec_sheets_open, set_sec_sheets_open) = signal(false);
    let (sec_nft_open, set_sec_nft_open) = signal(false);
    let (sec_settings_open, set_sec_settings_open) = signal(false);
    let (sec_deposit_open, set_sec_deposit_open) = signal(true);
    let (sec_people_open, set_sec_people_open) = signal(false);
    let (show_advanced, set_show_advanced) = signal(false);
    let (search_query, set_search_query) = signal(String::new());
    let search_input_ref: NodeRef<leptos::html::Input> = NodeRef::new();

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
                merkle_tree: current_form.merkle_tree.trim().to_string(),
                claim_base_url: current_form.claim_base_url.trim().to_string(),
                organizer_emails: parse_emails(&current_form.organizer_emails),
                staff_emails: parse_emails(&current_form.staff_emails),
                deposit_enabled: current_form.deposit_enabled,
                deposit_amount_usdc: (current_form.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u64,
                deposit_amount_thb: current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0),
                promptpay_id: current_form.promptpay_id.trim().to_string(),
                escrow_address: current_form.escrow_address.trim().to_string(),
                organizer_wallet: current_form.organizer_wallet.trim().to_string(),
                on_chain_event_id: current_form.on_chain_event_id.parse::<u64>().unwrap_or(0),
                refund_deadline_hours: current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0),
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
                merkle_tree: Some(current_form.merkle_tree.trim().to_string()),
                claim_base_url: Some(current_form.claim_base_url.trim().to_string()),
                organizer_emails: Some(parse_emails(&current_form.organizer_emails)),
                staff_emails: Some(parse_emails(&current_form.staff_emails)),
                deposit_enabled: Some(current_form.deposit_enabled),
                deposit_amount_usdc: Some((current_form.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u64),
                deposit_amount_thb: Some(current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0)),
                promptpay_id: Some(current_form.promptpay_id.trim().to_string()),
                escrow_address: Some(current_form.escrow_address.trim().to_string()),
                organizer_wallet: Some(current_form.organizer_wallet.trim().to_string()),
                on_chain_event_id: Some(current_form.on_chain_event_id.parse::<u64>().unwrap_or(0)),
                refund_deadline_hours: Some(current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0)),
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
                            let badge_class = status_badge_class(&event.status);
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
                                        <div class="flex-row-gap" style="flex-wrap:wrap">
                                            <span class="card-title">{ename}</span>
                                            <span class=badge_class>{status_text}</span>
                                        </div>
                                        {if can_manage { view! {
                                        <div class="flex-row-gap" style="gap:0.5rem">
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
                                                view! { <div></div> }.into_any()
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

            // === Create / Edit Form View ===
            <Show when=move || current_view.get() != EventsView::List fallback=|| view! { <div></div> }>
                {move || {
                    let is_edit = current_view.get() == EventsView::Edit;
                    let title = if is_edit { "Edit Event" } else { "Create Event" };
                    let save_label = if is_edit { "Update Event" } else { "Create Event" };
                    let is_saving = saving.get();
                    let archive_eid = editing_id.get().unwrap_or_default();

                    view! {
                        <div class="card">
                            <h2 class="admin-section-heading">{title}</h2>

                            // ── Basic Info ──
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_basic_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-basic"></span>
                                    <span class="form-section-title">"Basic Info"</span>
                                    <span class="form-section-badge form-section-badge-required">"Required"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_basic_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_basic_open.get()>
                                    <div class="quiz-settings-grid">
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"Name"<span class="field-required-badge">"Required"</span></label>
                                            <input
                                                type="text"
                                                class="quiz-number-input"
                                                placeholder="Event Name"
                                                prop:value=move || form.get().name
                                                on:input=handle_name_input
                                            />
                                        </div>
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"Slug"<span class="field-required-badge">"Required"</span></label>
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
                                            <label class="quiz-field-label">"Tagline"<span class="field-optional-badge">"Optional"</span></label>
                                            <input
                                                type="text"
                                                class="quiz-number-input"
                                                placeholder="A short description"
                                                prop:value=move || form.get().tagline
                                                on:input=move |ev| set_form.update(|f| f.tagline = event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"Link"<span class="field-optional-badge">"Optional"</span></label>
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
                            </div>

                            // ── Schedule ──
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_schedule_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-schedule"></span>
                                    <span class="form-section-title">"Schedule"</span>
                                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_schedule_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_schedule_open.get()>
                                    <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Event Start"</label>
                                        <input
                                            type="datetime-local"
                                            class="quiz-number-input"
                                            prop:value=move || format_datetime_local(parse_date_to_ms(&form.get().event_start).unwrap_or(0))
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                let ms = parse_date_to_ms(&val).unwrap_or(0);
                                                set_form.update(|f| f.event_start = if ms > 0 { format_datetime_local(ms) } else { val });
                                            }
                                        />
                                        <span class="quiz-setting-hint">"Times in your local timezone"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Event End"</label>
                                        <input
                                            type="datetime-local"
                                            class="quiz-number-input"
                                            prop:value=move || format_datetime_local(parse_date_to_ms(&form.get().event_end).unwrap_or(0))
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                let ms = parse_date_to_ms(&val).unwrap_or(0);
                                                set_form.update(|f| f.event_end = if ms > 0 { format_datetime_local(ms) } else { val });
                                            }
                                        />
                                        <span class="quiz-setting-hint">"Times in your local timezone"</span>
                                    </div>
                                </div>
                                </div>
                            </div>

                            // ── Google Sheets ──
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_sheets_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-sheets"></span>
                                    <span class="form-section-title">"Google Sheets"</span>
                                    <span class="form-section-badge form-section-badge-required">"Required"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_sheets_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_sheets_open.get()>
                                    <div class="quiz-settings-grid">
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"Sheet ID"<span class="field-required-badge">"Required"</span></label>
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
                            </div>

                            // ── NFT Configuration ──
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_nft_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-nft"></span>
                                    <span class="form-section-title">"NFT Configuration"</span>
                                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_nft_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_nft_open.get()>
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
                                            <label class="quiz-field-label">"Merkle Tree"</label>
                                            <input
                                                type="text"
                                                class="quiz-number-input"
                                                placeholder="Tree address (base58) — leave empty for Helius default"
                                                prop:value=move || form.get().merkle_tree
                                                on:input=move |ev| set_form.update(|f| f.merkle_tree = event_target_value(&ev))
                                            />
                                            <span class="quiz-setting-hint">"Reserved for future use. Helius mints to its own managed tree."</span>
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
                                        <Show
                                            when=move || {
                                                let f = form.get();
                                                let resolved = if f.nft_name_template.is_empty() {
                                                    format!("BeThere - {}", f.name)
                                                } else {
                                                    f.nft_name_template.replace("{event_name}", &f.name)
                                                };
                                                resolved.len() > 32
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "Resolved name exceeds 32-char limit (Bubblegum max). Name will be truncated."
                                            </div>
                                        </Show>
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
                            </div>

                            // ── Settings ──
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_settings_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-settings"></span>
                                    <span class="form-section-title">"Settings"</span>
                                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_settings_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_settings_open.get()>
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
                                        <label class="quiz-toggle-label" style="cursor:pointer;padding-top:0.3rem">
                                            <input
                                                type="checkbox"
                                                class="quiz-toggle-checkbox"
                                                prop:checked=move || form.get().quiz_enabled
                                                on:change=move |ev| {
                                                    let checked = event_target_checked(&ev);
                                                    set_form.update(|f| f.quiz_enabled = checked);
                                                }
                                            />
                                            <span class="quiz-toggle-switch"></span>
                                            <span class="quiz-toggle-text">
                                                {move || if form.get().quiz_enabled { "Yes" } else { "No" }}
                                            </span>
                                        </label>
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
                                                    <option value="draft">"Draft"</option>
                                                    <option value="active">"Active"</option>
                                                    <option value="completed">"Completed"</option>
                                                </select>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}
                                </div>
                                </div>
                            </div>

                            // ── Deposit Configuration ──
                            // Deposit toggle always visible
                            <div class="dep-config-row">
                                <span class="dep-config-label">"Deposit"</span>
                                <label class="quiz-toggle-label" style="cursor:pointer">
                                    <input
                                        type="checkbox"
                                        class="quiz-toggle-checkbox"
                                        prop:checked=move || form.get().deposit_enabled
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_form.update(|f| f.deposit_enabled = checked);
                                        }
                                    />
                                    <span class="quiz-toggle-switch"></span>
                                    <span class="quiz-toggle-text">
                                        {move || if form.get().deposit_enabled { "Enabled" } else { "Disabled" }}
                                    </span>
                                </label>
                            </div>

                            // Deposit config section — only when enabled
                            <Show when=move || form.get().deposit_enabled fallback=|| view! { <div></div> }>
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_deposit_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-deposit"></span>
                                    <span class="form-section-title">"Deposit Details"</span>
                                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_deposit_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_deposit_open.get()>
                                    <div class="quiz-settings-grid">
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"USDC Amount"<span class="field-optional-badge">"Optional"</span></label>
                                        <input
                                            type="number"
                                            class="quiz-number-input"
                                            placeholder="e.g. 10 (whole USDC)"
                                            step="0.01"
                                            min="0"
                                            prop:value=move || form.get().deposit_amount_usdc
                                            on:input=move |ev| set_form.update(|f| f.deposit_amount_usdc = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"Amount in whole USDC (e.g. 10 = 10 USDC)"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"THB Amount"</label>
                                        <input
                                            type="number"
                                            class="quiz-number-input"
                                            placeholder="e.g. 500"
                                            step="1"
                                            min="0"
                                            prop:value=move || form.get().deposit_amount_thb
                                            on:input=move |ev| set_form.update(|f| f.deposit_amount_thb = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"Amount in Thai Baht"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"PromptPay ID"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="e.g. 0812345678 or 1-1001-00000-00-0"
                                            prop:value=move || form.get().promptpay_id
                                            on:input=move |ev| set_form.update(|f| f.promptpay_id = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"Thai phone number or national ID for PromptPay QR generation"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">
                                            "Escrow Address"
                                            <Show when=move || !form.get().escrow_address.is_empty() fallback=|| view! { <span></span> }>
                                                <a
                                                    href=move || crate::utils::solscan_address_url(&form.get().escrow_address, &crate::utils::get_cluster())
                                                    target="_blank"
                                                    rel="noopener"
                                                    class="escrow-solscan-link"
                                                >
                                                    "Solscan"
                                                </a>
                                            </Show>
                                        </label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="Solana escrow PDA address (base58)"
                                            prop:value=move || form.get().escrow_address
                                            readonly=move || !form.get().escrow_address.is_empty()
                                            on:input=move |ev| set_form.update(|f| f.escrow_address = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"On-chain escrow PDA for this event (auto-filled after on-chain init)"</span>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Organizer Wallet"</label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="e.g. 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
                                            prop:value=move || form.get().organizer_wallet
                                            readonly=move || !form.get().organizer_wallet.is_empty()
                                            on:input=move |ev| set_form.update(|f| f.organizer_wallet = event_target_value(&ev))
                                        />
                                        <Show
                                            when=move || !form.get().organizer_wallet.is_empty()
                                            fallback=|| view! {
                                                <span class="quiz-setting-hint">"Organizer's Solana wallet address (base58) — connect wallet below or paste manually"</span>
                                            }
                                        >
                                            <span class="quiz-setting-hint" style="color:var(--success,green)">"Wallet locked -- use escrow panel to change"</span>
                                        </Show>
                                    </div>
                                    // Advanced toggle for on-chain event ID
                                    <div class="advanced-toggle-row" on:click=move |_| set_show_advanced.update(|v| *v = !*v)>
                                        <span class="advanced-toggle-icon" class:advanced-toggle-icon-open=move || show_advanced.get()>"▶"</span>
                                        <span class="advanced-toggle-label">"Advanced: On-Chain Event ID"</span>
                                    </div>
                                    <div class:advanced-fields-hidden=move || !show_advanced.get()>
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"On-Chain Event ID"<span class="field-optional-badge">"Auto"</span></label>
                                            <input
                                                type="number"
                                                class="quiz-number-input"
                                                placeholder="Leave empty for auto-derive from event slug"
                                                min="0"
                                                step="1"
                                                prop:value=move || form.get().on_chain_event_id
                                                readonly=move || !form.get().on_chain_event_id.is_empty()
                                                on:input=move |ev| set_form.update(|f| f.on_chain_event_id = event_target_value(&ev))
                                            />
                                            <span class="quiz-setting-hint">"Numeric ID for PDA seeds (0 = auto-derive via hash)"</span>
                                        </div>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Refund Deadline (hours)"</label>
                                        <input
                                            type="number"
                                            class="quiz-number-input"
                                            placeholder="e.g. 168 (= 7 days)"
                                            min="0"
                                            step="1"
                                            prop:value=move || form.get().refund_deadline_hours
                                            on:input=move |ev| set_form.update(|f| f.refund_deadline_hours = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"Hours after event end for refund deadline (default: 168 = 7 days)"</span>
                                    </div>
                                </div>

                                // ── Escrow Initialization (Single-TX) ──
                                // Uses Show instead of {move ||} so the EscrowInitPanel
                                // is NOT re-created when form fields update (e.g. organizer_wallet).
                                // Show only mounts/unmounts when the `when` result changes.

                                // Outer gate: only show when deposit enabled + editing an event
                                <Show when=move || {
                                    let f = form.get();
                                    f.deposit_enabled && !editing_id.get().unwrap_or_default().is_empty()
                                }>

                                    // Already initialized — show success badge
                                    <Show when=move || !form.get().escrow_address.is_empty()>
                                        {move || view! {
                                            <div style="margin-top:0.75rem;padding:0.5rem 0.75rem;border:1px solid var(--success,green);border-radius:6px;background:rgba(0,128,0,0.05)">
                                                <span style="font-size:0.8rem;color:var(--success,green)">
                                                    "Escrow initialized: "
                                                    <code class="code-xs">{form.get().escrow_address}</code>
                                                </span>
                                            </div>
                                        }}
                                    </Show>

                                    // Not yet initialized — show escrow init panel
                                    <Show when=move || form.get().escrow_address.is_empty()>
                                        <super::escrow_init::EscrowInitPanel
                                            event_id=editing_id.get().unwrap_or_default()
                                            form=form
                                            set_form=set_form
                                            set_toast=set_toast
                                        />
                                    </Show>

                                </Show>
                                </div>
                            </div>
                            </Show>

                            // ── People ──
                            <div class="form-section">
                                <div class="form-section-header" on:click=move |_| set_sec_people_open.update(|v| *v = !*v)>
                                    <span class="form-section-icon form-section-icon-people"></span>
                                    <span class="form-section-title">"People"</span>
                                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_people_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_people_open.get()>
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
                            </div>

                            // ── Action Buttons ──
                            <div class="form-actions-row">
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
                                            class="btn btn-outline btn-archive"
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
                                            "Archive Event"
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
