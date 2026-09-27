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

use std::collections::HashSet;

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use super::actions::{
    spawn_audience_export, spawn_bulk_check_in, spawn_flush_cache, spawn_manual_refund,
    spawn_qr_generation, spawn_walkin_export, spawn_walkin_sync,
};
use super::attendee_row::{RowCtx, attendee_row};
use super::csv::{download_csv, generate_csv};
use super::render::{
    render_qr_result, render_recent_check_ins, render_stats, render_velocity,
    render_walkin_sync_result,
};
use super::sidebar::{SidebarState, admin_sidebar, install_section_shortcuts};
use super::types::{AdminSection, DashboardTab, FilterPill, Notify};
use crate::api::{self, AttendeeListItem, EventFormat, GenerateQrData, StatsResponse};
use crate::auth;
use crate::components::{self, ToastType};
use crate::pages::admin_attendance_answer::{AnswerFilter, AnswerFilterBar};

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
    let (attendees, set_attendees) = signal(Vec::<AttendeeListItem>::new());
    let (stats, set_stats) = signal(None::<StatsResponse>);
    let (search_query, set_search_query) = signal(String::new());
    let (is_loading, set_is_loading) = signal(true);
    let (qr_generating, set_qr_generating) = signal(false);
    let (qr_result, set_qr_result) = signal(None::<GenerateQrData>);
    let (flushing_cache, set_flushing_cache) = signal(false);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);

    // Walk-in management state
    let (walkin_exporting, set_walkin_exporting) = signal(false);
    let (walkin_syncing, set_walkin_syncing) = signal(false);
    let (walkin_sync_result, set_walkin_sync_result) = signal(None::<api::WalkinSyncResponse>);

    // Cross-event audience aggregation export state
    let (audience_exporting, set_audience_exporting) = signal(false);

    // Active section — Events by default (organizers create event first)
    let (active_section, set_active_section) = signal(AdminSection::Events);

    // Promote-event-→-campaign handoff: EventsPage writes the payload, the
    // watcher Effect below switches section to Campaigns, and CampaignsPage
    // consumes the payload on mount (then clears the signal).
    let (pending_promote_event, set_pending_promote_event) =
        signal(None::<crate::pages::campaigns_page::PromoteEventPayload>);

    // When a promote payload appears, jump to the Campaigns section so
    // CampaignsPage mounts and can consume it.
    Effect::new(move |_| {
        if pending_promote_event.get().is_some() {
            set_active_section.set(AdminSection::Campaigns);
        }
    });

    // Active tab — In-Person by default
    let (active_tab, set_active_tab) = signal(DashboardTab::InPerson);

    // Active filter pill — All by default
    let (filter_pill, set_filter_pill) = signal(FilterPill::All);
    let (answer_filter, set_answer_filter) = signal(AnswerFilter::All);

    // B6: Pagination state — show PAGE_SIZE attendees at a time
    const PAGE_SIZE: usize = 50;
    let (visible_count, set_visible_count) = signal(PAGE_SIZE);

    // Refresh counter — increment to trigger data reload
    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let notify = Notify {
        set_toast,
        set_refresh_counter,
    };

    // Bulk selection state
    let (selected_ids, set_selected_ids) = signal(HashSet::<String>::new());
    let (bulk_checking_in, set_bulk_checking_in) = signal(false);

    // Manual refund state (bulk action for VIPs / attendees without deposit)
    let (show_manual_refund, set_show_manual_refund) = signal(false);
    let (manual_refund_status, set_manual_refund_status) = signal(String::from("refunded"));
    let (manual_refund_link, set_manual_refund_link) = signal(String::new());
    let (manual_refund_sending, set_manual_refund_sending) = signal(false);

    // Delete attendee confirmation state
    let (confirm_delete_id, set_confirm_delete_id) = signal(None::<String>);
    let (deleting_ids, set_deleting_ids) = signal(HashSet::<String>::new());

    // Participation-type override state — tracks which attendees have a
    // pending PATCH /participation-type request so we can disable their
    // toggle button and show a spinner.
    let (switching_ids, set_switching_ids) = signal(HashSet::<String>::new());

    // Event selector state
    let (events_list, set_events_list) = signal(Vec::<api::EventMeta>::new());
    let (active_event_id, set_active_event_id) = signal(None::<String>);
    let (events_loading, set_events_loading) = signal(false);

    // Deep-link trigger for the Record-Slip-on-behalf modal — set by the
    // Attendees list "Record slip" button (writes attendee_id + switches to
    // Deposits section). Consumed by `AdminRecordSlipModal` via `AdminDeposits`
    // — the modal opens itself, pre-fills the field, then clears the signal.
    // Owned here (not inside AdminDeposits) so the Attendees section can
    // write it even while AdminDeposits is unmounted.
    let (pending_record_slip_attendee, set_pending_record_slip_attendee) = signal(None::<String>);

    // Load events list on mount
    Effect::new(move |_| {
        set_events_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::list_events().await {
                Ok(data) => {
                    // Auto-select first active event
                    let active = data
                        .events
                        .iter()
                        .find(|e| e.status == api::EventStatus::Active);
                    if let Some(e) = active {
                        set_active_event_id.set(Some(e.id.clone()));
                    }
                    set_events_list.set(data.events);
                }
                Err(e) => {
                    log::warn!("[admin] failed to load events: {e}");
                }
            }
            set_events_loading.set(false);
        });
    });

    // Helper to get current event_id
    let get_event_id = move || active_event_id.get();

    // Current event's format — drives conditional sidebar + attendee UI
    let current_event_format = Memo::new(move |_| {
        active_event_id
            .get()
            .and_then(|id| {
                events_list
                    .get()
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.event_format.clone())
            })
            .unwrap_or_default()
    });

    // Current event's deposit_enabled flag — gates the per-attendee "Record
    // slip" action button in the Attendees list. False when no event is
    // selected or the event has deposits disabled.
    let current_deposit_enabled = Memo::new(move |_| {
        active_event_id
            .get()
            .and_then(|id| {
                events_list
                    .get()
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.deposit_enabled)
            })
            .unwrap_or(false)
    });

    // Current event's Google Sheet ID — drives the "View Sheet" sidebar link.
    // Empty when no event is selected or the event has no sheet_id.
    let current_sheet_id = Memo::new(move |_| {
        active_event_id
            .get()
            .and_then(|id| {
                events_list
                    .get()
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.sheet_id.clone())
            })
            .unwrap_or_default()
    });

    // Auto-switch tab when event format is single-track
    Effect::new(move |_| {
        let fmt = current_event_format.get();
        match fmt {
            EventFormat::Online => set_active_tab.set(DashboardTab::Online),
            EventFormat::InPerson => set_active_tab.set(DashboardTab::InPerson),
            EventFormat::Hybrid => {} // keep current selection
        }
    });

    // Filtered attendees: tab-filtered + search query + filter pill + sort
    let filtered_attendees = Memo::new(move |_| {
        let query = search_query.get().to_lowercase();
        let tab = active_tab.get();
        let pill = filter_pill.get();
        let answer = answer_filter.get();
        let list = attendees.get();

        let mut filtered: Vec<AttendeeListItem> = list
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
            .filter(|a| answer.matches(a))
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

    // Reset pagination when filters change
    Effect::new(move |_| {
        let _ = active_tab.get();
        let _ = search_query.get();
        let _ = filter_pill.get();
        let _ = answer_filter.get();
        set_visible_count.set(PAGE_SIZE);
    });

    install_section_shortcuts(set_active_section, set_active_tab);

    // Data loading effect — triggered by refresh_counter or active_event_id changes.
    // Skips the initial mount when active_event_id is still None (events not loaded yet),
    // avoiding a duplicate call that would resolve to the same default event.
    Effect::new(move |_| {
        let _ = refresh_counter.get(); // track refresh counter
        let eid = get_event_id();

        // Skip if events haven't loaded yet — the Effect will re-fire when
        // active_event_id transitions from None → Some after events load.
        if eid.is_none() {
            return;
        }

        set_is_loading.set(true);

        leptos::task::spawn_local(async move {
            match api::get_all_attendees(eid.as_deref()).await {
                Ok(data) => {
                    set_attendees.set(data.attendees);
                    set_stats.set(Some(data.stats));
                }
                Err(err) => {
                    log::error!("[admin] failed to load dashboard: {err}");
                    // Clear stale attendees from the previously selected event
                    // so the user doesn't see another event's data.
                    set_attendees.set(Vec::new());
                    set_stats.set(None);
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
        spawn_bulk_check_in(
            ids,
            get_event_id(),
            set_bulk_checking_in,
            set_selected_ids,
            notify,
        );
    };

    // Bulk manual refund handler
    let handle_manual_refund = move |_: web_sys::MouseEvent| {
        if manual_refund_sending.get() {
            return;
        }
        let ids: Vec<String> = selected_ids.get().into_iter().collect();
        if ids.is_empty() {
            return;
        }
        let Some(event_id) = get_event_id() else {
            notify.toast("Select an event first", ToastType::Warning);
            return;
        };
        let link = manual_refund_link.get();
        let body = api::ManualRefundRequest {
            event_id,
            refund_status: manual_refund_status.get(),
            refund_link: (!link.trim().is_empty()).then_some(link),
        };
        spawn_manual_refund(
            ids,
            body,
            set_manual_refund_sending,
            set_show_manual_refund,
            set_selected_ids,
            notify,
        );
    };

    // Handle QR code generation (normal, or force-overwrite existing URLs)
    let generate_qrs = move |force: bool| {
        if qr_generating.get() {
            return;
        }
        spawn_qr_generation(
            force,
            get_event_id(),
            set_qr_generating,
            set_qr_result,
            notify,
        );
    };
    let handle_generate_qrs = move |_: web_sys::MouseEvent| generate_qrs(false);
    let handle_force_generate_qrs = move |_: web_sys::MouseEvent| generate_qrs(true);

    // Handle flush cache button
    let handle_flush_cache = move |_: web_sys::MouseEvent| {
        if flushing_cache.get() {
            return;
        }
        spawn_flush_cache(get_event_id(), set_flushing_cache, notify);
    };

    // Handle walk-in CSV export
    let handle_walkin_export = move |_: web_sys::MouseEvent| {
        if walkin_exporting.get() {
            return;
        }
        let Some(eid) = get_event_id() else {
            notify.toast("Select an event first", ToastType::Warning);
            return;
        };
        spawn_walkin_export(eid, set_walkin_exporting, notify);
    };

    // Handle cross-event audience CSV export (ALL events, no event needed).
    let handle_audience_export = move |_: web_sys::MouseEvent| {
        if audience_exporting.get() {
            return;
        }
        spawn_audience_export(set_audience_exporting, notify);
    };

    // Handle walk-in sync to Google Sheet
    let handle_walkin_sync = move |_: web_sys::MouseEvent| {
        if walkin_syncing.get() {
            return;
        }
        let Some(eid) = get_event_id() else {
            notify.toast("Select an event first", ToastType::Warning);
            return;
        };
        spawn_walkin_sync(eid, set_walkin_syncing, set_walkin_sync_result, notify);
    };

    // Handle sign out
    let handle_sign_out = move |_: web_sys::MouseEvent| {
        auth::logout();
    };

    let row_ctx = RowCtx {
        active_event_id,
        deposit_enabled: current_deposit_enabled,
        set_selected_ids,
        switching_ids,
        set_switching_ids,
        confirm_delete_id,
        set_confirm_delete_id,
        deleting_ids,
        set_deleting_ids,
        set_pending_record_slip: set_pending_record_slip_attendee,
        set_active_section,
        notify,
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
                {admin_sidebar(SidebarState {
                    events_loading,
                    events_list,
                    active_event_id,
                    set_active_event_id,
                    set_refresh_counter,
                    active_section,
                    set_active_section,
                    active_tab,
                    set_active_tab,
                    current_event_format,
                    current_sheet_id,
                    attendees,
                })}
                // Content area
                <main class="admin-content">

                // Attendance section
                <Show when=move || active_section.get() == AdminSection::Attendance fallback=|| view! { <div></div> }>
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
                    <div class="admin-actions-row">
                        <button class="btn btn-outline btn-sm" on:click=handle_refresh>
                            "Refresh"
                        </button>
                        <button
                            class="btn btn-outline btn-sm"
                            on:click=handle_flush_cache
                            disabled=move || flushing_cache.get()
                        >
                            {move || {
                                if flushing_cache.get() {
                                    "Flushing...".to_string()
                                } else {
                                    "Flush Cache".to_string()
                                }
                            }}
                        </button>
                        // QR generation + walk-in actions — only for in-person/hybrid events
                        <Show when=move || current_event_format.get().has_in_person() fallback=|| view! { <span></span> }>
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
                        </Show>
                        <button class="btn btn-outline btn-sm" on:click=handle_export_csv>
                            "Export CSV"
                        </button>
                        // Cross-event audience export — deduped by email across ALL events.
                        // Not gated by event format; works without an event selected.
                        <button
                            class="btn btn-outline btn-sm"
                            on:click=handle_audience_export
                            disabled=move || audience_exporting.get()
                        >
                            {move || {
                                if audience_exporting.get() {
                                    "Exporting...".to_string()
                                } else {
                                    "Export Audience (All Events)".to_string()
                                }
                            }}
                        </button>
                        <button class="btn btn-outline btn-sm" on:click=handle_select_all>
                            "Select All Pending"
                        </button>
                        // Walk-in management — only for in-person/hybrid
                        <Show when=move || current_event_format.get().has_in_person() fallback=|| view! { <span></span> }>
                            <span class="admin-actions-divider"></span>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=handle_walkin_export
                                disabled=move || walkin_exporting.get()
                            >
                                {move || {
                                    if walkin_exporting.get() {
                                        "Exporting...".to_string()
                                    } else {
                                        "Export Walk-in CSV".to_string()
                                    }
                                }}
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=handle_walkin_sync
                                disabled=move || walkin_syncing.get()
                            >
                                {move || {
                                    if walkin_syncing.get() {
                                        "Syncing...".to_string()
                                    } else {
                                        "Sync Walk-ins to Sheet".to_string()
                                    }
                                }}
                            </button>
                        </Show>
                    </div>

                    // QR generation result
                    <Show
                        when=move || qr_result.get().is_some()
                        fallback=|| view! { <div></div> }
                    >
                        {move || render_qr_result(&qr_result.get())}
                        // Force regenerate button (shown after any generation)
                        <div class="admin-force-regen-row">
                            <button class="btn btn-outline btn-sm" on:click=handle_force_generate_qrs>
                                "Force Regenerate All"
                            </button>
                            <span class="admin-force-regen-hint">
                                "Overwrites existing QR URLs"
                            </span>
                        </div>
                    </Show>

                    // Walk-in sync result
                    <Show
                        when=move || walkin_sync_result.get().is_some()
                        fallback=|| view! { <div></div> }
                    >
                        {move || render_walkin_sync_result(walkin_sync_result.get())}
                    </Show>

                    // Stats cards (tab-aware)
                    {move || render_stats(&stats.get(), &attendees.get(), active_tab.get(), &current_event_format.get())}

                    // Check-in velocity neon progress bar
                    {move || attendees.with(|list| render_velocity(list))}

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
                        <button
                            class="filter-pill"
                            class:active=move || filter_pill.get() == FilterPill::Walkin
                            on:click=move |_| set_filter_pill.set(FilterPill::Walkin)
                        >
                            "Walk-in"
                        </button>
                    </div>

                    // "Can you still come?" answers (migration 0052), counted
                    // over the current tab before the answer filter applies.
                    <AnswerFilterBar
                        rows=Signal::derive(move || {
                            let tab = active_tab.get();
                            attendees.with(|list| {
                                list.iter()
                                    .filter(|a| tab.matches(&a.participation_type))
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                        })
                        filter=answer_filter
                        set_filter=set_answer_filter
                    />

                    // Attendee count
                    <div class="admin-count-row">
                        <span class="admin-count-text">
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
                                <button
                                    class="btn btn-outline btn-sm"
                                    on:click=move |_| set_show_manual_refund.set(!show_manual_refund.get())
                                >
                                    "Set Refund Status"
                                </button>
                                <button class="btn btn-outline btn-sm" on:click=handle_clear_selection>
                                    "Clear"
                                </button>
                            </div>
                            // Manual refund inline form
                            <Show when=move || show_manual_refund.get() fallback=|| view! { <div></div> }>
                                <div class="bulk-action-bar admin-refund-form-row">
                                    <label class="admin-refund-form-label">
                                        "Status:"
                                        <select
                                            class="admin-event-select admin-refund-form-select"
                                            on:change=move |ev| set_manual_refund_status.set(event_target_value(&ev))
                                        >
                                            <option value="refunded" selected>"refunded"</option>
                                            <option value="pending">"pending"</option>
                                            <option value="not_applicable">"not_applicable"</option>
                                            <option value="failed">"failed"</option>
                                        </select>
                                    </label>
                                    <label class="admin-refund-form-label">
                                        "Link (opt):"
                                        <input
                                            type="text"
                                            placeholder="https://..."
                                            class="admin-refund-form-input"
                                            on:input=move |ev| set_manual_refund_link.set(event_target_value(&ev))
                                        />
                                    </label>
                                    <button
                                        class="btn btn-primary btn-sm"
                                        disabled=move || manual_refund_sending.get()
                                        on:click=handle_manual_refund
                                    >
                                        {move || if manual_refund_sending.get() { "Applying..." } else { "Apply" }}
                                    </button>
                                </div>
                            </Show>
                        </Show>

                        // Inline attendee items with checkboxes (B6: paginated)
                        {move || {
                            let filtered = filtered_attendees.get();
                            let selected = selected_ids.get();
                            let limit = visible_count.get();
                            if filtered.is_empty() {
                                view! {
                                    <div class="admin-empty-state">
                                        "No attendees found"
                                    </div>
                                }.into_any()
                            } else {
                                let visible: Vec<_> = filtered.iter().take(limit).collect();
                                let has_more = filtered.len() > limit;
                                let remaining = filtered.len().saturating_sub(limit);

                                let items = visible
                                    .into_iter()
                                    .map(|attendee| {
                                        let is_selected = selected.contains(&attendee.api_id);
                                        attendee_row(attendee, is_selected, row_ctx)
                                    })
                                    .collect_view();

                                view! {
                                    {items}
                                    <Show when=move || has_more>
                                        <div class="admin-load-more">
                                            <button
                                                class="btn btn-outline btn-sm"
                                                on:click=move |_| set_visible_count.update(|c| *c += PAGE_SIZE)
                                            >
                                                {format!("Load more ({remaining} remaining)")}
                                            </button>
                                        </div>
                                    </Show>
                                }.into_any()
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
                </Show>

                // Deposits section
                <Show when=move || active_section.get() == AdminSection::Deposits fallback=|| view! { <div></div> }>
                    <crate::pages::admin_deposit::AdminDeposits
                        set_toast=set_toast
                        active_event_id=active_event_id
                        pending_attendee_id=pending_record_slip_attendee
                        set_pending_attendee_id=set_pending_record_slip_attendee
                    />
                </Show>

                // Escrow management section
                <Show when=move || active_section.get() == AdminSection::Escrow fallback=|| view! { <div></div> }>
                    <crate::pages::admin_escrow::AdminEscrow set_toast=set_toast active_event_id=active_event_id />
                </Show>

                // Cancellation section
                <Show when=move || active_section.get() == AdminSection::Cancellation fallback=|| view! { <div></div> }>
                    <crate::pages::admin_cancel::AdminCancel set_toast=set_toast active_event_id=active_event_id />
                </Show>

                // Quiz section
                <Show when=move || active_section.get() == AdminSection::Quiz fallback=|| view! { <div></div> }>
                    <crate::pages::quiz_editor::QuizEditor set_toast=set_toast active_event_id=active_event_id />
                </Show>

                // Form Builder section (Issue #049 Phase 2)
                <Show when=move || active_section.get() == AdminSection::FormBuilder fallback=|| view! { <div></div> }>
                    <crate::pages::form_builder::FormBuilder set_toast=set_toast active_event_id=active_event_id />
                </Show>

                // Adventure section
                <Show when=move || active_section.get() == AdminSection::Adventure fallback=|| view! { <div></div> }>
                    <crate::pages::adventure_config::AdventureConfigEditor set_toast=set_toast active_event_id=active_event_id />
                </Show>

                // Campaigns section (Issue #049 Phase 3)
                <Show when=move || active_section.get() == AdminSection::Campaigns fallback=|| view! { <div></div> }>
                    <crate::pages::campaigns_page::CampaignsPage
                        set_toast=set_toast
                        active_event_id=active_event_id
                        pending_promote_event=pending_promote_event
                        set_pending_promote_event=set_pending_promote_event
                    />
                </Show>

                // Events section
                <Show when=move || active_section.get() == AdminSection::Events fallback=|| view! { <div></div> }>
                    <crate::pages::events_page::EventsPage
                        set_toast=set_toast
                        active_event_id=active_event_id
                        set_pending_promote_event=set_pending_promote_event
                    />
                </Show>

                // Feedback section (Issue #113)
                <Show when=move || active_section.get() == AdminSection::Feedback fallback=|| view! { <div></div> }>
                    <crate::pages::admin_feedback::AdminFeedback
                        set_toast=set_toast
                        active_event_id=active_event_id
                    />
                </Show>
                </main>
            </div>

            <components::Toast toast_signal=toast />
        </div>
    }
}
