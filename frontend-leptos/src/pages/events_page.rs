//! Events management page — list, create, and configure events.
//!
//! Provides a full UI for listing, creating, editing, and archiving events
//! in the BeThere admin dashboard.

use leptos::prelude::*;

use crate::api;
use crate::components;
use crate::icons::{Icon, IconName};
use crate::utils;



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
    event_format: api::EventFormat,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: String,
    pub deposit_amount_thb: String,
    promptpay_id: String,
    pub escrow_address: String,
    pub escrow_status: api::EscrowStatus,
    pub organizer_wallet: String,
    pub on_chain_event_id: String,
    pub refund_deadline_hours: String,
    pub max_refundable_deposits: String,
    /// Server-side `updated_at` captured at load time for optimistic concurrency.
    pub updated_at: String,
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

/// Get self-hosted NFT badge URLs (served by the worker itself).
/// No Arweave/IPFS needed — the worker hosts the default badge and dynamic metadata.
fn get_self_hosted_nft_urls() -> (String, String) {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "https://bethere.solana-thailand.workers.dev".to_string());
    let image_url = format!("{origin}/api/badge-hd.svg");
    let metadata_uri_prefix = format!("{origin}/api/metadata/");
    (image_url, metadata_uri_prefix)
}

/// Create a default form state with sensible defaults.
fn default_form() -> EventForm {
    EventForm {
        sheet_name: "checkin".to_string(),
        staff_sheet_name: "staff".to_string(),
        event_format: api::EventFormat::InPerson,
        quiz_enabled: true,
        deposit_enabled: true,
        deposit_amount_usdc: String::new(),
        deposit_amount_thb: String::new(),
        promptpay_id: String::new(),
        escrow_address: String::new(),
        escrow_status: api::EscrowStatus::None,
        organizer_wallet: String::new(),
        on_chain_event_id: String::new(),
        refund_deadline_hours: String::new(),
        max_refundable_deposits: String::new(),
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
        event_format: detail.event_format.clone(),
        deposit_enabled: detail.deposit_enabled,
        deposit_amount_usdc: if detail.deposit_amount_usdc > 0 { format!("{:.6}", detail.deposit_amount_usdc as f64 / 1_000_000.0).trim_end_matches('0').trim_end_matches('.').to_string() } else { String::new() },
        deposit_amount_thb: if detail.deposit_amount_thb > 0 { detail.deposit_amount_thb.to_string() } else { String::new() },
        promptpay_id: detail.promptpay_id.clone(),
        escrow_address: detail.escrow_address.clone(),
        escrow_status: detail.escrow_status.clone(),
        organizer_wallet: detail.organizer_wallet.clone(),
        on_chain_event_id: if detail.on_chain_event_id > 0 { detail.on_chain_event_id.to_string() } else { String::new() },
        refund_deadline_hours: if detail.refund_deadline_hours > 0 { detail.refund_deadline_hours.to_string() } else { String::new() },
        max_refundable_deposits: if detail.max_refundable_deposits > 0 { detail.max_refundable_deposits.to_string() } else { String::new() },
        updated_at: detail.updated_at.clone(),
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
    let (saving, set_saving) = signal(false);
    let (slug_manually_edited, set_slug_manually_edited) = signal(false);
    let (refresh_counter, set_refresh_counter) = signal(0u32);

    // Section collapse signals (true = expanded)
    let (sec_basic_open, set_sec_basic_open) = signal(true);
    let (sec_schedule_open, set_sec_schedule_open) = signal(true);
    let (sec_sheets_open, set_sec_sheets_open) = signal(true);
    let (sec_nft_open, set_sec_nft_open) = signal(true);
    let (sec_settings_open, set_sec_settings_open) = signal(true);
    let (sec_deposit_open, set_sec_deposit_open) = signal(true);
    let (sec_people_open, set_sec_people_open) = signal(true);
    let (search_query, set_search_query) = signal(String::new());
    let search_input_ref: NodeRef<leptos::html::Input> = NodeRef::new();
    let (slug_taken, set_slug_taken) = signal(false);

    // Wallet connection state for combined Create Event + Escrow Init flow.
    // In Create mode, user connects wallet before clicking Create Event.
    // The save handler chains: create_event → init_escrow → sign+send TX.
    let (create_wallet_name, set_create_wallet_name) = signal(String::new());
    let (create_wallet_pk, set_create_wallet_pk) = signal(String::new());
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Detect installed wallets on mount (poll for late-injecting extensions).
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = super::escrow_init::get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo::timers::future::TimeoutFuture::new(300).await;
                    wallets = super::escrow_init::get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[events-page] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

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
        set_slug_manually_edited.set(false);
        // Reset wallet state so stale escrow fields don't leak from a prior Edit session
        set_create_wallet_name.set(String::new());
        set_create_wallet_pk.set(String::new());
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
        let val = event_target_value(&ev);
        set_form.update(|f| f.slug = val);
        set_slug_taken.set(false);
    };

    // Check slug availability on blur (client-side check against loaded events)
    let handle_slug_blur = move |_| {
        let current_slug = form.get().slug.trim().to_lowercase();
        if current_slug.is_empty() {
            set_slug_taken.set(false);
            return;
        }
        let editing = editing_id.get().unwrap_or_default();
        let taken = events.get().iter().any(|e| {
            e.slug.to_lowercase() == current_slug && e.id != editing
        });
        set_slug_taken.set(taken);
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
        // Check slug availability (client-side against loaded events)
        if slug_taken.get() {
            components::show_toast(&set_toast, "This slug is already taken by another event", components::ToastType::Error);
            return;
        }
        if current_form.sheet_id.trim().is_empty() {
            components::show_toast(&set_toast, "Google Sheet ID is required", components::ToastType::Error);
            return;
        }

        // Validate schedule — backend requires positive start_ms and end > start
        let start_ms = parse_date_to_ms(&current_form.event_start).unwrap_or(0);
        let end_ms = parse_date_to_ms(&current_form.event_end).unwrap_or(0);
        if start_ms <= 0 {
            components::show_toast(&set_toast, "Event start date is required", components::ToastType::Error);
            return;
        }
        if end_ms <= 0 {
            components::show_toast(&set_toast, "Event end date is required", components::ToastType::Error);
            return;
        }
        if end_ms <= start_ms {
            components::show_toast(&set_toast, "Event end must be after event start", components::ToastType::Error);
            return;
        }

        // Validate deposit fields when deposit is enabled
        if current_form.deposit_enabled {
            let usdc_val = current_form.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
            let thb_val = current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0);

            // At least one deposit amount must be set
            if usdc_val == 0.0 && thb_val == 0 {
                components::show_toast(&set_toast, "At least one deposit amount (USDC or THB) is required when deposit is enabled", components::ToastType::Error);
                return;
            }

            // USDC minimum precision (6 decimals → 0.01 smallest meaningful)
            if usdc_val > 0.0 && usdc_val < 0.01 {
                components::show_toast(&set_toast, "Minimum deposit is 0.01 USDC", components::ToastType::Error);
                return;
            }

            // USDC max cap (SEC-003: backend enforces $1,000 = 1,000,000,000 lamports)
            if usdc_val > 1000.0 {
                components::show_toast(&set_toast, "Maximum deposit is 1,000 USDC", components::ToastType::Error);
                return;
            }

            // THB amount set but no PromptPay ID — QR generation will fail
            if thb_val > 0 && current_form.promptpay_id.trim().is_empty() {
                components::show_toast(&set_toast, "PromptPay ID is required when THB amount is set", components::ToastType::Error);
                return;
            }

            // In Create mode with USDC deposit > 0, wallet connection is required
            // so the event is created with on-chain escrow in one atomic flow.
            if is_create && usdc_val > 0.0 && create_wallet_pk.get().is_empty() {
                components::show_toast(
                    &set_toast,
                    "Connect your Solana wallet to create event with USDC deposit escrow",
                    components::ToastType::Error,
                );
                return;
            }

            // Escrow init requires USDC amount — check early when wallet is connected
            let do_escrow_init = !create_wallet_pk.get().is_empty()
                && !create_wallet_name.get().is_empty();
            if do_escrow_init && usdc_val == 0.0 {
                components::show_toast(
                    &set_toast,
                    "USDC deposit amount is required to initialize on-chain escrow",
                    components::ToastType::Error,
                );
                return;
            }

            let deadline_hrs = current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0);
            if deadline_hrs == 0 {
                components::show_toast(&set_toast, "Refund deadline must be at least 1 hour", components::ToastType::Error);
                return;
            }
        }

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
                // In Create mode, wallet PK lives in create_wallet_pk signal (not form field).
                // Use it when available so backend has organizer_wallet for escrow init.
                organizer_wallet: if create_wallet_pk.get().is_empty() {
                    current_form.organizer_wallet.trim().to_string()
                } else {
                    create_wallet_pk.get()
                },
                on_chain_event_id: current_form.on_chain_event_id.parse::<u64>().unwrap_or(0),
                refund_deadline_hours: current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0),
                max_refundable_deposits: current_form.max_refundable_deposits.parse::<u32>().unwrap_or(0),
                event_format: current_form.event_format.clone(),
            };

            // Determine if we should also initialize escrow after creating the event.
            // This happens when: deposit_enabled + wallet connected in Create mode.
            let do_escrow_init = current_form.deposit_enabled
                && !create_wallet_pk.get().is_empty()
                && !create_wallet_name.get().is_empty();
            let wn = create_wallet_name.get();
            let pk = create_wallet_pk.get();

            leptos::task::spawn_local(async move {
                // Step 1: Create the event
                let created = match api::create_event(&body).await {
                    Ok(data) => {
                        log::info!("[events-page] event created: id={}", data.id);
                        data
                    }
                    Err(e) => {
                        log::error!("[events-page] create failed: {e}");
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to create event: {e}"),
                            components::ToastType::Error,
                        );
                        set_saving.set(false);
                        return;
                    }
                };

                // Step 2: Initialize escrow on-chain (if wallet connected + deposit enabled)
                if do_escrow_init {
                    log::info!("[events-page] initializing escrow for event {}...", created.id);
                    let req = api::InitEscrowRequest {
                        event_id: created.id.clone(),
                    };
                    match api::init_escrow(&req).await {
                        Ok(resp) => {
                            // SEC-014: Verify wallet cluster matches expected network.
                            let expected_cluster = crate::utils::get_cluster();
                            if let Err(cluster_err) = super::escrow_init::check_wallet_cluster(&wn, &expected_cluster).await {
                                log::error!("[events-page] cluster mismatch: {cluster_err}");
                                components::show_toast(
                                    &set_toast,
                                    &cluster_err,
                                    components::ToastType::Error,
                                );
                                set_saving.set(false);
                                return;
                            }
                            log::info!("[events-page] escrow TX built, signing via {}...", wn);
                            match super::escrow_init::sign_and_send_tx_js(&wn, &resp.transaction).await {
                                Some(signature) => {
                                    log::info!("[events-page] escrow TX confirmed: {}", signature);
                                    // Update the event with escrow fields from the response
                                    let update_body = api::UpdateEventBody {
                                        escrow_address: Some(resp.escrow_address.clone()),
                                        escrow_status: Some(api::EscrowStatus::Initialized),
                                        organizer_wallet: Some(pk.clone()),
                                        on_chain_event_id: Some(resp.on_chain_event_id),
                                        expected_updated_at: if created.updated_at.is_empty() { None } else { Some(created.updated_at.clone()) },
                                        ..Default::default()
                                    };
                                    if let Err(e) = api::update_event(&created.id, &update_body).await {
                                        log::warn!("[events-page] failed to save escrow fields: {e}");
                                    }
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Event '{}' created + escrow initialized", created.name),
                                        components::ToastType::Success,
                                    );
                                }
                                None => {
                                    log::error!("[events-page] escrow TX rejected by wallet");
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Event '{}' created, but escrow TX was rejected. Edit event to retry.", created.name),
                                        components::ToastType::Warning,
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[events-page] init_escrow failed: {e}");
                            components::show_toast(
                                &set_toast,
                                &format!("Event '{}' created, but escrow init failed: {e}. Edit event to retry.", created.name),
                                components::ToastType::Warning,
                            );
                        }
                    }
                } else {
                    components::show_toast(
                        &set_toast,
                        &format!("Event '{}' created", created.name),
                        components::ToastType::Success,
                    );
                }

                set_current_view.set(EventsView::List);
                do_reload();
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
                escrow_status: None, // not updated via general form — escrow panel manages this
                organizer_wallet: Some(current_form.organizer_wallet.trim().to_string()),
                on_chain_event_id: Some(current_form.on_chain_event_id.parse::<u64>().unwrap_or(0)),
                refund_deadline_hours: Some(current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0)),
                max_refundable_deposits: Some(current_form.max_refundable_deposits.parse::<u32>().unwrap_or(0)),
                expected_updated_at: if current_form.updated_at.is_empty() { None } else { Some(current_form.updated_at.clone()) },
                event_format: Some(current_form.event_format.clone()),
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
                        let msg = if e.message.contains("conflict") {
                            "Event was modified by another user. Please reload the event and re-apply your changes.".to_string()
                        } else {
                            format!("Failed to update event: {e}")
                        };
                        components::show_toast(&set_toast, &msg, components::ToastType::Error);
                    }
                }
                set_saving.set(false);
            });
        }
    };

    // Handle cancel
    let handle_cancel = move |_: web_sys::MouseEvent| {
        set_current_view.set(EventsView::List);
        set_editing_id.set(None);
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
                                    format!("{trunc}…")
                                };
                                let needs_escrow = evt.deposit_enabled && evt.escrow_address.is_empty();
                                let has_escrow = evt.deposit_enabled && !evt.escrow_address.is_empty();
                                let fmt_label = evt.event_format.label();
                                let fmt_badge_class = match evt.event_format {
                                    api::EventFormat::InPerson => "badge badge-info-xs",
                                    api::EventFormat::Online => "badge badge-warning-xs",
                                    api::EventFormat::Hybrid => "badge badge-success-xs",
                                };
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
                                            <div class="flex-row-gap" style="flex-wrap:wrap;align-items:center">
                                                <span class="card-title">{ename.clone()}</span>
                                                <span class=badge_class>{status_text}</span>
                                                <span class=fmt_badge_class>{fmt_label}</span>
                                                {if needs_escrow {
                                                    view! {
                                                        <span class="badge badge-warning-xs">"No Escrow"</span>
                                                    }.into_any()
                                                } else if has_escrow {
                                                    view! {
                                                        <span class="badge badge-success-xs">"Escrow"</span>
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

                            view! {
                                <div class="card">
                                    <div class="card-header">
                                        <div class="flex-row-gap" style="flex-wrap:wrap">
                                            <span class="card-title">{ename.clone()}</span>
                                            <span class=badge_class>{status_text}</span>
                                            <span class=fmt_badge_class>{fmt_label}</span>
                                            {if needs_escrow {
                                                view! {
                                                    <span class="badge badge-warning-xs">"No Escrow"</span>
                                                }.into_any()
                                            } else if has_escrow {
                                                view! {
                                                    <span class="badge badge-success-xs">"Escrow"</span>
                                                }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                        </div>
                                        {if can_manage { view! {
                                        <div class="flex-row-gap" style="gap:0.5rem">
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
                                            // Uses force=true to bypass escrow guard on devnet
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
                                    <div class="flex-row-gap" style="align-items:center">
                                        <span class="card-title" style="font-size:1rem">{ename}</span>
                                        <span class=badge_class>{status_text}</span>
                                        <span class=fmt_badge_class>{fmt_label}</span>
                                        {if needs_escrow {
                                            view! { <span class="badge badge-warning-xs">"No Escrow"</span> }.into_any()
                                        } else if has_escrow {
                                            view! { <span class="badge badge-success-xs">"Escrow"</span> }.into_any()
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

            // === Create / Edit Form View ===
            <Show when=move || current_view.get() != EventsView::List fallback=|| view! { <div></div> }>
                <div class="card">
                    // Title is reactive — re-renders heading text only, not the form inputs
                    <h2 class="admin-section-heading">{move || if current_view.get() == EventsView::Edit { "Edit Event" } else { "Create Event" }}</h2>

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
                                                on:focusout=handle_slug_blur
                                            />
                                            <Show
                                                when=move || slug_taken.get()
                                                fallback=|| view! { <div></div> }
                                            >
                                                <div class="hint-warning-xs">
                                                    "This slug is already taken by another event"
                                                </div>
                                            </Show>
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
                                    <span class="form-section-badge form-section-badge-required">"Required"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_schedule_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_schedule_open.get()>
                                    <div class="quiz-settings-grid">
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Event Start"<span class="field-required-badge">"Required"</span></label>
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
                                        <label class="quiz-field-label">"Event End"<span class="field-required-badge">"Required"</span></label>
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
                                    <Show
                                        when=move || {
                                            let start = parse_date_to_ms(&form.get().event_start).unwrap_or(0);
                                            let end = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                                            start > 0 && end > 0 && end <= start
                                        }
                                        fallback=|| view! { <div></div> }
                                    >
                                        <div class="hint-warning-xs">
                                            "Event end must be after event start"
                                        </div>
                                    </Show>
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
                                    // ── Quick Guide: How to get Google Sheet ID ──
                                    <div class="sheet-guide-box" style="margin-bottom:0.75rem">
                                        <div class="sheet-guide-heading"><Icon icon=IconName::Copy class="icon-sm"/>" Quick Guide"</div>
                                        <ol class="sheet-guide-steps">
                                            <li>
                                                <a
                                                    href="https://docs.google.com/forms/create"
                                                    target="_blank"
                                                    rel="noopener"
                                                    class="sheet-guide-link"
                                                >
                                                    "Create a Google Form"
                                                </a>
                                                " → add your registration fields"
                                            </li>
                                            <li>"In the Form editor → Responses tab → click 'Link to Sheets'"</li>
                                            <li>"Copy the Sheet ID from the URL:"</li>
                                        </ol>
                                        <div class="sheet-guide-url">
                                            <code>"docs.google.com/spreadsheets/d/"</code>
                                            <code class="sheet-guide-highlight">"YOUR_SHEET_ID"</code>
                                            <code>"/edit"</code>
                                        </div>
                                    </div>

                                    <div class="quiz-settings-grid">
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"Sheet ID"<span class="field-required-badge">"Required"</span></label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="Paste Google Sheet ID here"
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
                                    <span class="form-section-title">"NFT Attendance Badge"</span>
                                    <span class="form-section-badge form-section-badge-recommended">"Recommended"</span>
                                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_nft_open.get()>"▼"</span>
                                </div>
                                <div class="form-section-body" class:form-section-body-hidden=move || !sec_nft_open.get()>
                                    // ── Quick-fill default badge ──
                                    <div class="quiz-setting-item" style="grid-column:1/-1">
                                        <div class="hint-info" style="margin-bottom:var(--space-2xs)">
                                            "NFT badges reward attendees for showing up. Use the default BeThere badge or skip if you don't need one."
                                        </div>
                                        <div style="display:flex;gap:var(--space-xs);align-items:center;flex-wrap:wrap">
                                            <button
                                                class="btn btn-outline btn-sm"
                                                on:click=move |_| {
                                                    let (img_url, meta_prefix) = get_self_hosted_nft_urls();
                                                    let eid = editing_id.get().unwrap_or_default();
                                                    set_form.update(|f| {
                                                        f.nft_image_url = img_url;
                                                        if !eid.is_empty() {
                                                            f.nft_metadata_uri = format!("{meta_prefix}{eid}");
                                                        }
                                                        f.nft_name_template = "BeThere - {event_name}".to_string();
                                                        f.nft_symbol = "BETHERE".to_string();
                                                        f.nft_description_template = "Proof of attendance at {event_name}".to_string();
                                                    });
                                                }
                                            >
                                                <Icon icon=IconName::Palette class="icon-sm"/>" Use default badge"
                                            </button>
                                            <button
                                                class="btn btn-outline btn-sm"
                                                on:click=move |_| {
                                                    set_form.update(|f| {
                                                        f.nft_image_url = String::new();
                                                        f.nft_metadata_uri = String::new();
                                                        f.nft_name_template = String::new();
                                                        f.nft_symbol = String::new();
                                                        f.nft_description_template = String::new();
                                                        f.nft_collection_mint = String::new();
                                                        f.merkle_tree = String::new();
                                                    });
                                                }
                                            >
                                                "✕ Skip NFT"
                                            </button>
                                            // Badge preview
                                            <Show
                                                when=move || !form.get().nft_image_url.is_empty()
                                                fallback=|| view! { <div></div> }
                                            >
                                                <img
                                                    src=move || form.get().nft_image_url
                                                    alt="NFT badge preview"
                                                    style="width:48px;height:48px;border-radius:8px;border:1px solid var(--border,rgba(255,255,255,0.1))"
                                                />
                                                <span style="font-size:var(--text-xs);color:var(--text-muted,#888)">
                                                    {move || {
                                                        let url = form.get().nft_image_url;
                                                        if url.contains("/api/badge") { "Default badge" } else { "Custom badge" }.to_string()
                                                    }}
                                                </span>
                                            </Show>
                                        </div>
                                    </div>
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
                                        <Show
                                            when=move || {
                                                let v = form.get().nft_collection_mint.trim().to_string();
                                                !v.is_empty() && !v.chars().all(|c| c.is_ascii_alphanumeric() && (c.is_ascii_digit() || (c >= 'A' && c <= 'H') || (c >= 'J' && c <= 'N') || (c >= 'P' && c <= 'Z') || (c >= 'a' && c <= 'k') || (c >= 'm' && c <= 'z')))
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "Invalid base58 characters detected (expected Solana address format)"
                                            </div>
                                        </Show>
                                        <div class="quiz-setting-hint" style="display:flex;justify-content:space-between;align-items:center;">
                                            <span>"Solana mint address (base58)"</span>
                                            <Show
                                                when=move || !form.get().nft_collection_mint.trim().is_empty()
                                                fallback=|| view! { <span></span> }
                                            >
                                                <a
                                                    href=move || crate::utils::metaplex_explorer_url(&form.get().nft_collection_mint.trim(), &crate::utils::get_cluster())
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    style="font-size:0.75rem;color:var(--info);text-decoration:none;"
                                                >
                                                    "Verify on Metaplex ↗"
                                                </a>
                                            </Show>
                                        </div>
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
                                        <Show
                                            when=move || {
                                                let v = form.get().nft_metadata_uri.trim().to_string();
                                                !v.is_empty() && !v.starts_with("http://") && !v.starts_with("https://")
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "URI must start with http:// or https://"
                                            </div>
                                        </Show>
                                        <Show
                                            when=move || editing_id.get().unwrap_or_default().is_empty()
                                            fallback=|| view! { <div></div> }
                                        >
                                            <span class="quiz-setting-hint">"Save the event first, then edit to auto-fill this with the dynamic metadata URL."</span>
                                        </Show>
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
                                        <Show
                                            when=move || {
                                                let v = form.get().nft_image_url.trim().to_string();
                                                !v.is_empty() && !v.starts_with("http://") && !v.starts_with("https://")
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "URL must start with http:// or https://"
                                            </div>
                                        </Show>
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
                                        <label class="quiz-field-label">"Claim Base URL"<span class="field-optional-badge">"Auto"</span></label>
                                        <input
                                            type="text"
                                            class="quiz-number-input"
                                            placeholder="Leave empty to auto-use current domain"
                                            prop:value=move || form.get().claim_base_url
                                            on:input=move |ev| set_form.update(|f| f.claim_base_url = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">
                                            "The base URL for attendee claim links (e.g. "
                                            <code style="font-size:inherit">"https://bethere.solana-thailand.workers.dev/claim"</code>
                                            "). Leave empty — the system auto-generates claim links from your current domain."
                                        </span>
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
                                    <Show when=move || current_view.get() == EventsView::Edit fallback=|| view! { <div></div> }>
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
                                    </Show>
                                </div>
                                </div>
                            </div>

                            // ── Event Format ──
                            <div class="dep-config-row">
                                <span class="dep-config-label">"Format"</span>
                                <select
                                    class="form-select form-select-sm"
                                    on:change=move |ev| {
                                        let val = event_target_value(&ev);
                                        let fmt = match val.as_str() {
                                            "online" => api::EventFormat::Online,
                                            "hybrid" => api::EventFormat::Hybrid,
                                            _ => api::EventFormat::InPerson,
                                        };
                                        set_form.update(|f| {
                                            f.event_format = fmt.clone();
                                            // Auto-sync deposit: in-person/hybrid = enabled
                                            f.deposit_enabled = fmt.has_in_person();
                                        });
                                    }
                                    prop:value=move || form.get().event_format.as_str()
                                >
                                    <option value="in_person">"In-Person"</option>
                                    <option value="online">"Online"</option>
                                    <option value="hybrid">"Hybrid"</option>
                                </select>
                            </div>
                            // Format description
                            <div class="dep-info-note">
                                <p class="hint-note">
                                    {move || match form.get().event_format {
                                        api::EventFormat::InPerson => "Physical event with deposit commitment. Attendees get 100% refund at check-in.",
                                        api::EventFormat::Online => "Virtual event. No deposit — quest completion serves as virtual check-in.",
                                        api::EventFormat::Hybrid => "Both in-person and online tracks. In-person attendees deposit; online attendees complete quests.",
                                    }}
                                </p>
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
                                            <label class="quiz-field-label">"USDC Amount"<span class="field-optional-badge">"Required for escrow"</span></label>
                                        <input
                                            type="number"
                                            class="quiz-number-input"
                                            placeholder="e.g. 10 (whole USDC)"
                                            step="0.01"
                                            min="0.01"
                                            prop:value=move || form.get().deposit_amount_usdc
                                            on:input=move |ev| set_form.update(|f| f.deposit_amount_usdc = event_target_value(&ev))
                                        />
                                        <Show
                                            when=move || {
                                                let val = form.get().deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
                                                form.get().deposit_enabled && val > 0.0 && val < 0.01
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "Minimum deposit is 0.01 USDC"
                                            </div>
                                        </Show>
                                        <Show
                                            when=move || {
                                                let val = form.get().deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
                                                let thb = form.get().deposit_amount_thb.parse::<u64>().unwrap_or(0);
                                                form.get().deposit_enabled && val == 0.0 && thb == 0
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "At least one deposit amount (USDC or THB) is required"
                                            </div>
                                        </Show>
                                        <span class="quiz-setting-hint">"Amount in whole USDC (e.g. 10 = 10 USDC). Max: 1,000 USDC"</span>
                                    </div>
                                    <Show
                                        when=move || {
                                            let val = form.get().deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
                                            val > 1000.0
                                        }
                                        fallback=|| view! { <div></div> }
                                    >
                                        <div class="hint-warning-xs">
                                            "Maximum deposit is 1,000 USDC (SEC-003 cap)"
                                        </div>
                                    </Show>
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
                                        <Show
                                            when=move || {
                                                let thb = form.get().deposit_amount_thb.parse::<u64>().unwrap_or(0);
                                                let pp = form.get().promptpay_id.trim().to_string();
                                                thb > 0 && pp.is_empty()
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "PromptPay ID is required when THB amount is set"
                                            </div>
                                        </Show>
                                    </div>
                                    // ── On-chain escrow fields: always read-only ──
                                    // These are auto-filled by wallet connect or on-chain init.
                                    // Never editable — prevents garbage input.
                                    <Show when=move || !form.get().escrow_address.is_empty() fallback=|| view! { <div></div> }>
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">
                                                "Escrow Address"
                                                <a
                                                    href=move || crate::utils::solscan_address_url(&form.get().escrow_address, &crate::utils::get_cluster())
                                                    target="_blank"
                                                    rel="noopener"
                                                    class="escrow-solscan-link"
                                                >
                                                    "Solscan"
                                                </a>
                                            </label>
                                            <div class="readonly-field">
                                                <span class="readonly-value-mono">{move || form.get().escrow_address}</span>
                                                <span class="readonly-badge">"Locked"</span>
                                            </div>
                                            <span class="quiz-setting-hint">"On-chain escrow PDA — auto-filled after on-chain init"</span>
                                        </div>
                                    </Show>

                                    <Show when=move || !form.get().organizer_wallet.is_empty() fallback=|| view! { <div></div> }>
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"Organizer Wallet"</label>
                                            <div class="readonly-field">
                                                <span class="readonly-value-mono">{move || form.get().organizer_wallet}</span>
                                                <span class="readonly-badge">"Locked"</span>
                                            </div>
                                            <span class="quiz-setting-hint" style="color:var(--success,green)">"Wallet locked — set by escrow panel"</span>
                                        </div>
                                    </Show>

                                    <Show when=move || !form.get().on_chain_event_id.is_empty() && form.get().on_chain_event_id != "0" fallback=|| view! { <div></div> }>
                                        <div class="quiz-setting-item">
                                            <label class="quiz-field-label">"On-Chain Event ID"<span class="field-optional-badge">"Auto"</span></label>
                                            <div class="readonly-field">
                                                <span class="readonly-value">{move || form.get().on_chain_event_id}</span>
                                                <span class="readonly-badge">"Locked"</span>
                                            </div>
                                            <span class="quiz-setting-hint">"Numeric ID for PDA seeds — auto-derived from slug"</span>
                                        </div>
                                    </Show>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Refund Deadline (hours)"</label>
                                        <input
                                            type="number"
                                            class="quiz-number-input"
                                            placeholder="e.g. 168 (= 7 days)"
                                            min="1"
                                            step="1"
                                            prop:value=move || form.get().refund_deadline_hours
                                            on:input=move |ev| set_form.update(|f| f.refund_deadline_hours = event_target_value(&ev))
                                        />
                                        <Show
                                            when=move || {
                                                let val = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                                form.get().deposit_enabled && val == 0
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-warning-xs">
                                                "Refund deadline must be at least 1 hour"
                                            </div>
                                        </Show>
                                        <span class="quiz-setting-hint">"Hours after event end for refund deadline (default: 168 = 7 days)"</span>
                                        // Visual timeline: show computed deadline date
                                        <Show
                                            when=move || {
                                                let end_ms = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                                                let hrs = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                                end_ms > 0 && hrs > 0
                                            }
                                            fallback=|| view! { <div></div> }
                                        >
                                            <div class="hint-success-sm">
                                                {move || {
                                                    let end_ms = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                                                    let hrs = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                                    let deadline_ms = end_ms + (hrs as i64 * 3_600_000);
                                                    let days = hrs / 24;
                                                    let day_label = if days >= 7 {
                                                        format!("{} days", days)
                                                    } else if days > 0 {
                                                        format!("{}d {}h", days, hrs % 24)
                                                    } else {
                                                        format!("{}h", hrs)
                                                    };
                                                    format!("Refund deadline: {} ({day_label} after event end)", format_date_display(deadline_ms))
                                                }}
                                            </div>
                                        </Show>
                                    </div>
                                    <div class="quiz-setting-item">
                                        <label class="quiz-field-label">"Max Refundable Deposits"</label>
                                        <input
                                            type="number"
                                            class="quiz-number-input"
                                            placeholder="e.g., 18"
                                            min="0"
                                            step="1"
                                            prop:value=move || form.get().max_refundable_deposits
                                            on:input=move |ev| set_form.update(|f| f.max_refundable_deposits = event_target_value(&ev))
                                        />
                                        <span class="quiz-setting-hint">"First N deposits get refund on check-in. Leave 0 or empty for unlimited. Deposits beyond this count are non-refundable."</span>
                                    </div>
                                </div>

                                // ── Create Mode: Wallet Connect for combined Create + Escrow Init ──
                                // Only shows when creating a new event with deposit enabled.
                                // In Edit mode, the EscrowInitPanel handles wallet connection.
                                <Show when=move || {
                                    let f = form.get();
                                    f.deposit_enabled && current_view.get() == EventsView::Create
                                }>
                                    <div class="panel-box-dashed" style="margin-top:0.75rem">
                                        <div class="panel-label">
                                            "Escrow Setup"
                                        </div>
                                        <div class="panel-hint u-mb-sm">
                                            "Connect your Solana wallet to initialize escrow when the event is created."
                                        </div>

                                        // Wallet not yet connected — show connect buttons
                                        <Show when=move || create_wallet_pk.get().is_empty() fallback=|| view! { <div></div> }>
                                            <Show when=move || !detected_wallets.get().is_empty() fallback=|| view! {
                                                <div class="panel-hint">
                                                    "No Solana wallets detected. Install Phantom or another wallet extension."
                                                </div>
                                            }>
                                                <div class="flex-wrap-row">
                                                    {move || detected_wallets.get().iter().map(|wn| {
                                                        let wn_c = wn.clone();
                                                        let set_wn = set_create_wallet_name;
                                                        let set_wp = set_create_wallet_pk;
                                                        let set_t = set_toast;
                                                        view! {
                                                            <button
                                                                class="btn btn-outline btn-sm"
                                                                on:click=move |_| {
                                                                    let wn = wn_c.clone();
                                                                    let set_wn = set_wn;
                                                                    let set_wp = set_wp;
                                                                    let set_t = set_t;
                                                                    leptos::task::spawn_local(async move {
                                                                        match super::escrow_init::connect_wallet_js(&wn).await {
                                                                            Some(pk) => {
                                                                                log::info!("[events-page] wallet connected: {} ({})", wn, &pk[..8.min(pk.len())]);
                                                                                set_wn.set(wn);
                                                                                set_wp.set(pk);
                                                                            }
                                                                            None => {
                                                                                components::show_toast(&set_t, "Wallet connection rejected", components::ToastType::Error);
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                            >
                                                                {format!("Connect {}", wn)}
                                                            </button>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </Show>
                                        </Show>

                                        // Wallet connected — show confirmation
                                        <Show when=move || !create_wallet_pk.get().is_empty() fallback=|| view! { <div></div> }>
                                            <div class="wallet-connected-bar" style="margin-bottom:0">
                                                <div class="wallet-info-left">
                                                    <div class="wallet-label">
                                                        {move || format!("{} connected", create_wallet_name.get())}
                                                    </div>
                                                    <div class="wallet-address-bold">
                                                        {move || create_wallet_pk.get()}
                                                    </div>
                                                </div>
                                                <button
                                                    class="btn btn-outline btn-xs"
                                                    on:click=move |_| {
                                                        set_create_wallet_name.set(String::new());
                                                        set_create_wallet_pk.set(String::new());
                                                    }
                                                >
                                                    "Disconnect"
                                                </button>
                                            </div>
                                            <div class="hint-success-sm" style="margin-top:0.5rem">
                                                "Creating event will also initialize escrow on-chain (wallet signature required)."
                                            </div>
                                        </Show>
                                    </div>
                                </Show>

                                // ── Escrow Management (Edit Mode Only) ──
                                // Uses Show instead of {move ||} so the EscrowInitPanel
                                // is NOT re-created when form fields update (e.g. organizer_wallet).
                                // Show only mounts/unmounts when the `when` result changes.

                                // Outer gate: only show when deposit enabled + editing an event
                                <Show when=move || {
                                    let f = form.get();
                                    f.deposit_enabled && !editing_id.get().unwrap_or_default().is_empty()
                                }>

                                    // Always show the escrow panel — it handles both init and lifecycle
                                    // (deactivate/close) based on whether escrow_address is set.
                                    <super::escrow_init::EscrowInitPanel
                                        event_id=editing_id.get().unwrap_or_default()
                                        form=form
                                        set_form=set_form
                                        set_toast=set_toast
                                    />

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
                                    disabled=move || saving.get()
                                >
                                    {move || {
                                        if saving.get() {
                                            "Saving...".to_string()
                                        } else if current_view.get() == EventsView::Edit {
                                            "Update Event".to_string()
                                        } else if !create_wallet_pk.get().is_empty() && form.get().deposit_enabled {
                                            "Create Event + Initialize Escrow".to_string()
                                        } else {
                                            "Create Event".to_string()
                                        }
                                    }}
                                </button>
                                <button class="btn btn-outline" on:click=handle_cancel>
                                    "Cancel"
                                </button>
                                <Show
                                    when=move || {
                                        current_view.get() == EventsView::Edit
                                            && !editing_id.get().unwrap_or_default().is_empty()
                                    }
                                    fallback=|| view! { <div></div> }
                                >
                                    <button
                                        class="btn btn-outline btn-archive"
                                        on:click=move |_| {
                                            let aid = editing_id.get().unwrap_or_default();
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
                                </Show>
                            </div>
                        </div>
            </Show>
        </div>
    }
}
