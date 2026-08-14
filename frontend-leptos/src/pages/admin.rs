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

use crate::api::{self, AttendeeListItem, EventFormat, GenerateQrData, StatsResponse};
use crate::auth;
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};
use crate::utils;

// ===== Tab Type =====

/// Admin dashboard section selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AdminSection {
    Attendance,
    Deposits,
    Escrow,
    Cancellation,
    Quiz,
    FormBuilder,
    Adventure,
    Campaigns,
    Events,
}

/// Dashboard tab selection (within Attendance section).
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
    Walkin,
}

impl FilterPill {
    /// Whether an attendee passes this filter.
    fn matches(&self, a: &AttendeeListItem) -> bool {
        match self {
            FilterPill::All => true,
            FilterPill::CheckedIn => a.checked_in_at.is_some(),
            FilterPill::NotCheckedIn => a.checked_in_at.is_none(),
            FilterPill::Vip => a.ticket_name.to_lowercase().contains("vip"),
            FilterPill::Walkin => a.ticket_name.eq_ignore_ascii_case("Walk-in"),
        }
    }
}

/// Check if a ticket name indicates VIP status.
fn is_vip_ticket(ticket_name: &str) -> bool {
    ticket_name.to_lowercase().contains("vip")
}

/// Generate CSV content from a filtered attendee list.
fn generate_csv(attendees: &[AttendeeListItem]) -> String {
    let mut csv = String::from(
        "Name,Email,Ticket,Participation,Status,Checked In At,Checked In By,API ID,Deposit Status,Deposit Amount,Deposit TX,NFT,Refund Status\n",
    );
    for a in attendees {
        let status = if a.checked_in_at.is_some() {
            "Checked In"
        } else {
            "Pending"
        };
        let checked_at = a.checked_in_at.as_deref().unwrap_or("");
        let checked_by = a.checked_in_by.as_deref().unwrap_or("");
        let deposit_status = a.deposit_status.as_deref().unwrap_or("");
        let deposit_amount = a.deposit_amount.as_deref().unwrap_or("");
        let deposit_tx = a.deposit_tx_signature.as_deref().unwrap_or("");
        let nft = if a.nft_proof_url.is_some() { "Yes" } else { "" };
        let refund_status = a.refund_status.as_deref().unwrap_or("");
        // Escape CSV fields containing commas or quotes
        let escape = |s: &str| -> String {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            escape(&a.name),
            escape(&a.email),
            escape(&a.ticket_name),
            escape(&a.participation_type),
            status,
            escape(checked_at),
            escape(checked_by),
            escape(&a.api_id),
            deposit_status,
            deposit_amount,
            escape(deposit_tx),
            nft,
            refund_status,
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

    // B6: Pagination state — show PAGE_SIZE attendees at a time
    const PAGE_SIZE: usize = 50;
    let (visible_count, set_visible_count) = signal(PAGE_SIZE);

    // Refresh counter — increment to trigger data reload
    let (refresh_counter, set_refresh_counter) = signal(0u32);

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
    let event_id_for_delete = active_event_id;
    let (events_loading, set_events_loading) = signal(false);

    // Deep-link trigger for the Record-Slip-on-behalf modal — set by the
    // Attendees list "Record slip" button (writes attendee_id + switches to
    // Deposits section). Consumed by `AdminRecordSlipModal` via `AdminDeposits`
    // — the modal opens itself, pre-fills the field, then clears the signal.
    // Owned here (not inside AdminDeposits) so the Attendees section can
    // write it even while AdminDeposits is unmounted.
    let (pending_record_slip_attendee, set_pending_record_slip_attendee) =
        signal(None::<String>);

    // Load events list on mount
    Effect::new(move |_| {
        set_events_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::list_events().await {
                Ok(data) => {
                    // Auto-select first active event
                    let active = data.events.iter().find(|e| e.status == api::EventStatus::Active);
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
        set_visible_count.set(PAGE_SIZE);
    });

    // Keyboard shortcuts for sidebar navigation (Alt+1…Alt+0)
    Effect::new(move |_| {
        let handler = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
            move |ev: web_sys::KeyboardEvent| {
                if ev.alt_key() {
                    match ev.key().as_str() {
                        "1" => { ev.prevent_default(); set_active_section.set(AdminSection::Events); }
                        "2" => { ev.prevent_default(); set_active_section.set(AdminSection::Campaigns); }
                        "3" => { ev.prevent_default(); set_active_section.set(AdminSection::Quiz); }
                        "4" => { ev.prevent_default(); set_active_section.set(AdminSection::FormBuilder); }
                        "5" => { ev.prevent_default(); set_active_section.set(AdminSection::Adventure); }
                        "6" => { ev.prevent_default(); set_active_section.set(AdminSection::Attendance); set_active_tab.set(DashboardTab::InPerson); }
                        "7" => { ev.prevent_default(); set_active_section.set(AdminSection::Attendance); set_active_tab.set(DashboardTab::Online); }
                        "8" => { ev.prevent_default(); set_active_section.set(AdminSection::Deposits); }
                        "9" => { ev.prevent_default(); set_active_section.set(AdminSection::Escrow); }
                        "0" => { ev.prevent_default(); set_active_section.set(AdminSection::Cancellation); }
                        _ => {}
                    }
                }
            },
        );
        let window = web_sys::window().expect("no window");
        use wasm_bindgen::JsCast;
        let _ = window.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        handler.forget();
    });

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
            match api::get_attendees(eid.as_deref(), None, None).await {
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

        set_bulk_checking_in.set(true);
        let set_toast = set_toast;
        let set_selected = set_selected_ids;
        let set_refresh = set_refresh_counter;
        let set_busy = set_bulk_checking_in;
        let eid = get_event_id();

        leptos::task::spawn_local(async move {
            let mut succeeded = 0u32;
            let mut failed = 0u32;

            for id in ids {
                match api::check_in(&id, eid.as_deref(), false).await {
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
            api::invalidate_attendee_cache();
            set_selected.set(HashSet::new());
            set_refresh.update(|c| *c += 1);
            set_busy.set(false);
        });
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
        let eid = get_event_id();
        let Some(event_id) = eid else {
            components::show_toast(&set_toast, "Select an event first", ToastType::Warning);
            return;
        };
        let status = manual_refund_status.get();
        let link = manual_refund_link.get();
        let link_opt = if link.trim().is_empty() {
            None
        } else {
            Some(link)
        };

        set_manual_refund_sending.set(true);
        let set_toast = set_toast;
        let set_selected = set_selected_ids;
        let set_refresh = set_refresh_counter;
        let set_busy = set_manual_refund_sending;
        let set_show = set_show_manual_refund;

        leptos::task::spawn_local(async move {
            let mut succeeded = 0u32;
            let mut failed = 0u32;

            log::info!(
                "[admin] manual refund: sending {} requests, status={}, link={:?}",
                ids.len(),
                status,
                link_opt
            );

            for id in ids {
                let body = api::ManualRefundRequest {
                    event_id: event_id.clone(),
                    refund_status: status.clone(),
                    refund_link: link_opt.clone(),
                };
                match api::mark_manual_refund(&id, &body).await {
                    Ok(resp) => {
                        succeeded += 1;
                        log::info!("[admin] manual refund OK for {id}: {:?}", resp);
                    }
                    Err(e) => {
                        failed += 1;
                        log::warn!("[admin] manual refund failed for {id}: {e}");
                    }
                }
            }

            let msg = if failed > 0 {
                format!("Set refund status for {succeeded}, {failed} failed")
            } else {
                format!("Set refund status for {succeeded} attendees")
            };
            let toast_type = if failed > 0 {
                ToastType::Warning
            } else {
                ToastType::Success
            };
            components::show_toast(&set_toast, &msg, toast_type);
            api::invalidate_attendee_cache();
            set_selected.set(HashSet::new());
            set_refresh.update(|c| *c += 1);
            set_show.set(false);
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
            get_event_id(),
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
            get_event_id(),
            set_qr_generating,
            set_qr_result,
            set_toast,
            set_refresh_counter,
        );
    };

    // Handle flush cache button
    let handle_flush_cache = move |_: web_sys::MouseEvent| {
        if flushing_cache.get() {
            return;
        }
        let event_id = get_event_id();
        set_flushing_cache.set(true);
        leptos::task::spawn_local(async move {
            match api::flush_cache(event_id.as_deref()).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Cache flushed — attendee list & column mapping refreshed",
                        components::ToastType::Success,
                    );
                    set_refresh_counter.update(|c| *c += 1);
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Flush failed: {}", e.message),
                        components::ToastType::Error,
                    );
                }
            }
            set_flushing_cache.set(false);
        });
    };

    // Handle walk-in CSV export
    let handle_walkin_export = move |_: web_sys::MouseEvent| {
        if walkin_exporting.get() {
            return;
        }
        let event_id = get_event_id();
        let Some(eid) = event_id else {
            components::show_toast(&set_toast, "Select an event first", ToastType::Warning);
            return;
        };
        set_walkin_exporting.set(true);
        let set_toast = set_toast;
        let set_busy = set_walkin_exporting;
        leptos::task::spawn_local(async move {
            match api::export_walkin_csv(&eid).await {
                Ok(data) => {
                    download_csv(&data.filename, &data.csv);
                    components::show_toast(
                        &set_toast,
                        &format!("Exported {} walk-in attendees", data.count),
                        ToastType::Success,
                    );
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Walk-in export failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_busy.set(false);
        });
    };

    // Handle cross-event audience CSV export (ALL events).
    //
    // This is the unique cross-event view: deduped by email across every event,
    // with per-email participation metrics (events joined, check-ins, etc.).
    // It intentionally does NOT scope to the currently selected event — the
    // per-event detail is already covered by the "Export CSV" button above.
    // No event needs to be selected.
    let handle_audience_export = move |_: web_sys::MouseEvent| {
        if audience_exporting.get() {
            return;
        }
        set_audience_exporting.set(true);
        leptos::task::spawn_local(async move {
            // None ⇒ aggregate across ALL events (matches the backend default).
            match api::export_audience_csv(None).await {
                Ok(data) => {
                    let filename = data.filename.clone();
                    let csv = data.csv.clone();
                    match (filename, csv) {
                        (Some(f), Some(c)) => {
                            download_csv(&f, &c);
                            components::show_toast(
                                &set_toast,
                                &format!("Exported {} distinct emails", data.total),
                                ToastType::Success,
                            );
                            // Orphan event_id warning — attendees from
                            // unregistered events appear here but can't be
                            // selected in the per-event admin dashboard.
                            if !data.unregistered_event_ids.is_empty() {
                                let n = data.unregistered_event_ids.len();
                                let preview = data
                                    .unregistered_event_ids
                                    .iter()
                                    .take(3)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let more = if n > 3 {
                                    format!(" (+{} more)", n - 3)
                                } else {
                                    String::new()
                                };
                                components::show_toast(
                                    &set_toast,
                                    &format!(
                                        "{n} unregistered event(s) not in the event selector: \
                                         {preview}{more}. Their attendees are visible here \
                                         but not in per-event views.",
                                    ),
                                    ToastType::Warning,
                                );
                            }
                        }
                        _ => {
                            components::show_toast(
                                &set_toast,
                                "Audience export returned no CSV payload",
                                ToastType::Warning,
                            );
                        }
                    }
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Audience export failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_audience_exporting.set(false);
        });
    };

    // Handle walk-in sync to Google Sheet
    let handle_walkin_sync = move |_: web_sys::MouseEvent| {
        if walkin_syncing.get() {
            return;
        }
        let event_id = get_event_id();
        let Some(eid) = event_id else {
            components::show_toast(&set_toast, "Select an event first", ToastType::Warning);
            return;
        };
        set_walkin_syncing.set(true);
        set_walkin_sync_result.set(None);
        let set_toast = set_toast;
        let set_busy = set_walkin_syncing;
        let set_result = set_walkin_sync_result;
        let set_refresh = set_refresh_counter;
        leptos::task::spawn_local(async move {
            match api::sync_walkins(&eid).await {
                Ok(data) => {
                    let msg = if data.errors.is_empty() {
                        format!(
                            "Synced {} walk-in attendees ({} already synced)",
                            data.synced, data.skipped
                        )
                    } else {
                        format!(
                            "Synced {} of {} walk-ins ({} errors)",
                            data.synced, data.total_walkins, data.errors.len()
                        )
                    };
                    let toast_type = if data.errors.is_empty() {
                        ToastType::Success
                    } else {
                        ToastType::Warning
                    };
                    components::show_toast(&set_toast, &msg, toast_type);
                    set_result.set(Some(data));
                    api::invalidate_attendee_cache();
                    set_refresh.update(|c| *c += 1);
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Walk-in sync failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_busy.set(false);
        });
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
                    // Quick nav — Home link at top of sidebar for easy exit
                    <div class="admin-sidebar-section admin-sidebar-topnav">
                        <a href="/" class="admin-sidebar-item admin-sidebar-home" title="Back to home">
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
                                    <polyline points="9 22 9 12 15 12 15 22"></polyline>
                                </svg>
                            </span>
                            "Home"
                        </a>
                    </div>

                    // Event selector dropdown (always visible at top of sidebar)
                    <Show when=move || !events_loading.get() && !events_list.get().is_empty() fallback=|| view! { <div></div> }>
                        <div class="admin-sidebar-section">
                            <div class="admin-sidebar-heading">"Event"</div>
                            <crate::pages::admin_event_selector::AdminEventSelector
                                events_list=events_list
                                active_event_id=active_event_id
                                set_active_event_id=set_active_event_id
                                set_refresh_counter=set_refresh_counter
                            />
                        </div>
                    </Show>

                    // ── Group 1: Event Setup (most important) ──
                    <div class="admin-sidebar-group">
                        <div class="admin-sidebar-group-label">"Event Setup"</div>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Events
                            on:click=move |_| set_active_section.set(AdminSection::Events)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect>
                                    <line x1="16" y1="2" x2="16" y2="6"></line>
                                    <line x1="8" y1="2" x2="8" y2="6"></line>
                                    <line x1="3" y1="10" x2="21" y2="10"></line>
                                </svg>
                            </span>
                            "Manage Events"
                            <span class="admin-sidebar-kbd">"Alt+1"</span>
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Quiz
                            on:click=move |_| set_active_section.set(AdminSection::Quiz)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"></path>
                                    <rect x="8" y="2" width="8" height="4" rx="1" ry="1"></rect>
                                </svg>
                            </span>
                            "Quiz"
                            <span class="admin-sidebar-kbd">"Alt+3"</span>
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::FormBuilder
                            on:click=move |_| set_active_section.set(AdminSection::FormBuilder)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                                </svg>
                            </span>
                            "Form Builder"
                            <span class="admin-sidebar-kbd">"Alt+4"</span>
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Adventure
                            on:click=move |_| set_active_section.set(AdminSection::Adventure)
                        >
                            <span class="admin-sidebar-icon"><Icon icon=IconName::Crab class="icon-sm"/></span>
                            "Adventure"
                            <span class="admin-sidebar-kbd">"Alt+5"</span>
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Campaigns
                            on:click=move |_| set_active_section.set(AdminSection::Campaigns)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M12 2L2 7l10 5 10-5-10-5z"></path>
                                    <path d="M2 17l10 5 10-5"></path>
                                    <path d="M2 12l10 5 10-5"></path>
                                </svg>
                            </span>
                            "Campaigns"
                            <span class="admin-sidebar-kbd">"Alt+2"</span>
                        </button>
                    </div>

                    // ── Group 2: Check-in (day-of-event) ──
                    <div class="admin-sidebar-group">
                        <div class="admin-sidebar-group-label">"Check-in"</div>
                        <Show when=move || current_event_format.get().has_in_person() fallback=|| view! { <div></div> }>
                            <button
                                class="admin-sidebar-item"
                                class:active=move || active_section.get() == AdminSection::Attendance && active_tab.get() == DashboardTab::InPerson
                                on:click=move |_| {
                                    set_active_section.set(AdminSection::Attendance);
                                    set_active_tab.set(DashboardTab::InPerson);
                                }
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
                                <span class="admin-sidebar-kbd">"Alt+6"</span>
                            </button>
                        </Show>
                        <Show when=move || current_event_format.get().has_online() fallback=|| view! { <div></div> }>
                            <button
                                class="admin-sidebar-item"
                                class:active=move || active_section.get() == AdminSection::Attendance && active_tab.get() == DashboardTab::Online
                                on:click=move |_| {
                                    set_active_section.set(AdminSection::Attendance);
                                    set_active_tab.set(DashboardTab::Online);
                                }
                            >
                                <span class="admin-sidebar-icon">
                                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <circle cx="12" cy="12" r="10"></circle>
                                        <line x1="2" y1="12" x2="22" y2="12"></line>
                                        <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
                                    </svg>
                                </span>
                                "Online"
                                <span class="admin-sidebar-kbd">"Alt+7"</span>
                            </button>
                        </Show>
                        // Google Sheet link — one per event (Sheet is shared by both tabs).
                        // Hidden when no event selected or sheet_id is empty.
                        <Show
                            when=move || !current_sheet_id.get().trim().is_empty()
                            fallback=|| view! { <span></span> }
                        >
                            <a
                                class="admin-sidebar-item admin-sidebar-link-external"
                                href=move || crate::utils::google_sheet_url(&current_sheet_id.get())
                                target="_blank"
                                rel="noopener noreferrer"
                                title="Open this event's Google Sheet in a new tab"
                            >
                                <span class="admin-sidebar-icon">
                                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                                        <polyline points="14 2 14 8 20 8"></polyline>
                                        <line x1="18" y1="13" x2="6" y2="13"></line>
                                        <line x1="18" y1="17" x2="6" y2="17"></line>
                                        <line x1="8" y1="9" x2="6" y2="9"></line>
                                    </svg>
                                </span>
                                "View Google Sheet"
                                <span class="admin-sidebar-external-arrow">"↗"</span>
                            </a>
                        </Show>
                    </div>

                    // ── Group 3: Payments (deposit & escrow) — only for events with in-person track ──
                    <Show when=move || current_event_format.get().has_in_person() fallback=|| view! { <div></div> }>
                    <div class="admin-sidebar-group">
                        <div class="admin-sidebar-group-label">"Payments"</div>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Deposits
                            on:click=move |_| set_active_section.set(AdminSection::Deposits)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <line x1="12" y1="1" x2="12" y2="23"></line>
                                    <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"></path>
                                </svg>
                            </span>
                            "Deposits & Refunds"
                            <span class="admin-sidebar-kbd">"Alt+8"</span>
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Escrow
                            on:click=move |_| set_active_section.set(AdminSection::Escrow)
                        >
                            <span class="admin-sidebar-icon"><Icon icon=IconName::Lock class="icon-sm"/></span>
                            "Escrow"
                            <span class="admin-sidebar-kbd">"Alt+9"</span>
                        </button>
                        <button
                            class="admin-sidebar-item"
                            class:active=move || active_section.get() == AdminSection::Cancellation
                            on:click=move |_| set_active_section.set(AdminSection::Cancellation)
                        >
                            <span class="admin-sidebar-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <circle cx="12" cy="12" r="10"></circle>
                                    <line x1="15" y1="9" x2="9" y2="15"></line>
                                    <line x1="9" y1="9" x2="15" y2="15"></line>
                                </svg>
                            </span>
                            "Cancellation"
                            <span class="admin-sidebar-kbd">"Alt+0"</span>
                        </button>
                    </div>
                    </Show>

                    // Quick stats at bottom of sidebar
                    <div class="admin-sidebar-stats">
                        {move || {
                            if active_event_id.get().is_none() {
                                view! {
                                    <div class="admin-sidebar-stats-empty">
                                        "Select an event to see stats"
                                    </div>
                                }.into_any()
                            } else {
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
                                        <span class="admin-sidebar-stat-value admin-stat-value-success">{checked_in}</span>
                                        <span class="admin-sidebar-stat-label">"Checked In"</span>
                                    </div>
                                    <div class="admin-sidebar-stat">
                                        <span class="admin-sidebar-stat-value admin-stat-value-warning">{remaining}</span>
                                        <span class="admin-sidebar-stat-label">"Remaining"</span>
                                    </div>
                                }.into_any()
                            }
                        }}
                    </div>
                </aside>

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
                        {move || {
                            let result = walkin_sync_result.get();
                            match result {
                                Some(r) => {
                                    let errors = r.errors.clone();
                                    let has_errors = !errors.is_empty();
                                    view! {
                                        <div class="admin-info-card">
                                            <div class="admin-info-card-header">"Walk-in Sync Result"</div>
                                            <div class="admin-info-card-body">
                                                <div>"Synced: "<strong>{r.synced}</strong></div>
                                                <div>"Skipped (already synced): "<strong>{r.skipped}</strong></div>
                                                <div>"Total walk-ins: "<strong>{r.total_walkins}</strong></div>
                                                <Show
                                                    when=move || has_errors
                                                    fallback=|| view! { <div></div> }
                                                >
                                                    <div class="admin-sync-errors">
                                                        <strong>"Errors:"</strong>
                                                        <ul>
                                                            {errors.iter().map(|e| view! {
                                                                <li>{utils::escape_html(e)}</li>
                                                            }).collect_view()}
                                                        </ul>
                                                    </div>
                                                </Show>
                                            </div>
                                        </div>
                                    }.into_any()
                                }
                                None => view! { <div></div> }.into_any(),
                            }
                        }}
                    </Show>

                    // Stats cards (tab-aware)
                    {move || render_stats(&stats.get(), &attendees.get(), active_tab.get(), &current_event_format.get())}

                    // Check-in velocity neon progress bar
                    {move || {
                        let list = attendees.get();
                        let total = list.len();
                        let checked_in = list.iter().filter(|a| a.checked_in_at.is_some()).count();
                        let pct = if total > 0 { (checked_in as f64 / total as f64 * 100.0) as u32 } else { 0 };
                        view! {
                            <div style="background: rgba(19, 20, 28, 0.6); border: 1px solid rgba(153, 69, 255, 0.25); border-radius: 14px; padding: 14px 18px; margin: 16px 0; backdrop-filter: blur(12px);">
                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                                    <span style="font-weight: 700; color: #fff; font-size: 0.88rem; display: flex; align-items: center; gap: 6px;">
                                        <span style="color: #14F195;">"⚡"</span>" Check-in Velocity"
                                    </span>
                                    <span style="font-weight: 800; color: #14F195; font-size: 0.88rem;">{pct}"% Checked In ("{checked_in}" / "{total}")"</span>
                                </div>
                                <div style="width: 100%; height: 8px; background: rgba(255,255,255,0.08); border-radius: 8px; overflow: hidden;">
                                    <div style=format!("width: {pct}%; height: 100%; background: linear-gradient(90deg, #9945FF, #14F195); border-radius: 8px; transition: width 0.4s ease;")></div>
                                </div>
                            </div>
                        }
                    }}

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

                                let items = visible.into_iter().map(|attendee| {
                                    let is_checked_in = attendee.checked_in_at.is_some();
                                    let is_attendee_in_person = utils::is_in_person(&attendee.participation_type);
                                    let is_vip = is_vip_ticket(&attendee.ticket_name);
                                    let is_walkin = attendee.ticket_name.eq_ignore_ascii_case("Walk-in");
                                    let is_selected = selected.contains(&attendee.api_id);
                                    let api_id = attendee.api_id.clone();
                                    let delete_id = api_id.clone();
                                    // Dedicated clone for the participation-toggle children
                                    // closure (Fn) — avoids moving api_id out of the
                                    // environment, which would break other closures.
                                    let switch_display_id = api_id.clone();
                                    let switch_click_id = api_id.clone();
                                    // Clone for the deep-link "Record slip" button — fires
                                    // the cross-section modal trigger. Same Fn-closure
                                    // reasoning as the participation-toggle clones above.
                                    let record_slip_id = api_id.clone();
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
                                    let deposit_link = match active_event_id.get() {
                                        Some(ref eid) => format!("/deposit/{api_id}?event_id={eid}"),
                                        None => format!("/deposit/{api_id}"),
                                    };
                                    let ticket_link = match active_event_id.get() {
                                        Some(ref eid) => format!("/ticket/{api_id}?event_id={eid}"),
                                        None => format!("/ticket/{api_id}"),
                                    };
                                    // Participation-aware status badges
                                    let has_nft = attendee.nft_proof_url.is_some();
                                    let nft_url = attendee.nft_proof_url.clone();

                                    // For in-person: deposit/refund badges
                                    let deposit_badge = if is_attendee_in_person {
                                        let has_deposit = attendee.deposit_amount.is_some();
                                        let is_deposit_verified = attendee.deposit_verified.as_deref() == Some("true");
                                        if attendee.refund_status.is_some() {
                                            Some(("badge badge-refunded", "Refunded"))
                                        } else if attendee.used_credit {
                                            // Got in by spending rolling credit — distinct from cash.
                                            Some(("badge badge-info", "Credit \u{2713}"))
                                        } else if is_deposit_verified {
                                            Some(("badge badge-success", "Deposit \u{2713}"))
                                        } else if has_deposit {
                                            Some(("badge badge-warning", "Deposit pending"))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None // online attendees don't deposit
                                    };

                                    // For online: show claim status when no deposit flow
                                    let claim_badge = if !is_attendee_in_person && has_nft {
                                        Some(("badge badge-success", "Claimed \u{2713}"))
                                    } else if !is_attendee_in_person && !is_checked_in {
                                        Some(("badge badge-info", "Registered"))
                                    } else {
                                        None
                                    };

                                    // "Apply Credit" is offered when an in-person attendee holds rolling
                                    // THB credit and has not completed/refunded a deposit — the backend
                                    // spends it (only if sufficient) and writes a covered deposit.
                                    // NOTE: deposit_amount is USDC-only, so it can't detect THB-stuck rows;
                                    // credit_thb (annotated by the list handler) is the correct gate.
                                    let credit_thb = attendee.credit_thb;
                                    let has_credit = credit_thb > 0;
                                    let can_apply_credit = is_attendee_in_person
                                        && has_credit
                                        && attendee.deposit_verified.as_deref() != Some("true")
                                        && attendee.refund_status.is_none();
                                    let apply_credit_id_click = attendee.api_id.clone();
                                    let apply_credit_id_disabled = attendee.api_id.clone();

                                    view! {
                                        <div class="attendee-item" class:vip=is_vip class:selected=is_selected>
                                            // Row 1: checkbox + name + badges + status indicators
                                            <div class="attendee-row-top">
                                                <button
                                                    class=format!("attendee-checkbox{}", if is_selected { " checked" } else { "" })
                                                    on:click=move |_| set_selected_ids.update(|ids| {
                                                        if ids.contains(&api_id) { ids.remove(&api_id); } else { ids.insert(api_id.clone()); }
                                                    })
                                                    disabled=is_checked_in
                                                >
                                                    {if is_selected { "✓" } else { "" }}
                                                </button>
                                                <div class="attendee-name">{utils::escape_html(&name)}</div>
                                                <span class=p_class.clone()>{p_label.clone()}</span>
                                                <span class=badge_class>{badge_text}</span>
                                                <Show
                                                    when=move || has_ticket && is_vip
                                                    fallback=|| view! { <span></span> }
                                                >
                                                    <span class="vip-badge">"VIP"</span>
                                                </Show>
                                                <Show
                                                    when=move || has_ticket && is_walkin
                                                    fallback=|| view! { <span></span> }
                                                >
                                                    <span class="walkin-badge">"Walk-in"</span>
                                                </Show>
                                                <Show
                                                    when=move || deposit_badge.is_some()
                                                    fallback=|| view! { <span></span> }
                                                >
                                                    {
                                                        let (cls, txt) = deposit_badge.unwrap_or(("", ""));
                                                        view! { <span class=cls.to_string()>{txt}</span> }
                                                    }
                                                </Show>
                                                <Show
                                                    when=move || claim_badge.is_some()
                                                    fallback=|| view! { <span></span> }
                                                >
                                                    {
                                                        let (cls, txt) = claim_badge.unwrap_or(("", ""));
                                                        view! { <span class=cls.to_string()>{txt}</span> }
                                                    }
                                                </Show>
                                                // Rolling deposit credit the attendee holds — makes credit
                                                // visible at the row level (previously only on the Held tab).
                                                <Show
                                                    when=move || has_credit
                                                    fallback=|| view! { <span></span> }
                                                >
                                                    <span class="badge badge-info" title="Rolling deposit credit available — use the Apply Credit button to cover this event">
                                                        {format!("\u{0e3f}{credit_thb} credit")}
                                                    </span>
                                                </Show>
                                                <Show
                                                    when=move || has_nft
                                                    fallback=|| view! { <span></span> }
                                                >
                                                    {
                                                        let nft_href = nft_url.clone().unwrap_or_default();
                                                        view! {
                                                            <a
                                                                href=nft_href
                                                                target="_blank"
                                                                rel="noopener noreferrer"
                                                                class="badge badge-nft"
                                                                title="View NFT"
                                                            >
                                                                "NFT ✦"
                                                            </a>
                                                        }
                                                    }
                                                </Show>
                                            </div>
                                            // Row 2: email + ticket + time ago + action buttons
                                            <div class="attendee-row-bottom">
                                                <div class="attendee-meta">
                                                    <span class="attendee-email-inline">{utils::escape_html(&email)}</span>
                                                    <Show
                                                        when=move || has_ticket
                                                        fallback=|| view! { <span></span> }
                                                    >
                                                        <span class="admin-ticket-tag">{utils::escape_html(&ticket)}</span>
                                                    </Show>
                                                    <Show
                                                        when=move || has_time_ago
                                                        fallback=|| view! { <span></span> }
                                                    >
                                                        <span class="admin-time-ago-inline">
                                                            {time_ago_str.clone()}{checked_in_by_suffix.clone()}
                                                        </span>
                                                    </Show>
                                                </div>
                                                <div class="attendee-actions">
                                                    // Deposit button — only for in-person attendees
                                                    <Show
                                                        when=move || is_attendee_in_person
                                                        fallback=|| view! { <span></span> }
                                                    >
                                                        <a
                                                            href=deposit_link.clone()
                                                            class="btn btn-outline btn-xs btn-xs-override"
                                                            title="Deposit page"
                                                        >
                                                            "Deposit"
                                                        </a>
                                                    </Show>
                                                    // Record slip on behalf of attendee — deep-links into
                                                    // the Deposits section's Record-Slip modal. Use case:
                                                    // attendee sent the slip via LINE/email and cannot upload
                                                    // themselves (JWT expired, browser bug, etc.).
                                                    // Gated on deposit_enabled for the current event.
                                                    <Show
                                                        when=move || current_deposit_enabled.get()
                                                        fallback=|| view! { <span></span> }
                                                    >
                                                        <button
                                                            class="btn btn-outline btn-xs btn-xs-override"
                                                            title="Record a slip on behalf of this attendee (they sent it via LINE/email and cannot upload themselves). Opens the deposit modal pre-filled."
                                                            on:click={
                                                                let id = record_slip_id.clone();
                                                                let set_pending = set_pending_record_slip_attendee;
                                                                let set_section = set_active_section;
                                                                move |_| {
                                                                    set_pending.set(Some(id.clone()));
                                                                    set_section.set(AdminSection::Deposits);
                                                                }
                                                            }
                                                        >
                                                            "Record Slip"
                                                        </button>
                                                    </Show>
                                                    // Apply Credit — complete a registration stuck at the
                                                    // deposit step by spending the attendee's rolling credit.
                                                    // Shown only for the stuck state (in-person, registered,
                                                    // no deposit). The backend spends credit only if it covers
                                                    // the deposit, else surfaces a toast error.
                                                    <Show
                                                        when=move || current_deposit_enabled.get() && can_apply_credit
                                                        fallback=|| view! { <span></span> }
                                                    >
                                                        <button
                                                            class="btn btn-outline btn-xs btn-xs-override"
                                                            disabled=switching_ids.get().contains(&apply_credit_id_disabled)
                                                            title="Apply this attendee's rolling deposit credit to cover this event (completes a registration stuck at the deposit step). No effect if they have insufficient credit."
                                                            on:click={
                                                                let aid = apply_credit_id_click.clone();
                                                                let set_toast = set_toast;
                                                                let set_switching_ids = set_switching_ids;
                                                                let set_refresh_counter = set_refresh_counter;
                                                                let eid = event_id_for_delete.get();
                                                                move |_| {
                                                                    let aid = aid.clone();
                                                                    let set_toast = set_toast;
                                                                    let set_switching_ids = set_switching_ids;
                                                                    let set_refresh_counter = set_refresh_counter;
                                                                    let eid = eid.clone();
                                                                    set_switching_ids.update(|ids| { ids.insert(aid.clone()); });
                                                                    leptos::task::spawn_local(async move {
                                                                        let body = api::ApplyCreditRequest { event_id: eid.clone().unwrap_or_default() };
                                                                        match api::apply_credit(&aid, &body).await {
                                                                            Ok(_) => {
                                                                                api::invalidate_attendee_cache();
                                                                                set_refresh_counter.update(|c| *c += 1);
                                                                                components::show_toast(
                                                                                    &set_toast,
                                                                                    "Rolling credit applied \u{2014} registration completed",
                                                                                    ToastType::Success,
                                                                                );
                                                                            }
                                                                            Err(e) => {
                                                                                components::show_toast(
                                                                                    &set_toast,
                                                                                    &format!("Apply credit failed: {e}"),
                                                                                    ToastType::Error,
                                                                                );
                                                                            }
                                                                        }
                                                                        set_switching_ids.update(|ids| { ids.remove(&aid); });
                                                                    });
                                                                }
                                                            }
                                                        >
                                                            "Apply Credit"
                                                        </button>
                                                    </Show>
                                                    // Participation-type toggle — flip In-Person ⇄ Online.
                                                    // Use case: attendee chose deposit/in-person but confirmed
                                                    // out-of-band they'll attend online (or vice-versa).
                                                    // Hidden for walk-ins (their participation_type is 'walkin').
                                                    <Show
                                                        when=move || !is_walkin
                                                        fallback=|| view! { <span></span> }
                                                    >
                                                        <button
                                                            class="btn btn-outline btn-xs btn-xs-override admin-participation-toggle"
                                                            disabled=switching_ids.get().contains(&switch_display_id)
                                                            title=if is_attendee_in_person {
                                                                "Switch to Online (confirmed out-of-band)"
                                                            } else {
                                                                "Switch to In-Person"
                                                            }
                                                            on:click={
                                                                let switch_id = switch_click_id.clone();
                                                                let target_mode = if is_attendee_in_person { "Online" } else { "In-Person" };
                                                                let target_label = target_mode.to_string();
                                                                let set_toast = set_toast;
                                                                let set_switching_ids = set_switching_ids;
                                                                let set_refresh_counter = set_refresh_counter;
                                                                let eid = event_id_for_delete.get();
                                                                move |_| {
                                                                    let aid = switch_id.clone();
                                                                    let mode = target_label.clone();
                                                                    let set_toast = set_toast;
                                                                    let set_switching_ids = set_switching_ids;
                                                                    let set_refresh_counter = set_refresh_counter;
                                                                    let eid = eid.clone();
                                                                    set_switching_ids.update(|ids| { ids.insert(aid.clone()); });
                                                                    leptos::task::spawn_local(async move {
                                                                        match api::update_participation_type(&aid, eid.as_deref(), &mode).await {
                                                                            Ok(_) => {
                                                                                api::invalidate_attendee_cache();
                                                                                set_refresh_counter.update(|c| *c += 1);
                                                                                components::show_toast(
                                                                                    &set_toast,
                                                                                    &format!("Switched to {mode}"),
                                                                                    ToastType::Success,
                                                                                );
                                                                            }
                                                                            Err(e) => {
                                                                                components::show_toast(
                                                                                    &set_toast,
                                                                                    &format!("Switch failed: {e}"),
                                                                                    ToastType::Error,
                                                                                );
                                                                            }
                                                                        }
                                                                        set_switching_ids.update(|ids| { ids.remove(&aid); });
                                                                    });
                                                                }
                                                            }
                                                        >
                                                            // Static label (computed once, like the Delete button).
                                                            // The whole list re-renders via refresh_counter after success,
                                                            // so no reactive closure is needed here.
                                                            {if is_attendee_in_person { "→ Online" } else { "→ In-Person" }}
                                                        </button>
                                                    </Show>
                                                    <button
                                                        class="btn btn-outline btn-xs btn-xs-override"
                                                        title="Copy ticket link"
                                                        on:click={
                                                            let ticket_link = ticket_link.clone();
                                                            move |_| {
                                                                let full_url = format!("{}{}",
                                                                    web_sys::window()
                                                                        .and_then(|w| w.location().origin().ok())
                                                                        .unwrap_or_default(),
                                                                    ticket_link
                                                                );
                                                                let clipboard = web_sys::window()
                                                                    .unwrap()
                                                                    .navigator()
                                                                    .clipboard();
                                                                let _ = clipboard.write_text(&full_url);
                                                                components::show_toast(
                                                                    &set_toast,
                                                                    "Ticket link copied!",
                                                                    ToastType::Success,
                                                                );
                                                            }
                                                        }
                                                    >
                                                        "Ticket"
                                                    </button>
                                                    <button
                                                        class={
                                                            let is_confirming = confirm_delete_id.get().as_deref() == Some(&delete_id);
                                                            let is_deleting = deleting_ids.get().contains(&delete_id);
                                                            if is_deleting {
                                                                "btn btn-xs btn-xs-override".to_string()
                                                            } else if is_confirming {
                                                                "btn btn-confirm-danger btn-xs btn-xs-override".to_string()
                                                            } else {
                                                                "btn btn-danger btn-xs btn-xs-override".to_string()
                                                            }
                                                        }
                                                        disabled=deleting_ids.get().contains(&delete_id)
                                                        title="Delete attendee"
                                                        on:click={
                                                            let delete_id = delete_id.clone();
                                                            move |_| {
                                                                let is_confirming = confirm_delete_id.get().as_deref() == Some(&delete_id);
                                                                if is_confirming {
                                                                    set_confirm_delete_id.set(None);
                                                                    set_deleting_ids.update(|ids| { ids.insert(delete_id.clone()); });
                                                                    let aid = delete_id.clone();
                                                                    let eid = event_id_for_delete.get();
                                                                    let set_toast = set_toast;
                                                                    let set_deleting_ids = set_deleting_ids;
                                                                    let set_refresh_counter = set_refresh_counter;
                                                                    leptos::task::spawn_local(async move {
                                                                        match api::delete_attendee(&aid, eid.as_deref()).await {
                                                                            Ok(()) => {
                                                                                components::show_toast(&set_toast, "Attendee deleted", ToastType::Success);
                                                                                api::invalidate_attendee_cache();
                                                                                set_refresh_counter.update(|c| *c += 1);
                                                                            }
                                                                            Err(e) => {
                                                                                components::show_toast(&set_toast, &format!("Delete failed: {e}"), ToastType::Error);
                                                                            }
                                                                        }
                                                                        set_deleting_ids.update(|ids| { ids.remove(&aid); });
                                                                    });
                                                                } else {
                                                                    set_confirm_delete_id.set(Some(delete_id.clone()));
                                                                    let set_confirm = set_confirm_delete_id;
                                                                    gloo_timers::callback::Timeout::new(3000, move || {
                                                                        set_confirm.set(None);
                                                                    }).forget();
                                                                }
                                                            }
                                                        }
                                                    >
                                                        {
                                                            let is_confirming = confirm_delete_id.get().as_deref() == Some(&delete_id);
                                                            let is_deleting = deleting_ids.get().contains(&delete_id);
                                                            if is_deleting { "Deleting..." } else if is_confirming { "⚠ Confirm?" } else { "Delete" }
                                                        }
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect_view();

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
    event_id: Option<String>,
    set_qr_generating: WriteSignal<bool>,
    set_qr_result: WriteSignal<Option<GenerateQrData>>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) {
    set_qr_generating.set(true);
    leptos::task::spawn_local(async move {
        match api::generate_qrs(force, event_id.as_deref()).await {
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
                api::invalidate_attendee_cache();
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
    attendees: &[AttendeeListItem],
    tab: DashboardTab,
    event_format: &EventFormat,
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

                // Cross-tab summary — only for hybrid events with both tracks
                {if event_format == &EventFormat::Hybrid {
                    view! {
                        <div class="admin-cross-tab-summary">
                            {format!("{} {} attendee{}", other_count, other_tab.label(), if other_count != 1 { "s" } else { "" })}
                            " — "
                            <span
                                class="admin-tab-switch-link"
                                on:click=move |_| {
                                    // Tab summary is informational; switching is done via the tab bar
                                }
                            >
                                "switch tab to view"
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}

                // Progress bar
                <div class="card mb-2">
                    <div class="admin-progress-header">
                        <span class="admin-progress-title">
                            {format!("{} Progress", tab.label())}
                        </span>
                        <span class="admin-progress-pct">
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
                <div class="card mb-2 admin-qr-result-card">
                    <div class="admin-qr-result-header">

                        <span class="admin-qr-result-title">
                            "QR Codes Generated"
                        </span>
                    </div>
                    <div class="admin-qr-stats-row">
                        <div>
                            <span class="admin-qr-count-success">{generated}</span>
                            <span class="admin-qr-count-label">" created"</span>
                        </div>
                        <Show when=move || has_skipped fallback=|| view! { <div></div> }>
                            <div>
                                <span class="admin-qr-count-warning">{skipped}</span>
                                <span class="admin-qr-count-label">" skipped (already exist)"</span>
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
    attendees: &[AttendeeListItem],
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
                        <h3 class="admin-section-heading">"Recent Check-Ins"</h3>
                        <div class="admin-empty-state-sm">
                            {format!("No recent {} check-ins", tab.label().to_lowercase())}
                        </div>
                    </div>
                }
                    .into_any();
            }

            view! {
                <div class="card mt-3">
                    <h3 class="admin-section-heading">
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
                                    <div class="attendee-row-top">
                                        <div class="attendee-name">{utils::escape_html(&name)}</div>
                                        <span class=format!("{p_class} admin-badge-inline")>
                                            {p_label.clone()}
                                        </span>
                                    </div>
                                    <div class="attendee-row-bottom">
                                        <div class="attendee-meta">
                                            <span class="attendee-email-inline admin-recent-email">
                                                {utils::escape_html(&api_id)}
                                            </span>
                                        </div>
                                        <div class="admin-checkin-time">
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
