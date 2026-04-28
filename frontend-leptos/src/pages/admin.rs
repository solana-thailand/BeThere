//! Admin dashboard page — stats, attendee list, QR generation.
//!
//! Features:
//! - In-Person / Online tab separation
//! - Check-in statistics with progress bar (in-person focused)
//! - Attendee list with search, participation badges, check-in status
//! - QR code generation with force-regenerate option
//! - Recent check-in history
//!
//! Requires being wrapped in `<ProtectedRoute>` to provide
//! `ReadSignal<String>` (user email) via context.

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::{self, AttendeeResponse, GenerateQrData, StatsResponse};
use crate::auth;
use crate::components::{self, ToastType};
use crate::utils;

// ===== Tab Type =====

/// Dashboard tab selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DashboardTab {
    InPerson,
    Online,
}

impl DashboardTab {
    fn label(&self) -> &'static str {
        match self {
            DashboardTab::InPerson => "In-Person",
            DashboardTab::Online => "Online",
        }
    }

    /// Whether an attendee belongs to this tab.
    fn matches(&self, participation_type: &str) -> bool {
        match self {
            DashboardTab::InPerson => utils::is_in_person(participation_type),
            DashboardTab::Online => !utils::is_in_person(participation_type),
        }
    }
}

// ===== Filter Pills =====

/// Attendee list filter pill selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FilterPill {
    All,
    CheckedIn,
    NotCheckedIn,
    Vip,
}

impl FilterPill {
    /// Whether an attendee passes this filter.
    fn matches(&self, a: &AttendeeResponse) -> bool {
        match self {
            FilterPill::All => true,
            FilterPill::CheckedIn => a.checked_in_at.is_some(),
            FilterPill::NotCheckedIn => a.checked_in_at.is_none(),
            FilterPill::Vip => a.ticket_name.to_lowercase().contains("vip"),
        }
    }
}

/// Check if a ticket name indicates VIP status.
fn is_vip_ticket(ticket_name: &str) -> bool {
    ticket_name.to_lowercase().contains("vip")
}

/// Generate CSV content from a filtered attendee list.
fn generate_csv(attendees: &[AttendeeResponse]) -> String {
    let mut csv = String::from(
        "Name,Email,Ticket,Participation,Status,Checked In At,Checked In By,API ID\n",
    );
    for a in attendees {
        let status = if a.checked_in_at.is_some() {
            "Checked In"
        } else {
            "Pending"
        };
        let checked_at = a.checked_in_at.as_deref().unwrap_or("");
        let checked_by = a.checked_in_by.as_deref().unwrap_or("");
        // Escape CSV fields containing commas or quotes
        let escape = |s: &str| -> String {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            escape(&a.name),
            escape(&a.email),
            escape(&a.ticket_name),
            escape(&a.participation_type),
            status,
            escape(checked_at),
            escape(checked_by),
            escape(&a.api_id),
        ));
    }
    csv
}

/// Trigger CSV file download in browser using proper web_sys APIs.
fn download_csv(filename: &str, content: &str) {
    use js_sys::{Array, Uint8Array};

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Encode CSV content as UTF-8 bytes
    let bytes = content.as_bytes();
    let uint8 = Uint8Array::new_with_length(bytes.len() as u32);
    uint8.copy_from(bytes);

    // Create Blob from byte array
    let parts = Array::new();
    parts.push(&uint8.buffer());

    let blob_options = web_sys::BlobPropertyBag::new();
    blob_options.set_type("text/csv;charset=utf-8;");

    let blob = match web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &blob_options) {
        Ok(b) => b,
        Err(_) => return,
    };

    // Create object URL
    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    // Create temporary <a> element, trigger click, cleanup
    if let Ok(a) = document.create_element("a") {
        let _ = a.set_attribute("href", &url);
        let _ = a.set_attribute("download", filename);
        if let Some(body) = document.body() {
            let _ = body.append_child(&a);
            // Cast Element → HtmlElement for .click()
            use wasm_bindgen::JsCast;
            a.unchecked_ref::<web_sys::HtmlElement>().click();
            let _ = body.remove_child(&a);
        }
    }

    web_sys::Url::revoke_object_url(&url).unwrap_or(());
}

// ===== Admin Component =====

/// Admin dashboard page component.
#[component]
pub fn Admin() -> impl IntoView {
    // Get user email and role from ProtectedRoute context
    let user_email = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[admin] no user_email in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });
    let user_role = use_context::<ReadSignal<String>>().unwrap_or_else(|| {
        log::error!(
            "[admin] no user_role in context — route not wrapped in \
                 ProtectedRoute?"
        );
        signal(String::new()).0
    });

    // Redirect non-admin users to /staff
    let navigate = use_navigate();
    Effect::new(move |_| {
        let role = user_role.get();
        if !role.is_empty() && !crate::components::is_admin_role(&role) {
            log::warn!("[admin] non-admin user attempted access, redirecting to /staff");
            navigate("/staff", Default::default());
        }
    });

    // Data state
    let (attendees, set_attendees) = signal(Vec::<AttendeeResponse>::new());
    let (stats, set_stats) = signal(None::<StatsResponse>);
    let (search_query, set_search_query) = signal(String::new());
    let (is_loading, set_is_loading) = signal(true);
    let (qr_generating, set_qr_generating) = signal(false);
    let (qr_result, set_qr_result) = signal(None::<GenerateQrData>);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);

    // Active tab — In-Person by default
    let (active_tab, set_active_tab) = signal(DashboardTab::InPerson);

    // Active filter pill — All by default
    let (filter_pill, set_filter_pill) = signal(FilterPill::All);

    // Refresh counter — increment to trigger data reload
    let (refresh_counter, set_refresh_counter) = signal(0u32);

    // Bulk selection state
    let (selected_ids, set_selected_ids) = signal(HashSet::<String>::new());
    let (bulk_checking_in, set_bulk_checking_in) = signal(false);

    // Filtered attendees: tab-filtered + search query + filter pill + sort
    let filtered_attendees = Memo::new(move |_| {
        let query = search_query.get().to_lowercase();
        let tab = active_tab.get();
        let pill = filter_pill.get();
        let list = attendees.get();

        let mut filtered: Vec<AttendeeResponse> = list
            .iter()
            .filter(|a| tab.matches(&a.participation_type))
            .filter(|a| {
                if query.is_empty() {
                    return true;
                }
                let name = a.name.to_lowercase();
                let email = a.email.to_lowercase();
                let api_id = a.api_id.to_lowercase();
                let ticket = a.ticket_name.to_lowercase();
                name.contains(&query)
                    || email.contains(&query)
                    || api_id.contains(&query)
                    || ticket.contains(&query)
            })
            .filter(|a| pill.matches(a))
            .cloned()
            .collect();

        // Sort: not checked in first, then by name
        filtered.sort_by(|a, b| {
            let a_checked = a.checked_in_at.is_some();
            let b_checked = b.checked_in_at.is_some();
            match (a_checked, b_checked) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        filtered
    });

    // Data loading effect — triggered by refresh_counter changes
    Effect::new(move |_| {
        let _ = refresh_counter.get(); // track refresh counter
        set_is_loading.set(true);

        leptos::task::spawn_local(async move {
            match api::get_attendees().await {
                Ok(data) => {
                    set_attendees.set(data.attendees);
                    set_stats.set(Some(data.stats));
                }
                Err(err) => {
                    log::error!("[admin] failed to load dashboard: {err}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to load dashboard: {err}"),
                        ToastType::Error,
                    );
                }
            }
            set_is_loading.set(false);
        });
    });

    // Handle refresh button click
    let handle_refresh = move |_: web_sys::MouseEvent| {
        set_refresh_counter.update(|c| *c += 1);
    };

    // Handle CSV export
    let handle_export_csv = move |_: web_sys::MouseEvent| {
        let filtered = filtered_attendees.get();
        let tab = active_tab.get();
        let tab_label = tab.label().to_lowercase().replace('-', "_");
        let filename = format!("attendees_{tab_label}.csv");
        let csv = generate_csv(&filtered);
        download_csv(&filename, &csv);
        components::show_toast(
            &set_toast,
            &format!("Exported {} attendees", filtered.len()),
            ToastType::Success,
        );
    };

    // Select all visible (filtered) attendees that are NOT checked in
    let handle_select_all = move |_: web_sys::MouseEvent| {
        let filtered = filtered_attendees.get();
        set_selected_ids.update(|ids| {
            ids.clear();
            for a in &filtered {
                if a.checked_in_at.is_none() {
                    ids.insert(a.api_id.clone());
                }
            }
        });
    };

    // Clear selection
    let handle_clear_selection = move |_: web_sys::MouseEvent| {
        set_selected_ids.set(HashSet::new());
    };

    // Bulk check-in all selected attendees
    let handle_bulk_checkin = move |_: web_sys::MouseEvent| {
        if bulk_checking_in.get() {
            return;
        }
        let ids: Vec<String> = selected_ids.get().into_iter().collect();
        if ids.is_empty() {
            return;
        }

        set_bulk_checking_in.set(true);
        let set_toast = set_toast;
        let set_selected = set_selected_ids;
        let set_refresh = set_refresh_counter;
        let set_busy = set_bulk_checking_in;

        leptos::task::spawn_local(async move {
            let mut succeeded = 0u32;
            let mut failed = 0u32;

            for id in ids {
                match api::check_in(&id).await {
                    Ok(_) => succeeded += 1,
                    Err(e) => {
                        failed += 1;
                        log::warn!("[admin] bulk check-in failed for {id}: {e}");
                    }
                }
            }

            let msg = if failed > 0 {
                format!("Checked in {succeeded}, {failed} failed")
            } else {
                format!("Checked in {succeeded} attendees")
            };
            let toast_type = if failed > 0 {
                ToastType::Warning
            } else {
                ToastType::Success
            };
            components::show_toast(&set_toast, &msg, toast_type);
            set_selected.set(HashSet::new());
            set_refresh.update(|c| *c += 1);
            set_busy.set(false);
        });
    };

    // Handle QR code generation (normal)
    let handle_generate_qrs = move |_: web_sys::MouseEvent| {
        if qr_generating.get() {
            return;
        }
        spawn_qr_generation(
            false,
            set_qr_generating,
            set_qr_result,
            set_toast,
            set_refresh_counter,
        );
    };

    // Handle QR code generation (force)
    let handle_force_generate_qrs = move |_: web_sys::MouseEvent| {
        if qr_generating.get() {
            return;
        }
        spawn_qr_generation(
            true,
            set_qr_generating,
            set_qr_result,
            set_toast,
            set_refresh_counter,
        );
    };

    // Handle sign out
    let handle_sign_out = move |_: web_sys::MouseEvent| {
        auth::logout();
    };

    // Compute show_loading (once, used in view)
    let show_loading = move || is_loading.get() && attendees.get().is_empty();
    let show_content = move || !is_loading.get() || !attendees.get().is_empty();

    view! {
        <div>
            <components::AppHeader
                title="Admin Dashboard"
                user_email=user_email
                user_role=user_role
                on_sign_out=handle_sign_out
            />

            <div class="admin-layout">
                // Sidebar
                <aside class="admin-sidebar">
                    <div class="admin-sidebar-section">
                        <div class="admin-sidebar-heading">"Dashboard"</div>
                    </div>
                    <div class="admin-sidebar-section">
                        <div class="admin-sidebar-heading">"Attendance"</div>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_tab.get() == DashboardTab::InPerson
                            on:click=move |_| set_active_tab.set(DashboardTab::InPerson)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
                                    <circle cx="9" cy="7" r="4"></circle>
                                    <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
                                    <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
                                </svg>
                            </span>
                            "In-Person"
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_tab.get() == DashboardTab::Online
                            on:click=move |_| set_active_tab.set(DashboardTab::Online)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <circle cx="12" cy="12" r="10"></circle>
                                    <line x1="2" y1="12" x2="22" y2="12"></line>
                                    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
                                </svg>
                            </span>
                            "Online"
                        </button>
                    </div>
                    // Quick stats at bottom of sidebar
                    <div class="admin-sidebar-stats">
                        {move || {
                            let attendees_list = attendees.get();
                            let tab_attendees: Vec<_> = attendees_list.iter()
                                .filter(|a| active_tab.get().matches(&a.participation_type))
                                .collect();
                            let total = tab_attendees.len();
                            let checked_in = tab_attendees.iter().filter(|a| a.checked_in_at.is_some()).count();
                            let remaining = total.saturating_sub(checked_in);
                            view! {
                                <div class="admin-sidebar-stat">
                                    <span class="admin-sidebar-stat-value">{total}</span>
                                    <span class="admin-sidebar-stat-label">"Total"</span>
                                </div>
                                <div class="admin-sidebar-stat">
                                    <span class="admin-sidebar-stat-value" style="color:var(--success);">{checked_in}</span>
                                    <span class="admin-sidebar-stat-label">"Checked In"</span>
                                </div>
                                <div class="admin-sidebar-stat">
                                    <span class="admin-sidebar-stat-value" style="color:var(--warning);">{remaining}</span>
                                    <span class="admin-sidebar-stat-label">"Remaining"</span>
                                </div>
                            }.into_any()
                        }}
                    </div>
                </aside>

                // Content area
                <main class="admin-content">
                // Loading state
                <Show when=show_loading fallback=|| view! { <div></div> }>
                    <div class="page-loading">
                        <span class="spinner spinner-lg"></span>
                        "Loading dashboard..."
                    </div>
                </Show>

                // Dashboard content
                <Show when=show_content fallback=|| view! { <div></div> }>
                    // Action buttons row
                    <div style="display:flex;gap:0.5rem;margin-bottom:1rem;flex-wrap:wrap;">
                        <button class="btn btn-outline btn-sm" on:click=handle_refresh>
                            "Refresh"
                        </button>
                        <button
                            class="btn btn-primary btn-sm"
                            on:click=handle_generate_qrs
                            disabled=move || qr_generating.get()
                        >
                            {move || {
                                    if qr_generating.get() {
                                        "Generating...".to_string()
                                    } else {
                                        "Generate QR Codes".to_string()
                                    }
                                }}
                        </button>
                        <button class="btn btn-outline btn-sm" on:click=handle_export_csv>
                            "Export CSV"
                        </button>
                        <button class="btn btn-outline btn-sm" on:click=handle_select_all>
                            "Select All Pending"
                        </button>
                    </div>

                    // QR generation result
                    <Show
                        when=move || qr_result.get().is_some()
                        fallback=|| view! { <div></div> }
                    >
                        {move || render_qr_result(&qr_result.get())}
                        // Force regenerate button (shown after any generation)
                        <div style="margin-top:0.5rem;display:flex;align-items:center;gap:0.5rem;">
                            <button class="btn btn-outline btn-sm" on:click=handle_force_generate_qrs>
                                "Force Regenerate All"
                            </button>
                            <span style="font-size:0.75rem;color:var(--text-muted);">
                                "Overwrites existing QR URLs"
                            </span>
                        </div>
                    </Show>

                    // Stats cards (tab-aware)
                    {move || render_stats(&stats.get(), &attendees.get(), active_tab.get())}

                    // Search box
                    <div class="search-box">
                        <span class="search-icon"></span>
                        <input
                            type="text"
                            placeholder="Search by name, email, ID, or ticket..."
                            prop:value=move || search_query.get()
                            on:input=move |ev| {
                                let val = event_target_value(&ev);
                                set_search_query.set(val);
                            }
                        />
                    </div>

                    // Filter pills
                    <div class="filter-pills">
                        <button
                            class="filter-pill"
                            class:active=move || filter_pill.get() == FilterPill::All
                            on:click=move |_| set_filter_pill.set(FilterPill::All)
                        >
                            "All"
                        </button>
                        <button
                            class="filter-pill"
                            class:active=move || filter_pill.get() == FilterPill::CheckedIn
                            on:click=move |_| set_filter_pill.set(FilterPill::CheckedIn)
                        >
                            "Checked In"
                        </button>
                        <button
                            class="filter-pill"
                            class:active=move || filter_pill.get() == FilterPill::NotCheckedIn
                            on:click=move |_| set_filter_pill.set(FilterPill::NotCheckedIn)
                        >
                            "Not Checked In"
                        </button>
                        <button
                            class="filter-pill"
                            class:active=move || filter_pill.get() == FilterPill::Vip
                            on:click=move |_| set_filter_pill.set(FilterPill::Vip)
                        >
                            "VIP"
                        </button>
                    </div>

                    // Attendee count
                    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.75rem;">
                        <span style="font-size:0.85rem;color:var(--text-secondary);">
                            {move || {
                                let count = filtered_attendees.get().len();
                                let tab = active_tab.get();
                                format!("{count} {} attendee{}", tab.label().to_lowercase(), if count != 1 { "s" } else { "" })
                            }}
                        </span>
                    </div>

                    // Attendee list with selection
                    <div class="attendee-list">
                        // Bulk action bar (shown when items selected)
                        <Show
                            when=move || !selected_ids.get().is_empty()
                            fallback=|| view! { <div></div> }
                        >
                            <div class="bulk-action-bar">
                                <span>{move || format!("{} selected", selected_ids.get().len())}</span>
                                <button
                                    class="btn btn-success btn-sm"
                                    disabled=move || bulk_checking_in.get()
                                    on:click=handle_bulk_checkin
                                >
                                    {move || if bulk_checking_in.get() { "Checking in..." } else { "Check In Selected" }}
                                </button>
                                <button class="btn btn-outline btn-sm" on:click=handle_clear_selection>
                                    "Clear"
                                </button>
                            </div>
                        </Show>

                        // Inline attendee items with checkboxes
                        {move || {
                            let filtered = filtered_attendees.get();
                            let selected = selected_ids.get();
                            if filtered.is_empty() {
                                view! {
                                    <div style="text-align:center;padding:2rem;color:var(--text-muted);">
                                        "No attendees found"
                                    </div>
                                }.into_any()
                            } else {
                                filtered.iter().map(|attendee| {
                                    let is_checked_in = attendee.checked_in_at.is_some();
                                    let is_vip = is_vip_ticket(&attendee.ticket_name);
                                    let is_selected = selected.contains(&attendee.api_id);
                                    let api_id = attendee.api_id.clone();
                                    let badge_class = if is_checked_in { "badge badge-success" } else { "badge badge-warning" };
                                    let badge_text = if is_checked_in { "Checked In" } else { "Pending" };
                                    let participation = utils::get_participation_badge(&attendee.participation_type);
                                    let p_class = participation.css_class.to_string();
                                    let p_label = participation.label;
                                    let name = attendee.name.clone();
                                    let email = attendee.email.clone();
                                    let ticket = attendee.ticket_name.clone();
                                    let has_ticket = !ticket.is_empty();
                                    let time_ago_str = attendee.checked_in_at.as_deref().map(utils::time_ago).unwrap_or_default();
                                    let has_time_ago = is_checked_in && !time_ago_str.is_empty();
                                    let checked_in_by_suffix = attendee.checked_in_by.as_ref().map_or(String::new(), |by| {
                                        if by.is_empty() { String::new() } else { format!(" by {}", utils::escape_html(by)) }
                                    });

                                    view! {
                                        <div class="attendee-item" class:vip=is_vip class:selected=is_selected>
                                            <button
                                                class=format!("attendee-checkbox{}", if is_selected { " checked" } else { "" })
                                                on:click=move |_| set_selected_ids.update(|ids| {
                                                    if ids.contains(&api_id) { ids.remove(&api_id); } else { ids.insert(api_id.clone()); }
                                                })
                                                disabled=is_checked_in
                                            >
                                                {if is_selected { "✓" } else { "" }}
                                            </button>
                                            <div class="attendee-info">
                                                <div class="attendee-name">{utils::escape_html(&name)}</div>
                                                <div class="attendee-email">{utils::escape_html(&email)}</div>
                                                <Show
                                                    when=move || has_ticket
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    <div style="font-size:0.75rem;color:var(--text-muted);margin-top:2px;">
                                                        {utils::escape_html(&ticket)}
                                                        <Show when=move || is_vip fallback=|| view! { <span></span> }>
                                                            <span class="vip-badge">"VIP"</span>
                                                        </Show>
                                                    </div>
                                                </Show>
                                            </div>
                                            <div class="attendee-status">
                                                <span class=p_class.clone()>{p_label.clone()}</span>
                                                <span class=badge_class>{badge_text}</span>
                                                <Show
                                                    when=move || has_time_ago
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    <div style="font-size:0.7rem;color:var(--text-muted);margin-top:4px;text-align:right;">
                                                        {time_ago_str.clone()}{checked_in_by_suffix.clone()}
                                                    </div>
                                                </Show>
                                            </div>
                                        </div>
                                    }
                                }).collect_view().into_any()
                            }
                        }}
                    </div>

                    // Recent check-ins (tab-aware)
                    {move || render_recent_check_ins(&stats.get(), &attendees.get(), active_tab.get())}

                    // Footer
                    <div class="claim-footer">
                        <div class="brand-line">
                            <span class="accent">"BeThere"</span>
                            " x Solana Thailand"
                        </div>
                    </div>
                </Show>
                </main>
            </div>

            <components::Toast toast_signal=toast />
        </div>
    }
}

// ===== QR Generation =====

/// Spawn QR code generation task.
fn spawn_qr_generation(
    force: bool,
    set_qr_generating: WriteSignal<bool>,
    set_qr_result: WriteSignal<Option<GenerateQrData>>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) {
    set_qr_generating.set(true);
    leptos::task::spawn_local(async move {
        match api::generate_qrs(force).await {
            Ok(data) => {
                let count = data.generated;
                let skipped = data.skipped;
                let msg = if skipped > 0 {
                    format!("Generated {count} QR codes ({skipped} skipped)")
                } else {
                    format!("Generated {count} QR codes")
                };
                components::show_toast(&set_toast, &msg, ToastType::Success);
                set_qr_result.set(Some(data));
                // Refresh attendee list after generation
                set_refresh_counter.update(|c| *c += 1);
            }
            Err(err) => {
                log::error!("[admin] QR generation failed: {err}");
                components::show_toast(
                    &set_toast,
                    &format!("QR generation failed: {err}"),
                    ToastType::Error,
                );
            }
        }
        set_qr_generating.set(false);
    });
}

// ===== Render Functions =====

/// Render tab-aware stats cards and progress bar.
fn render_stats(
    stats: &Option<StatsResponse>,
    attendees: &[AttendeeResponse],
    tab: DashboardTab,
) -> AnyView {
    match stats {
        Some(_s) => {
            // Compute counts for this tab
            let tab_attendees: Vec<_> = attendees
                .iter()
                .filter(|a| tab.matches(&a.participation_type))
                .collect();

            let tab_total = tab_attendees.len();
            let tab_checked_in = tab_attendees
                .iter()
                .filter(|a| a.checked_in_at.is_some())
                .count();
            let tab_remaining = tab_total.saturating_sub(tab_checked_in);
            let tab_percentage = if tab_total > 0 {
                (tab_checked_in as f64 / tab_total as f64) * 100.0
            } else {
                0.0
            };
            let remaining_percentage = if tab_total > 0 {
                (tab_remaining as f64 / tab_total as f64) * 100.0
            } else {
                0.0
            };

            // Also show the other tab count as a summary line
            let other_tab = match tab {
                DashboardTab::InPerson => DashboardTab::Online,
                DashboardTab::Online => DashboardTab::InPerson,
            };
            let other_count = attendees
                .iter()
                .filter(|a| other_tab.matches(&a.participation_type))
                .count();

            view! {
                <div class="stats-grid">
                    <div class="stat-card info">
                        <div class="stat-value">{tab_total}</div>
                        <div class="stat-label">{format!("{} Total", tab.label())}</div>
                    </div>
                    <div class="stat-card success">
                        <div class="stat-value">{tab_checked_in}</div>
                        <div class="stat-label">"Checked In"</div>
                        <div class="stat-progress">
                            <div class="stat-progress-fill" style=format!("width: {tab_percentage:.1}%")></div>
                        </div>
                    </div>
                    <div class="stat-card warning">
                        <div class="stat-value">{tab_remaining}</div>
                        <div class="stat-label">"Remaining"</div>
                        <div class="stat-progress">
                            <div class="stat-progress-fill" style=format!("width: {remaining_percentage:.1}%")></div>
                        </div>
                    </div>
                </div>

                // Cross-tab summary
                <div style="text-align:center;margin:0.5rem 0;font-size:0.8rem;color:var(--text-muted);">
                    {format!("{} {} attendee{}", other_count, other_tab.label(), if other_count != 1 { "s" } else { "" })}
                    " — "
                    <span
                        style="cursor:pointer;text-decoration:underline;"
                        on:click=move |_| {
                            // This won't work in a render function since we can't access set_active_tab
                            // The tab summary is informational; switching is done via the tab bar
                        }
                    >
                        "switch tab to view"
                    </span>
                </div>

                // Progress bar
                <div class="card mb-2">
                    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.5rem;">
                        <span style="font-size:0.85rem;font-weight:600;color:var(--text-primary);">
                            {format!("{} Check-In Progress", tab.label())}
                        </span>
                        <span style="font-size:0.85rem;color:var(--text-secondary);">
                            {format!("{tab_percentage:.1}% ({tab_checked_in} / {tab_total})")}
                        </span>
                    </div>
                    <div class="progress-bar">
                        <div
                            class="progress-fill"
                            style=move || format!("width: {tab_percentage}%")
                        ></div>
                    </div>
                </div>
            }
                .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render QR generation result summary.
fn render_qr_result(data: &Option<GenerateQrData>) -> AnyView {
    match data {
        Some(d) => {
            let generated = d.generated;
            let skipped = d.skipped;
            let has_skipped = skipped > 0;
            view! {
                <div class="card mb-2" style="border-color:rgba(34,197,94,0.4);background:rgba(34,197,94,0.05);">
                    <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.5rem;">

                        <span style="font-weight:600;color:var(--text-primary);">
                            "QR Codes Generated"
                        </span>
                    </div>
                    <div style="display:flex;gap:1rem;">
                        <div>
                            <span style="font-weight:600;color:#22c55e;">{generated}</span>
                            <span style="color:var(--text-secondary);">" created"</span>
                        </div>
                        <Show when=move || has_skipped fallback=|| view! { <div></div> }>
                            <div>
                                <span style="font-weight:600;color:#f59e0b;">{skipped}</span>
                                <span style="color:var(--text-secondary);">" skipped (already exist)"</span>
                            </div>
                        </Show>
                    </div>
                </div>
            }
                .into_any()
        }
        None => view! { <div></div> }.into_any(),
    }
}

/// Render the recent check-ins panel, filtered by tab.
fn render_recent_check_ins(
    stats: &Option<StatsResponse>,
    attendees: &[AttendeeResponse],
    tab: DashboardTab,
) -> AnyView {
    match stats {
        Some(s) if !s.recent_check_ins.is_empty() => {
            // Build a lookup map for participation type by api_id
            let participation_map: HashMap<String, String> = attendees
                .iter()
                .map(|a| (a.api_id.clone(), a.participation_type.clone()))
                .collect();

            let recent: Vec<_> = {
                let mut r = s.recent_check_ins.clone();
                r.sort_by(|a, b| {
                    let a_time = js_sys::Date::parse(&a.checked_in_at);
                    let b_time = js_sys::Date::parse(&b.checked_in_at);
                    b_time
                        .partial_cmp(&a_time)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                // Filter by active tab
                r.into_iter()
                    .filter(|ci| {
                        let p_type = participation_map
                            .get(&ci.api_id)
                            .cloned()
                            .unwrap_or_default();
                        tab.matches(&p_type)
                    })
                    .take(10)
                    .collect()
            };

            if recent.is_empty() {
                return view! {
                    <div class="card mt-3">
                        <h3 style="margin-bottom:0.75rem;">"Recent Check-Ins"</h3>
                        <div style="text-align:center;padding:1.5rem;color:var(--text-muted);">
                            {format!("No recent {} check-ins", tab.label().to_lowercase())}
                        </div>
                    </div>
                }
                    .into_any();
            }

            view! {
                <div class="card mt-3">
                    <h3 style="margin-bottom:0.75rem;">
                        {format!("Recent {} Check-Ins", tab.label())}
                    </h3>
                    <div class="attendee-list">
                        {recent.iter().map(|check_in| {
                            let name = check_in.name.clone();
                            let api_id = check_in.api_id.clone();
                            let at = check_in.checked_in_at.clone();
                            let formatted = utils::format_timestamp(&at);
                            let by_suffix = check_in.checked_in_by.as_ref().map_or(String::new(), |by| {
                                if by.is_empty() { String::new() } else { format!(" by {}", utils::escape_html(by)) }
                            });

                            let p_type = participation_map
                                .get(&api_id)
                                .cloned()
                                .unwrap_or_default();
                            let participation = utils::get_participation_badge(&p_type);
                            let p_class = participation.css_class.to_string();
                            let p_label = participation.label;

                            view! {
                                <div class="attendee-item">
                                    <div class="attendee-info">
                                        <div class="attendee-name">{utils::escape_html(&name)}</div>
                                        <div class="attendee-email" style="font-size:0.8rem;">
                                            {utils::escape_html(&api_id)}
                                        </div>
                                    </div>
                                    <div class="attendee-status text-right">
                                        <span class=p_class.clone() style="font-size:0.7rem;margin-bottom:4px;display:inline-block;">
                                            {p_label.clone()}
                                        </span>
                                        <div style="font-size:0.8rem;color:var(--text-secondary);">
                                            {formatted}{by_suffix}
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </div>
            }
                .into_any()
        }
        _ => view! { <div></div> }.into_any(),
    }
}
